//! The `rubric` kind: questions in wire shape, and the policy that reads
//! their answers.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Error, Result, VERSION, check_version, from_text, to_yaml};
use crate::answer::{Choice, Confidence, FromAnswer, Noul, Probability, Response, Score};
use crate::eval::canonical;
use crate::question::{NoulCriteria, Question, Questions};

/// A rubric: the questions a request sends, in wire shape and wire order,
/// with the policy an application reads the answers by.
///
/// The questions are a [`Questions`], built through the same checks as one
/// written in code, so a rubric that parses is a request that can be sent.
/// The policy is never sent: it says, per question, where an answer becomes
/// an action ([`Gate`]), and where that number came from ([`Tuning`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Rubric {
    /// A name for the rubric, stable across edits: what a recording's
    /// `rubric` or a cases document's `rubric` may name.
    pub id: String,
    /// An edition of the rubric, free-form (`3`, `2026-10`), when the
    /// author keeps one; the questions' [`fingerprint`](Self::fingerprint)
    /// is the exact identity.
    pub version: Option<String>,
    /// What the rubric decides, for the person reading it.
    pub description: Option<String>,
    /// The questions, exactly as sent.
    pub questions: Questions,
    /// The gates and their provenance.
    pub policy: Policy,
}

/// What an application does with the answers: one [`Gate`] per gated
/// question, and the [`Tuning`] the gates came from.
///
/// In the document the gates are the `policy` map and the tuning is the
/// top-level `tuning` object; the two are kept together here because a gate
/// without its provenance is a number nobody can revisit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Policy {
    /// Gates by question id, in document order. A question without a gate
    /// is read at the defaults [`Rubric::apply`] documents.
    pub gates: IndexMap<String, Gate>,
    /// Where the gates came from, when recorded.
    pub tuning: Option<Tuning>,
}

/// Where one question's answer becomes an action. Every field is optional
/// and each applies to one primitive, which [`Rubric::parse`] checks:
/// `threshold` to a Noul, `confidence` and `fallback` to a Choice or a
/// Score.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    /// A Noul is yes at this probability and above; 0.5 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// A Choice or a Score is acted on at this confidence and above, and
    /// [`Verdict::Deferred`] below it; 0 when absent, so every answer is
    /// acted on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// What a deferred Choice or Score falls back to: an offered option key
    /// or a level (its text, or its index as a string). It is the rubric's
    /// no-match option made explicit; absent, a deferred answer has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    /// Why the bar is where it is, for the person reading it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Where the gates came from: the cases they were tuned on, the model and
/// the server that answered them, when. Every field is optional; a rubric
/// written by hand records none and says so.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Tuning {
    /// The cases document, by id or by fingerprint (`sha256:…`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cases: Option<String>,
    /// The versioned model the cases were answered by (`jev-1.13.0`): a
    /// gate is tuned per version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The server that answered, as a base URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// When, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuned_at: Option<String>,
    /// Anything else the tuner wants kept: the accuracy and coverage at the
    /// chosen bar, the sweep itself, a link to the run.
    #[serde(flatten)]
    pub extra: IndexMap<String, Value>,
}

/// What a gated answer says, per question, from [`Rubric::apply`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Verdict {
    /// A Noul at or above its threshold.
    Yes {
        /// The probability of yes.
        probability: Probability,
    },
    /// A Noul below its threshold.
    No {
        /// The probability of yes.
        probability: Probability,
    },
    /// A Choice at or above its confidence bar.
    Option {
        /// The chosen option key.
        key: String,
        /// The answer's confidence.
        confidence: Confidence,
    },
    /// A Score at or above its confidence bar.
    Level {
        /// The nearest level, as an index into the rubric's levels.
        index: usize,
        /// The nearest level's text.
        label: String,
        /// The probability-weighted position.
        value: f64,
        /// The answer's confidence.
        confidence: Confidence,
    },
    /// A Choice or a Score below its bar: the application routes to the
    /// fallback, or to a person.
    Deferred(Deferred),
}

/// A Choice or Score answer the gate did not let through.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Deferred {
    /// The gate's fallback, when it has one.
    pub fallback: Option<String>,
    /// What the model would have answered: the chosen option key, or the
    /// nearest level's index as a string.
    pub nearest: String,
    /// The answer's confidence, below the bar.
    pub confidence: Confidence,
    /// The bar it was below.
    pub bar: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRubric {
    #[serde(default)]
    jud: Option<Value>,
    kind: String,
    id: String,
    #[serde(default)]
    version: Option<Value>,
    #[serde(default)]
    description: Option<String>,
    questions: IndexMap<String, RawQuestion>,
    #[serde(default)]
    policy: IndexMap<String, Gate>,
    #[serde(default)]
    tuning: Option<Tuning>,
}

/// The wire shape of a question, read. [`Question`] only serialises; this
/// is its mirror, and lowering goes through the builder's checks.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum RawQuestion {
    Noul {
        #[serde(default)]
        instructions: Value,
        #[serde(default)]
        criteria: Option<RawNoulCriteria>,
    },
    Choice {
        #[serde(default)]
        instructions: Value,
        criteria: IndexMap<String, Value>,
    },
    Score {
        #[serde(default)]
        instructions: Value,
        criteria: Vec<Value>,
    },
}

/// YAML `true:` and `false:` keys are booleans to a YAML parser and the
/// strings `true` and `false` on the wire (and in JSON). A key is read
/// either way and any other key is refused.
type RawNoulCriteria = IndexMap<CriteriaKey, Value>;

#[derive(Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
enum CriteriaKey {
    Bool(bool),
    Text(String),
}

fn noul_criteria(id: &str, raw: RawNoulCriteria) -> Result<NoulCriteria> {
    let mut criteria = NoulCriteria::default();
    for (key, value) in raw {
        match key {
            CriteriaKey::Bool(true) => criteria.yes = Some(value),
            CriteriaKey::Bool(false) => criteria.no = Some(value),
            CriteriaKey::Text(text) => match text.as_str() {
                "true" => criteria.yes = Some(value),
                "false" => criteria.no = Some(value),
                other => {
                    return Err(Error::Invalid {
                        field: format!("questions.{id}.criteria"),
                        reason: format!("a Noul's criteria are `true` and `false`, not `{other}`"),
                    });
                }
            },
        }
    }
    Ok(criteria)
}

impl RawQuestion {
    fn lower(self, id: &str) -> Result<Question> {
        Ok(match self {
            Self::Noul {
                instructions,
                criteria,
            } => Question::Noul {
                instructions,
                criteria: criteria.map(|c| noul_criteria(id, c)).transpose()?,
            },
            Self::Choice {
                instructions,
                criteria,
            } => Question::Choice {
                instructions,
                criteria,
            },
            Self::Score {
                instructions,
                criteria,
            } => Question::Score {
                instructions,
                criteria,
            },
        })
    }
}

#[derive(Serialize)]
struct RubricDoc<'a> {
    jud: u64,
    kind: &'static str,
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    questions: &'a Questions,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    policy: &'a IndexMap<String, Gate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tuning: Option<&'a Tuning>,
}

impl Rubric {
    /// A rubric over questions built in code, with no policy yet: the way
    /// to write one out for the first time. [`Rubric::gate`] adds gates.
    pub fn new(id: impl Into<String>, questions: Questions) -> Self {
        Self {
            id: id.into(),
            version: None,
            description: None,
            questions,
            policy: Policy::default(),
        }
    }

    /// Set a gate, checked against the question it is for.
    pub fn gate(&mut self, id: impl Into<String>, gate: Gate) -> Result<()> {
        let id = id.into();
        validate_gate(&id, &gate, &self.questions)?;
        self.policy.gates.insert(id, gate);
        Ok(())
    }

    /// Parse a `rubric` document, YAML or JSON.
    ///
    /// Each question goes through the [`Questions`] builder, so it gets the
    /// checks a question written in code gets ([`Error::Question`]); each
    /// gate is checked against its question's primitive and options
    /// ([`Error::Policy`]).
    pub fn parse(text: &str) -> Result<Self> {
        let raw: RawRubric = from_text(text)?;
        check_version(raw.jud.as_ref())?;
        if raw.kind != "rubric" {
            return Err(Error::Kind { found: raw.kind });
        }
        if raw.id.is_empty() {
            return Err(Error::Invalid {
                field: "id".to_owned(),
                reason: "a rubric needs a non-empty id".to_owned(),
            });
        }
        if raw.questions.is_empty() {
            return Err(Error::Invalid {
                field: "questions".to_owned(),
                reason: "a rubric needs at least one question".to_owned(),
            });
        }
        let mut questions = Questions::new();
        for (id, question) in raw.questions {
            let question = question.lower(&id)?;
            questions
                .add(id.clone(), question)
                .map_err(|source| Error::Question {
                    id,
                    source: Box::new(source),
                })?;
        }
        for (id, gate) in &raw.policy {
            validate_gate(id, gate, &questions)?;
        }
        Ok(Self {
            id: raw.id,
            version: raw.version.map(|v| match v {
                Value::String(s) => s,
                other => other.to_string(),
            }),
            description: raw.description,
            questions,
            policy: Policy {
                gates: raw.policy,
                tuning: raw.tuning,
            },
        })
    }

    /// The rubric as a `.jud` document, YAML.
    pub fn to_yaml(&self) -> Result<String> {
        to_yaml(&RubricDoc {
            jud: VERSION,
            kind: "rubric",
            id: &self.id,
            version: self.version.as_deref(),
            description: self.description.as_deref(),
            questions: &self.questions,
            policy: &self.policy.gates,
            tuning: self.policy.tuning.as_ref(),
        })
    }

    /// The fingerprint of the questions, `sha256:…` over their canonical
    /// JSON ([`canonical::fingerprint`]): the exact identity of what the
    /// model is asked, the same from every implementation, unchanged by
    /// the policy, the id or the description. It is what a recording's
    /// `rubric` names when it needs more than the id.
    pub fn fingerprint(&self) -> String {
        canonical::fingerprint(&serde_json::to_value(&self.questions).unwrap_or(Value::Null))
    }

    /// Read a response through the policy, one [`Verdict`] per question
    /// in wire order.
    ///
    /// The response is verified against the questions first
    /// ([`Response::verify`], [`Error::Response`]). A Noul is [`Verdict::Yes`]
    /// at its gate's `threshold` and above, 0.5 without a gate. A Choice or
    /// a Score is [`Verdict::Option`] or [`Verdict::Level`] at its gate's
    /// `confidence` and above, and [`Verdict::Deferred`] below it, carrying
    /// the gate's `fallback`; without a gate the bar is 0 and nothing is
    /// deferred.
    pub fn apply(&self, response: &Response) -> Result<IndexMap<String, Verdict>> {
        response
            .verify(&self.questions)
            .map_err(|source| Error::Response {
                source: Box::new(source),
            })?;
        let mut verdicts = IndexMap::with_capacity(self.questions.len());
        for (id, question) in self.questions.iter() {
            let gate = self.policy.gates.get(id);
            let answer = response.answers.get(id).ok_or_else(|| Error::Response {
                source: Box::new(crate::Error::MissingAnswer {
                    id: id.to_owned(),
                    request_id: response.request_id.clone(),
                }),
            })?;
            let verdict = match question {
                Question::Noul { .. } => {
                    let noul = Noul::from_answer(id, answer).map_err(|source| Error::Response {
                        source: Box::new(source),
                    })?;
                    let threshold = gate.and_then(|g| g.threshold).unwrap_or(0.5);
                    if noul.yes.at_least(threshold) {
                        Verdict::Yes {
                            probability: noul.yes,
                        }
                    } else {
                        Verdict::No {
                            probability: noul.yes,
                        }
                    }
                }
                Question::Choice { .. } => {
                    let choice = Choice::<String>::from_answer(id, answer).map_err(|source| {
                        Error::Response {
                            source: Box::new(source),
                        }
                    })?;
                    let bar = gate.and_then(|g| g.confidence).unwrap_or(0.0);
                    if choice.confidence.at_least(bar) {
                        Verdict::Option {
                            key: choice.chosen,
                            confidence: choice.confidence,
                        }
                    } else {
                        Verdict::Deferred(Deferred {
                            fallback: gate.and_then(|g| g.fallback.clone()),
                            nearest: choice.chosen,
                            confidence: choice.confidence,
                            bar,
                        })
                    }
                }
                Question::Score { criteria, .. } => {
                    let score =
                        Score::from_answer(id, answer).map_err(|source| Error::Response {
                            source: Box::new(source),
                        })?;
                    let bar = gate.and_then(|g| g.confidence).unwrap_or(0.0);
                    let index = score.nearest_level();
                    if score.confidence.at_least(bar) {
                        Verdict::Level {
                            index,
                            label: level_text(criteria.get(index)),
                            value: score.value,
                            confidence: score.confidence,
                        }
                    } else {
                        Verdict::Deferred(Deferred {
                            fallback: gate.and_then(|g| g.fallback.clone()),
                            nearest: index.to_string(),
                            confidence: score.confidence,
                            bar,
                        })
                    }
                }
            };
            verdicts.insert(id.to_owned(), verdict);
        }
        Ok(verdicts)
    }
}

/// A level as text: a string level is itself, a structured one its compact
/// JSON, as [`Score::levels`] reads it.
pub(crate) fn level_text(level: Option<&Value>) -> String {
    match level {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

/// The index of the level a label names: its index as a string, or its
/// text.
pub(crate) fn level_index(levels: &[Value], label: &str) -> Option<usize> {
    if let Ok(index) = label.parse::<usize>()
        && index < levels.len()
    {
        return Some(index);
    }
    levels
        .iter()
        .position(|level| level_text(Some(level)) == label)
}

fn validate_gate(id: &str, gate: &Gate, questions: &Questions) -> Result<()> {
    let policy = |reason: String| {
        Err(Error::Policy {
            id: id.to_owned(),
            reason,
        })
    };
    let Some(question) = questions.get(id) else {
        return policy("no question has this id".to_owned());
    };
    let unit = |name: &str, value: Option<f64>| -> Result<()> {
        match value {
            Some(v) if !(0.0..=1.0).contains(&v) => Err(Error::Policy {
                id: id.to_owned(),
                reason: format!("`{name}` must be between 0 and 1, got {v}"),
            }),
            _ => Ok(()),
        }
    };
    unit("threshold", gate.threshold)?;
    unit("confidence", gate.confidence)?;
    match question {
        Question::Noul { .. } => {
            if gate.confidence.is_some() || gate.fallback.is_some() {
                return policy(
                    "a Noul is gated by `threshold` alone; `confidence` and `fallback` are for a Choice or a Score"
                        .to_owned(),
                );
            }
        }
        Question::Choice { criteria, .. } => {
            if gate.threshold.is_some() {
                return policy("a Choice is gated by `confidence`, not `threshold`".to_owned());
            }
            if let Some(fallback) = &gate.fallback
                && !criteria.contains_key(fallback)
            {
                return policy(format!(
                    "`fallback: {fallback}` is not one of the offered options"
                ));
            }
        }
        Question::Score { criteria, .. } => {
            if gate.threshold.is_some() {
                return policy("a Score is gated by `confidence`, not `threshold`".to_owned());
            }
            if let Some(fallback) = &gate.fallback
                && level_index(criteria, fallback).is_none()
            {
                return policy(format!(
                    "`fallback: {fallback}` is neither a level's text nor a level index"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;
    use crate::Fake;
    use crate::backend::SystemOne;
    use serde_json::json;

    const RUBRIC: &str = r"
jud: 1
kind: rubric
id: triage
version: 3
description: Route a message.
questions:
  actionable:
    type: noul
    instructions: Does `message` ask for something to be done?
    criteria:
      true: a request, a report of something broken
      false: small talk, an acknowledgement
  owner:
    type: choice
    instructions: Who should handle `message`?
    criteria:
      billing: Invoices, payments, refunds
      support: Everything about using the product
      none_of_these: Not clearly either
  tone:
    type: score
    instructions: How upset is the writer of `message`?
    criteria: [calm, annoyed, angry]
policy:
  actionable:
    threshold: 0.65
    note: tuned on cases-2026-10, F1 0.91
  owner:
    confidence: 0.4
    fallback: none_of_these
  tone:
    confidence: 0.3
tuning:
  cases: sha256:0000000000000000000000000000000000000000000000000000000000000000
  model: jev-1.13.0
  server: https://api.typesafe.ai
  tuned_at: 2026-10-04T10:00:00Z
  accuracy: 0.93
";

    #[test]
    fn a_rubric_lowers_to_questions_in_wire_order() {
        let rubric = Rubric::parse(RUBRIC).unwrap();
        assert_eq!(rubric.id, "triage");
        assert_eq!(rubric.version.as_deref(), Some("3"));
        let ids: Vec<&str> = rubric.questions.ids().collect();
        assert_eq!(ids, ["actionable", "owner", "tone"]);
        let wire = serde_json::to_value(&rubric.questions).unwrap();
        assert_eq!(
            wire["actionable"]["criteria"]["true"],
            "a request, a report of something broken"
        );
        assert_eq!(wire["owner"]["type"], "choice");
        assert_eq!(
            wire["tone"]["criteria"],
            json!(["calm", "annoyed", "angry"])
        );
        // Option order on the wire is the document's, not alphabetical
        // (`serde_json::to_value` sorts; the client streams with `to_vec`).
        let body = serde_json::to_string(&rubric.questions).unwrap();
        let at = |key: &str| body.find(&format!("\"{key}\"")).unwrap();
        assert!(
            at("billing") < at("support") && at("support") < at("none_of_these"),
            "{body}"
        );
        assert_eq!(rubric.policy.gates["actionable"].threshold, Some(0.65));
        let tuning = rubric.policy.tuning.as_ref().unwrap();
        assert_eq!(tuning.model.as_deref(), Some("jev-1.13.0"));
        assert_eq!(tuning.extra["accuracy"], json!(0.93));
    }

    #[test]
    fn the_builder_s_checks_apply_to_a_parsed_question() {
        let text = "jud: 1\nkind: rubric\nid: r\nquestions:\n  c:\n    type: choice\n    instructions: pick\n    criteria: {only: one}\n";
        let err = Rubric::parse(text).unwrap_err();
        assert!(
            matches!(&err, Error::Question { id, source } if id == "c" && matches!(**source, crate::Error::InvalidQuestion { .. })),
            "{err}"
        );
        let text = "jud: 1\nkind: rubric\nid: r\nquestions:\n  n:\n    type: noul\n";
        let err = Rubric::parse(text).unwrap_err();
        assert!(
            matches!(&err, Error::Question { id, .. } if id == "n"),
            "{err}"
        );
        let text = "jud: 1\nkind: rubric\nid: r\nquestions: {}\n";
        let err = Rubric::parse(text).unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "questions"),
            "{err}"
        );
        let text = "jud: 1\nkind: rubric\nid: r\nquestions:\n  n:\n    type: noul\n    criteria: {true: a, maybe: b}\n";
        let err = Rubric::parse(text).unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "questions.n.criteria"),
            "{err}"
        );
        let text = "jud: 1\nkind: rubric\nid: r\nquestions:\n  n:\n    type: noul\n    instructions: ok?\n    extra: 1\n";
        let err = Rubric::parse(text).unwrap_err();
        assert!(
            matches!(err, Error::Syntax(_)),
            "an unknown field is refused: {err}"
        );
    }

    #[test]
    fn a_gate_must_fit_its_question() {
        let base = "jud: 1\nkind: rubric\nid: r\nquestions:\n  n:\n    type: noul\n    instructions: ok?\n  c:\n    type: choice\n    instructions: pick\n    criteria: {a: A, b: B}\n  s:\n    type: score\n    instructions: rate\n    criteria: [low, high]\n";
        let cases = [
            ("policy:\n  n: {confidence: 0.5}\n", "n", "threshold` alone"),
            ("policy:\n  c: {threshold: 0.5}\n", "c", "not `threshold`"),
            (
                "policy:\n  c: {fallback: z}\n",
                "c",
                "not one of the offered",
            ),
            ("policy:\n  s: {fallback: medium}\n", "s", "neither a level"),
            ("policy:\n  n: {threshold: 1.5}\n", "n", "between 0 and 1"),
            ("policy:\n  x: {threshold: 0.5}\n", "x", "no question"),
        ];
        for (policy, id, needle) in cases {
            let err = Rubric::parse(&format!("{base}{policy}")).unwrap_err();
            assert!(
                matches!(&err, Error::Policy { id: got, reason } if got == id && reason.contains(needle)),
                "{policy}: {err}"
            );
        }
        // A level index as a string, and a level's text, are both fallbacks.
        Rubric::parse(&format!("{base}policy:\n  s: {{fallback: '1'}}\n")).unwrap();
        Rubric::parse(&format!("{base}policy:\n  s: {{fallback: high}}\n")).unwrap();
        let err = Rubric::parse(&format!("{base}policy:\n  n: {{treshold: 0.5}}\n")).unwrap_err();
        assert!(
            matches!(err, Error::Syntax(_)),
            "a misspelt gate field is refused: {err}"
        );
    }

    #[test]
    fn the_fingerprint_is_the_questions_alone() {
        let rubric = Rubric::parse(RUBRIC).unwrap();
        let mut other = rubric.clone();
        other.id = "renamed".to_owned();
        other.policy = Policy::default();
        other.description = None;
        assert_eq!(rubric.fingerprint(), other.fingerprint());
        assert!(rubric.fingerprint().starts_with("sha256:"));
        let mut questions = Questions::new();
        questions.noul("actionable", "other", None).unwrap();
        assert_ne!(
            Rubric::new("triage", questions).fingerprint(),
            rubric.fingerprint()
        );
    }

    #[test]
    fn yaml_round_trips() {
        let rubric = Rubric::parse(RUBRIC).unwrap();
        let yaml = rubric.to_yaml().unwrap();
        let again = Rubric::parse(&yaml).unwrap();
        assert_eq!(again, rubric);
        assert_eq!(again.fingerprint(), rubric.fingerprint());
    }

    #[test]
    fn a_rubric_built_in_code_writes_out_and_reads_back() {
        let mut questions = Questions::new();
        questions
            .noul("n", "ok?", Some(NoulCriteria::new("yes means", "no means")))
            .unwrap();
        questions
            .dynamic_choice(
                "c",
                (),
                [
                    ("a".to_owned(), Some("A".to_owned())),
                    ("b".to_owned(), None),
                ],
            )
            .unwrap();
        questions
            .score("s", "rate", [json!({"what": "low"}), json!("high")])
            .unwrap();
        let mut rubric = Rubric::new("built", questions);
        rubric
            .gate(
                "n",
                Gate {
                    threshold: Some(0.7),
                    ..Gate::default()
                },
            )
            .unwrap();
        let err = rubric
            .gate(
                "c",
                Gate {
                    fallback: Some("z".to_owned()),
                    ..Gate::default()
                },
            )
            .unwrap_err();
        assert!(matches!(err, Error::Policy { .. }), "{err}");
        let yaml = rubric.to_yaml().unwrap();
        let again = Rubric::parse(&yaml).unwrap();
        assert_eq!(again, rubric);
        // Null instructions and null descriptions survive the trip.
        let wire = serde_json::to_value(&again.questions).unwrap();
        assert_eq!(wire["c"]["instructions"], Value::Null);
        assert_eq!(wire["c"]["criteria"]["b"], Value::Null);
        assert_eq!(wire["s"]["criteria"][0], json!({"what": "low"}));
    }

    #[tokio::test]
    async fn apply_reads_a_response_through_the_gates() {
        let rubric = Rubric::parse(RUBRIC).unwrap();
        let fake = Fake::new()
            .noul("actionable", 0.7)
            .unwrap()
            .choice(
                "owner",
                [("billing", 0.3), ("support", 0.5), ("none_of_these", 0.2)],
                0.25,
            )
            .unwrap()
            .score("tone", [0.1, 0.2, 0.7], 0.6)
            .unwrap();
        let response = fake
            .answer(&json!({"message": "x"}), "jev-latest", &rubric.questions)
            .await
            .unwrap();
        let verdicts = rubric.apply(&response).unwrap();
        let ids: Vec<&String> = verdicts.keys().collect();
        assert_eq!(ids, ["actionable", "owner", "tone"]);
        assert!(
            matches!(verdicts["actionable"], Verdict::Yes { probability } if probability.value() == 0.7)
        );
        // Confidence 0.25 is under the 0.4 bar: deferred to the fallback.
        assert!(
            matches!(&verdicts["owner"], Verdict::Deferred(Deferred { fallback: Some(f), nearest, bar, .. })
                if f == "none_of_these" && nearest == "support" && *bar == 0.4),
            "{:?}",
            verdicts["owner"]
        );
        assert!(
            matches!(&verdicts["tone"], Verdict::Level { index: 2, label, .. } if label == "angry"),
            "{:?}",
            verdicts["tone"]
        );
        // Below the threshold: no.
        let fake = Fake::new()
            .noul("actionable", 0.6)
            .unwrap()
            .choice(
                "owner",
                [("billing", 0.9), ("support", 0.05), ("none_of_these", 0.05)],
                0.85,
            )
            .unwrap()
            .score("tone", [0.5, 0.3, 0.2], 0.1)
            .unwrap();
        let response = fake
            .answer(&json!({"message": "x"}), "jev-latest", &rubric.questions)
            .await
            .unwrap();
        let verdicts = rubric.apply(&response).unwrap();
        assert!(matches!(verdicts["actionable"], Verdict::No { .. }));
        assert!(matches!(&verdicts["owner"], Verdict::Option { key, .. } if key == "billing"));
        // Weighted position 0.7 rounds to level 1: the nearest level, as
        // `Score::nearest_level` reads it, not the most probable one.
        assert!(
            matches!(&verdicts["tone"], Verdict::Deferred(Deferred { fallback: None, nearest, .. }) if nearest == "1"),
            "{:?}",
            verdicts["tone"]
        );
    }

    #[tokio::test]
    async fn apply_refuses_a_response_to_other_questions() {
        let rubric = Rubric::parse(RUBRIC).unwrap();
        let fake = Fake::new().noul("actionable", 0.7).unwrap();
        let mut only = Questions::new();
        only.noul("actionable", "ok?", None).unwrap();
        let response = fake.answer(&json!({}), "m", &only).await.unwrap();
        let err = rubric.apply(&response).unwrap_err();
        assert!(
            matches!(&err, Error::Response { source } if matches!(**source, crate::Error::MissingAnswer { .. })),
            "{err}"
        );
    }
}
