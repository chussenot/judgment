//! Where the answers come from.
//!
//! A [`SystemOne`] is anything that takes a state and a set of questions
//! and returns calibrated answers: the hosted model behind
//! [`Client`](crate::client::Client), an
//! open-weights model behind the same wire, a recording of an earlier run,
//! or a fake with the answers a test wants. The trait exists so that the
//! code consuming judgments never has to know which; an application, a
//! harness or a test holds a `&dyn SystemOne` and the choice is made at
//! construction. It is also the seam that makes an official SDK, should
//! one appear, an adapter rather than a rewrite.
//!
//! The state is a [`serde_json::Value`] rather than a generic `Serialize`
//! so the trait can be used as a trait object; a typed state is one
//! `serde_json::to_value` away, and [`SystemOne::answer_typed`] does it.
//! The cost is one boxed future per call and that one conversion, both
//! small next to a network round trip.
//!
//! Three implementations ship here besides the client: [`Fake`] answers
//! from a table and remembers what it was asked; [`Recorder`] wraps another
//! backend and writes every response to a directory; [`Replay`] answers
//! from such a directory without any model, keyed by a content hash of the
//! request, so a suite can run against yesterday's real answers offline.
//!
//! Every one of them returns only a response that answers the questions it
//! was given ([`Response::verify`]), so the code consuming judgments can
//! rely on it whichever backend is behind the trait: a test with a [`Fake`]
//! cannot pass on a scripted answer the real client would have refused, and
//! a [`Replay`] never answers questions it was not recorded for. The request
//! hash covers the questions, so a changed question set is
//! [`Error::NoRecording`], and a recording filed under the right hash that
//! no longer fits (edited by hand, or made by an older release that did not
//! check) fails naming the question.

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
/// The contract: an implementation returns only a [`Response`] that fits
/// `questions`, that is one [`Response::verify`] accepts, and an error
/// otherwise (one of the [`Error::is_unfit`] errors when the answer came
/// back but does not fit). Every backend in this crate verifies, so a caller
/// reads the answers through its handles without checking them again. A
/// backend written elsewhere should call [`Response::verify`] before it
/// returns, since nothing else will; [`Recorder`] verifies what the backend
/// it wraps returns, because that may be one of those.
pub trait SystemOne: Send + Sync {
    /// Answer `questions` about `state` with `model` (a name or alias; a
    /// backend that has one model may ignore it and report its own).
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
/// client's own settings and default headers: per-call options do not cross
/// the trait (`client` module docs, `# Per-call options`). If extra body
/// fields ever do, they must enter [`request_hash`], with an empty set
/// hashing as it does today, or a [`Replay`] would answer a different
/// request.
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

// ---------------------------------------------------------------------------
// Fake
// ---------------------------------------------------------------------------

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
/// Every question in a request must have an answer, or the call fails with
/// [`Error::MissingAnswer`] naming it: a test that forgets a question learns
/// so from the fake, not from a wrong decision downstream. The response is
/// verified against the questions like the client's ([`Response::verify`]),
/// so a scripted answer of the wrong primitive, a Choice option the question
/// does not offer or a Score off its scale fails the call, as it would from
/// the real client, and a test cannot pass on an answer the client would
/// have refused. A refused call is not recorded in [`Fake::calls`]. A test
/// that needs a malformed response builds the [`Response`] directly.
///
/// A Score is scripted as probabilities only ([`Fake::score`]); its legend
/// is the levels of the question it answers, filled in when it answers, so
/// the echo the client checks is the one a server would send.
#[derive(Debug, Default)]
pub struct Fake {
    model: String,
    answers: BTreeMap<String, Scripted>,
    usage: Usage,
    calls: Mutex<Vec<Call>>,
}

/// One scripted answer: a wire answer as given, or a Score whose legend and
/// score are derived from the question it answers.
#[derive(Debug, Clone)]
enum Scripted {
    Answer(Answer),
    Score {
        probabilities: Vec<Probability>,
        confidence: Confidence,
    },
}

impl Scripted {
    /// The wire answer to `question`. A scripted Score echoes `question`'s
    /// levels as its legend when `question` is a Score, and has an empty
    /// legend otherwise, so the check then reports the primitive rather
    /// than the legend.
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

    /// The wire answer for question `id`, stored as given: for a shape the
    /// helpers below do not build, or to feed a real recorded answer back
    /// through a fake. It is verified like any other when the fake answers,
    /// so a Score given here must carry the question's levels as its legend.
    #[must_use]
    pub fn with_answer(mut self, id: impl Into<String>, answer: Answer) -> Self {
        self.answers.insert(id.into(), Scripted::Answer(answer));
        self
    }

    /// A Noul answer: the probability of yes. Fails when `p_yes` is outside
    /// `[0, 1]`, the same check the wire gets.
    pub fn noul(self, id: impl Into<String>, p_yes: f64) -> Result<Self> {
        let noul = Probability::new(p_yes)?;
        Ok(self.with_answer(id, Answer::Noul { noul }))
    }

    /// A Choice answer from `(option, probability)` pairs; the chosen option
    /// is the most probable one and the confidence is as given. Fails when a
    /// probability or the confidence is outside `[0, 1]`. Whether the
    /// probabilities sum to 1 is not checked, since the wire layer does not
    /// check it either. Every option named here must be one the question
    /// offers, or the fake refuses the call with [`Error::UnknownOption`];
    /// an offered option left out reads as zero.
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

    /// A Score answer from one probability per level, lowest level first.
    /// When the fake answers, the legend is the levels of the question it
    /// answers, as a server echoes them, and the score is the
    /// probability-weighted position `Σ i·p_i`. Fails when a probability or
    /// the confidence is outside `[0, 1]`.
    ///
    /// Fewer probabilities than the question has levels read as zero for the
    /// rest; more than it has levels is [`Error::InvalidAnswer`] when the
    /// fake answers. The probabilities should sum to at most 1, or the
    /// derived score can leave the scale: `[0, 0, 1, 1]` on four levels gives
    /// a score of 5, which the fake refuses with [`Error::InvalidAnswer`] as
    /// the client would.
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

    /// Every request received so far, oldest first: what a test asserts to
    /// check that the code under test asked the right questions about the
    /// right state with the right model.
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

// ---------------------------------------------------------------------------
// Recorder and Replay
// ---------------------------------------------------------------------------

/// A backend that passes every request to another and writes the response
/// to `dir/<request hash>.json` as a [`Recording`], so a [`Replay`] over the
/// same directory answers the same requests later without a model.
///
/// The response is written as received, [`Response::request_id`] included,
/// so a recording still names the call TypeSafe can look up.
///
/// The inner response is verified against the questions before anything is
/// written ([`Response::verify`]), and a response that does not fit is
/// returned as the error with no file written: a recording is only ever a
/// response a replay can return. The error and its request id are then the
/// only trace of that response. For the crate's own backends the check has
/// already been made; it matters for a third-party backend behind the
/// recorder, which the [`SystemOne`] contract asks to verify but nothing
/// forces to.
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

    /// The wrapped backend, for reading what it saw: a [`Fake`]'s calls, for
    /// example.
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
/// [`Response::request_id`], not a new one: it is that call's answers, and
/// the id is how to find that call in TypeSafe's logs. A recording made
/// before the field existed replays with `None`.
///
/// A recorded response is verified against the questions of the request
/// that found it ([`Response::verify`]), as the client verifies a live one,
/// so a recording that no longer fits (edited by hand, or made by an older
/// release that did not check) fails naming the question rather than
/// replaying an answer the client would refuse. The request hash and the
/// fingerprint both cover the questions, so a recording found by either was
/// made for these questions.
///
/// With the `jud` feature, `*.jud` files of kind `recording` are read
/// beside the `*.json` ones ([`crate::jud`]), and a recording that carries
/// only a [`Recording::fingerprint`] (one made by another tool) is found by
/// it.
#[derive(Debug, Default)]
pub struct Replay {
    responses: Vec<Response>,
    by_hash: BTreeMap<String, usize>,
    by_fingerprint: BTreeMap<String, usize>,
}

impl Replay {
    /// Load every `*.json` recording under `dir` (and every `*.jud`
    /// recording, with the `jud` feature) that carries a request hash or a
    /// fingerprint. Recordings with neither (a harness's, keyed by case) are
    /// skipped; a file that is not a recording is an error.
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

    /// Add one recording, under its request hash, its fingerprint, or both.
    /// A recording with neither is skipped. A later recording of the same
    /// request replaces an earlier one.
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

    /// True when the directory held no hashed or fingerprinted recording:
    /// it was never recorded, or it was recorded by a harness keyed by case
    /// id, which [`Replay::open`] skips.
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
