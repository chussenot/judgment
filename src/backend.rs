//! Where the answers come from.
//!
//! A [`SystemOne`] takes a state and questions and returns calibrated
//! answers: the hosted model behind [`Client`](crate::client::Client), a
//! compatible server, a recording ([`Replay`]) or a [`Fake`]. The code
//! consuming judgments holds a `&dyn SystemOne` and the choice is made at
//! construction. The state is a [`serde_json::Value`] rather than a generic
//! `Serialize` so the trait can be a trait object;
//! [`SystemOne::answer_typed`] converts. Every backend returns only a
//! response that answers the questions it was given ([`Response::verify`]);
//! `docs/testing.md` shows the backends in use.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;

use crate::answer::{Answer, Confidence, Probability, Response, Usage};
use crate::error::{Error, Result};
use crate::eval::{Recording, request_hash};
use crate::question::{Question, Questions};

/// A boxed, sendable future: what a trait object can return.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Something that answers typed questions about a state.
///
/// The contract: only a [`Response`] that [`Response::verify`] accepts for
/// `questions`, or an error ([`Error::is_unfit`] when an answer came back
/// but does not fit), so a caller reads the answers through its handles
/// without checking again. A backend written elsewhere must verify itself;
/// [`Recorder`] verifies what it wraps because nothing forces that.
pub trait SystemOne: Send + Sync {
    /// Answer `questions` about `state` with `model` (a name or alias; a
    /// backend with one model may ignore it and report its own).
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>>;

    /// [`Self::answer`] over any serialisable state.
    fn answer_typed<'a, S: Serialize + Sync>(
        &'a self,
        state: &'a S,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let state = serde_json::to_value(state)?;
            self.answer(&state, model, questions).await
        })
    }
}

impl<T: SystemOne + ?Sized> SystemOne for &T {
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>> {
        (**self).answer(state, model, questions)
    }
}

impl<T: SystemOne + ?Sized> SystemOne for std::sync::Arc<T> {
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>> {
        (**self).answer(state, model, questions)
    }
}

impl<T: SystemOne + ?Sized> SystemOne for Box<T> {
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>> {
        (**self).answer(state, model, questions)
    }
}

/// Through [`Client::evaluate`](crate::client::Client::evaluate), with the
/// client's own settings: per-call options do not cross the trait, because
/// a [`Replay`] is keyed by [`request_hash`] and an extra body field can
/// change the answer. If they ever do, they enter that hash.
#[cfg(feature = "http")]
impl SystemOne for crate::client::Client {
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>> {
        Box::pin(async move {
            self.evaluate(&crate::client::Request {
                state,
                model,
                questions,
            })
            .await
        })
    }
}

/// One request a [`Fake`] received.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// The state as sent.
    pub state: Value,
    /// The model asked for.
    pub model: String,
    /// The question ids, in order.
    pub question_ids: Vec<String>,
}

/// A backend that answers from a table and remembers what it was asked.
///
/// A question without a scripted answer is [`Error::MissingAnswer`], and the
/// response is verified like the client's ([`Response::verify`]), so a test
/// cannot pass on an answer the client would have refused; a refused call
/// is not in [`Fake::calls`]. A test that needs a malformed response builds
/// the [`Response`] directly.
#[derive(Debug, Default)]
pub struct Fake {
    model: String,
    answers: BTreeMap<String, Scripted>,
    usage: Usage,
    calls: Mutex<Vec<Call>>,
}

/// A wire answer as given, or a Score derived from the question it answers.
#[derive(Debug, Clone)]
enum Scripted {
    Answer(Answer),
    Score {
        probabilities: Vec<Probability>,
        confidence: Confidence,
    },
}

impl Scripted {
    /// A scripted Score takes `question`'s levels as its legend, or an empty
    /// one when `question` is not a Score, so the check reports the primitive.
    #[allow(clippy::cast_precision_loss)] // at most ten levels
    fn answer(&self, question: &Question) -> Answer {
        match self {
            Self::Answer(answer) => answer.clone(),
            Self::Score {
                probabilities,
                confidence,
            } => {
                let legend = match question {
                    Question::Score { criteria, .. } => criteria
                        .iter()
                        .enumerate()
                        .map(|(i, level)| (i.to_string(), level.clone()))
                        .collect(),
                    _ => BTreeMap::new(),
                };
                let score = probabilities
                    .iter()
                    .enumerate()
                    .map(|(i, p)| i as f64 * p.value())
                    .sum();
                Answer::Score {
                    score,
                    legend,
                    probabilities: probabilities
                        .iter()
                        .enumerate()
                        .map(|(i, p)| (i.to_string(), *p))
                        .collect(),
                    confidence: *confidence,
                }
            }
        }
    }
}

impl Fake {
    /// An empty fake reporting model `fake`.
    pub fn new() -> Self {
        Self {
            model: "fake".to_owned(),
            ..Self::default()
        }
    }

    /// The model name every response reports.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// The usage every response reports.
    #[must_use]
    pub fn usage(mut self, usage: Usage) -> Self {
        self.usage = usage;
        self
    }

    /// The wire answer for question `id`, stored as given, for a shape the
    /// helpers do not build or a recorded answer fed back; a Score given
    /// here carries the question's levels as its legend.
    #[must_use]
    pub fn with_answer(mut self, id: impl Into<String>, answer: Answer) -> Self {
        self.answers.insert(id.into(), Scripted::Answer(answer));
        self
    }

    /// A Noul answer: the probability of yes, checked as on the wire.
    pub fn noul(self, id: impl Into<String>, p_yes: f64) -> Result<Self> {
        let noul = Probability::new(p_yes)?;
        Ok(self.with_answer(id, Answer::Noul { noul }))
    }

    /// A Choice answer from `(option, probability)` pairs; the chosen option
    /// is the most probable, the probabilities need not sum to 1 (nor on
    /// the wire), an option the question does not offer is
    /// [`Error::UnknownOption`] when the fake answers.
    pub fn choice<'p>(
        self,
        id: impl Into<String>,
        probabilities: impl IntoIterator<Item = (&'p str, f64)>,
        confidence: f64,
    ) -> Result<Self> {
        let mut probs = BTreeMap::new();
        let mut best: Option<(String, f64)> = None;
        for (option, p) in probabilities {
            probs.insert(option.to_owned(), Probability::new(p)?);
            if best.as_ref().is_none_or(|(_, b)| p > *b) {
                best = Some((option.to_owned(), p));
            }
        }
        let choice = best.map(|(k, _)| k).unwrap_or_default();
        Ok(self.with_answer(
            id,
            Answer::Choice {
                choice,
                probabilities: probs,
                confidence: Confidence::new(confidence)?,
            },
        ))
    }

    /// A Score answer from one probability per level, lowest first; the
    /// legend is the question's levels, as a server echoes them, and the
    /// score `Σ i·p_i`. Fewer probabilities than levels read as zero; more,
    /// or a sum above 1 that pushes the score off the scale, are
    /// [`Error::InvalidAnswer`] when the fake answers.
    pub fn score(
        mut self,
        id: impl Into<String>,
        probabilities: impl IntoIterator<Item = f64>,
        confidence: f64,
    ) -> Result<Self> {
        let probabilities = probabilities
            .into_iter()
            .map(Probability::new)
            .collect::<Result<Vec<_>>>()?;
        let confidence = Confidence::new(confidence)?;
        self.answers.insert(
            id.into(),
            Scripted::Score {
                probabilities,
                confidence,
            },
        );
        Ok(self)
    }

    /// Every request received so far, oldest first.
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

impl SystemOne for Fake {
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>> {
        Box::pin(async move {
            let answers = questions
                .iter()
                .filter_map(|(id, question)| {
                    self.answers
                        .get(id)
                        .map(|scripted| (id.to_owned(), scripted.answer(question)))
                })
                .collect();
            let response = Response {
                model: self.model.clone(),
                answers,
                usage: self.usage,
                request_id: None,
                extra: BTreeMap::new(),
            };
            // A question without a script is the verify's MissingAnswer.
            response.verify(questions)?;
            if let Ok(mut calls) = self.calls.lock() {
                calls.push(Call {
                    state: state.clone(),
                    model: model.to_owned(),
                    question_ids: questions.ids().map(str::to_owned).collect(),
                });
            }
            Ok(response)
        })
    }
}

/// A backend that passes every request to another and writes the response
/// to `dir/<request hash>.json` as a [`Recording`], for a [`Replay`] over
/// the same directory.
///
/// The response is written as received, [`Response::request_id`] included,
/// so a recording still names the call TypeSafe can look up, and only when
/// it fits the questions ([`Response::verify`]): a recording is only ever a
/// response a replay can return.
#[derive(Debug)]
pub struct Recorder<B> {
    inner: B,
    dir: PathBuf,
}

impl<B: SystemOne> Recorder<B> {
    /// Record `inner`'s answers under `dir`, created on first use.
    pub fn new(inner: B, dir: impl Into<PathBuf>) -> Self {
        Self {
            inner,
            dir: dir.into(),
        }
    }

    /// The wrapped backend, for reading what it saw: a [`Fake`]'s calls, say.
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

impl<B: SystemOne> SystemOne for Recorder<B> {
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>> {
        Box::pin(async move {
            let hash = request_hash(state, questions);
            let started = Instant::now();
            let response = self.inner.answer(state, model, questions).await?;
            response.verify(questions)?;
            let recording = Recording {
                case: hash.clone(),
                response: response.clone(),
                elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                request_hash: Some(hash),
                fingerprint: Some(crate::eval::canonical::request_fingerprint(
                    state, questions,
                )),
                rubric: None,
                server: None,
                recorded_at: Some(crate::eval::now_rfc3339()),
            };
            crate::eval::write_recording(&self.dir, &recording).map_err(|e| Error::Io {
                context: format!("recording under {}", self.dir.display()),
                source: std::io::Error::other(e),
            })?;
            Ok(response)
        })
    }
}

/// A backend that answers from recordings keyed by request hash, and
/// nothing else: a request nobody recorded is [`Error::NoRecording`].
///
/// A replayed response carries the recorded call's
/// [`Response::request_id`], the way to find that call in TypeSafe's logs,
/// and is verified against the questions that found it
/// ([`Response::verify`]), so a recording edited by hand fails naming the
/// question. With the `jud` feature, `*.jud` recordings are read beside the
/// `*.json` ones ([`crate::jud`]), found by [`Recording::fingerprint`] when
/// another tool made them.
#[derive(Debug, Default)]
pub struct Replay {
    responses: Vec<Response>,
    by_hash: BTreeMap<String, usize>,
    by_fingerprint: BTreeMap<String, usize>,
}

impl Replay {
    /// Load every `*.json` (and, with the `jud` feature, `*.jud`) recording
    /// under `dir` that carries a request hash or a fingerprint; a harness's
    /// recording keyed by case alone is skipped.
    pub fn open(dir: &Path) -> Result<Self> {
        let mut replay = Self::default();
        let entries = std::fs::read_dir(dir).map_err(|source| Error::Io {
            context: dir.display().to_string(),
            source,
        })?;
        for entry in entries {
            let path = entry
                .map_err(|source| Error::Io {
                    context: dir.display().to_string(),
                    source,
                })?
                .path();
            let extension = path.extension().and_then(|e| e.to_str());
            let recording = match extension {
                Some("json") => {
                    let text = read(&path)?;
                    serde_json::from_str::<Recording>(&text)?
                }
                #[cfg(feature = "jud")]
                Some(crate::jud::EXTENSION) => {
                    let text = read(&path)?;
                    crate::jud::parse_recording(&text).map_err(|source| {
                        Error::InvalidRecording {
                            path: path.display().to_string(),
                            reason: source.to_string(),
                        }
                    })?
                }
                _ => continue,
            };
            replay.add(recording);
        }
        Ok(replay)
    }

    /// Add one recording under its request hash and its fingerprint (neither:
    /// skipped); a later recording of the same request replaces an earlier.
    pub fn add(&mut self, recording: Recording) {
        let Recording {
            response,
            request_hash,
            fingerprint,
            ..
        } = recording;
        if request_hash.is_none() && fingerprint.is_none() {
            return;
        }
        let index = self.responses.len();
        self.responses.push(response);
        if let Some(fingerprint) = fingerprint {
            self.by_fingerprint.insert(fingerprint, index);
        }
        if let Some(hash) = request_hash {
            self.by_hash.insert(hash, index);
        }
    }

    /// Number of recordings this replay can answer from.
    pub fn len(&self) -> usize {
        self.responses.len()
    }

    /// True when no recording carries a hash or a fingerprint.
    pub fn is_empty(&self) -> bool {
        self.by_hash.is_empty() && self.by_fingerprint.is_empty()
    }
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|source| Error::Io {
        context: path.display().to_string(),
        source,
    })
}

impl SystemOne for Replay {
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        _model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>> {
        Box::pin(async move {
            let hash = request_hash(state, questions);
            let index = if let Some(index) = self.by_hash.get(&hash) {
                *index
            } else {
                let fingerprint = crate::eval::canonical::request_fingerprint(state, questions);
                *self
                    .by_fingerprint
                    .get(&fingerprint)
                    .ok_or(Error::NoRecording(hash))?
            };
            let response = self.responses[index].clone();
            response.verify(questions)?;
            Ok(response)
        })
    }
}
