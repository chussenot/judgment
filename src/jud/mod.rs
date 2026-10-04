//! The `.jud` document format: a rubric with its policy, the cases it is
//! graded on, and the recordings of what a model answered, as YAML any tool
//! can read, write and name by content.
//!
//! A System One request is simple; what is hard to keep is everything around
//! it. The questions live in code, the threshold a probability is acted on
//! at lives in a configuration file or a constant, the labelled examples the
//! threshold was tuned on live in a notebook, and the answers a model gave
//! last month live nowhere. When the model version moves, nobody can say
//! which of the four changed. The format gives each of them a file with a
//! stated shape, an identity, and a fingerprint that any implementation
//! computes the same way, so a policy says which rubric and which cases it
//! was tuned on, a recording says which request and which server it
//! answers, and two tools exchange all three without agreeing on anything
//! but the files.
//!
//! Three kinds of document share one envelope, `jud: 1` (the format's
//! version) and `kind`:
//!
//! - **`rubric`**: the questions exactly as the wire sends them, in the order
//!   the model should see them, plus `policy`: the gates (a Noul's
//!   `threshold`, a Choice's or a Score's `confidence` bar and `fallback`
//!   option) and the `tuning` they came from. The policy is never sent; it
//!   is what the application does with an answer, kept beside the question
//!   it applies to ([`Rubric`], [`Policy`]).
//! - **`cases`**: labelled states to grade a rubric on, one expectation per
//!   question the case is labelled for; a conversation is a state that is an
//!   array of turns and may be labelled by the turn a Noul becomes true
//!   ([`Cases`], [`Expect`]).
//! - **`recording`**: a model's response to one request, with the request's
//!   fingerprint, the server, the model and the time
//!   ([`crate::eval::Recording`]).
//!
//! The fingerprint of a document, a rubric's questions or a request is the
//! SHA-256 of its canonical JSON ([`crate::eval::canonical`], RFC 8785).
//! The questions of a rubric are the wire's own shape, so lowering a rubric
//! into a [`Questions`](crate::Questions) loses nothing and runs the same
//! checks the builder runs; what a `.jud` adds is what the wire has no place
//! for. YAML is the encoding because people write and review rubrics, and
//! JSON is valid YAML, so a reader of one reads the other. The specification
//! is `docs/jud.md`; the JSON Schemas under `schemas/jud/` are the formal
//! statement of each kind.
//!
//! This module is behind the `jud` feature, off by default: a client that
//! builds its questions in code has no use for a YAML parser.

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::eval::Recording;

mod cases;
mod rubric;

pub use cases::{Case, Cases, Expect, FromTurn, Turn, grade, turns};
pub use rubric::{Deferred, Gate, Policy, Rubric, Tuning, Verdict};

/// The format version this crate reads and writes: the value of `jud:`.
pub const VERSION: u64 = 1;

/// The file extension of a document of any kind.
pub const EXTENSION: &str = "jud";

/// One document of any kind, as [`parse`] returns it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Document {
    /// A rubric with its policy.
    Rubric(Rubric),
    /// Labelled cases.
    Cases(Cases),
    /// One recorded response.
    Recording(Recording),
}

/// Reading or writing a document.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The text is not YAML, or not the shape the kind requires; the
    /// message names the line.
    #[error("not a .jud document: {0}")]
    Syntax(String),
    /// A `jud:` version this crate does not read.
    #[error("`jud: {found}` is not a version this crate reads; it reads `jud: {VERSION}`")]
    Version {
        /// What the document said.
        found: String,
    },
    /// A `kind:` this crate does not know.
    #[error("`kind: {found}` is not a document kind; the kinds are rubric, cases and recording")]
    Kind {
        /// What the document said.
        found: String,
    },
    /// A required field is absent.
    #[error("the document has no `{field}`")]
    Missing {
        /// The field.
        field: &'static str,
    },
    /// A field holds something the format does not allow.
    #[error("`{field}`: {reason}")]
    Invalid {
        /// The field, as a path.
        field: String,
        /// What is wrong with it.
        reason: String,
    },
    /// A question the builder refused: the same checks a question built in
    /// code gets.
    #[error("question `{id}`: {source}")]
    Question {
        /// The question id.
        id: String,
        /// The builder's reason, boxed because the crate's error is large.
        #[source]
        source: Box<crate::Error>,
    },
    /// A gate that does not fit its question.
    #[error("policy for `{id}`: {reason}")]
    Policy {
        /// The question id.
        id: String,
        /// What is wrong.
        reason: String,
    },
    /// A response that does not answer the rubric's questions as asked:
    /// what [`crate::Response::verify`] found.
    #[error("the response does not answer the rubric: {source}")]
    Response {
        /// The verification failure, boxed because the crate's error is
        /// large.
        #[source]
        source: Box<crate::Error>,
    },
    /// A case whose state or expectation does not fit.
    #[error("case {case}: {reason}")]
    Case {
        /// The case id, or its position.
        case: String,
        /// What is wrong.
        reason: String,
    },
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Parse a document of any kind, YAML or JSON, by its `kind`.
pub fn parse(text: &str) -> Result<Document> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        #[serde(default)]
        jud: Option<serde_json::Value>,
        #[serde(default)]
        kind: Option<String>,
    }
    let envelope: Envelope = from_text(text)?;
    check_version(envelope.jud.as_ref())?;
    match envelope.kind.as_deref() {
        Some("rubric") => Rubric::parse(text).map(Document::Rubric),
        Some("cases") => Cases::parse(text).map(Document::Cases),
        Some("recording") => parse_recording(text).map(Document::Recording),
        Some(other) => Err(Error::Kind {
            found: other.to_owned(),
        }),
        None => Err(Error::Missing { field: "kind" }),
    }
}

/// Parse a recording document: the fields of [`Recording`] under the
/// envelope.
pub fn parse_recording(text: &str) -> Result<Recording> {
    #[derive(serde::Deserialize)]
    struct Doc {
        #[serde(default)]
        jud: Option<serde_json::Value>,
        kind: String,
        #[serde(flatten)]
        recording: Recording,
    }
    let doc: Doc = from_text(text)?;
    check_version(doc.jud.as_ref())?;
    if doc.kind != "recording" {
        return Err(Error::Kind { found: doc.kind });
    }
    Ok(doc.recording)
}

/// A recording as a `.jud` document, YAML.
pub fn recording_to_yaml(recording: &Recording) -> Result<String> {
    #[derive(Serialize)]
    struct Doc<'a> {
        jud: u64,
        kind: &'static str,
        #[serde(flatten)]
        recording: &'a Recording,
    }
    to_yaml(&Doc {
        jud: VERSION,
        kind: "recording",
        recording,
    })
}

/// Read YAML (or JSON) under the YAML 1.2 core schema: only `true` and
/// `false` are booleans, so a Choice option or a level called `yes`, `no`,
/// `on` or `off` is the string it reads as, and a duplicate key is an
/// error rather than a silent overwrite. Both are what another
/// implementation reading the same file with a YAML 1.2 parser does.
pub(crate) fn from_text<T: DeserializeOwned>(text: &str) -> Result<T> {
    let mut options = serde_saphyr::Options::default();
    options.strict_booleans = true;
    serde_saphyr::from_str_with_options(text, options).map_err(|e| Error::Syntax(e.to_string()))
}

pub(crate) fn to_yaml<T: Serialize>(value: &T) -> Result<String> {
    serde_saphyr::to_string(value).map_err(|e| Error::Syntax(e.to_string()))
}

pub(crate) fn check_version(found: Option<&serde_json::Value>) -> Result<()> {
    match found {
        None => Err(Error::Missing { field: "jud" }),
        Some(v) if v.as_u64() == Some(VERSION) => Ok(()),
        Some(v) => Err(Error::Version {
            found: v.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_kind_picks_the_document() {
        let rubric = "jud: 1\nkind: rubric\nid: r\nquestions:\n  q:\n    type: noul\n    instructions: ok?\n";
        assert!(matches!(parse(rubric).unwrap(), Document::Rubric(_)));
        let cases = "jud: 1\nkind: cases\ncases:\n  - state: s\n    expect: {q: true}\n";
        assert!(matches!(parse(cases).unwrap(), Document::Cases(_)));
    }

    #[test]
    fn the_envelope_is_checked_first() {
        let err = parse("kind: rubric\nid: r\nquestions: {}\n").unwrap_err();
        assert!(matches!(err, Error::Missing { field: "jud" }), "{err}");
        let err = parse("jud: 2\nkind: rubric\n").unwrap_err();
        assert!(matches!(err, Error::Version { .. }), "{err}");
        let err = parse("jud: 1\nkind: verdicts\n").unwrap_err();
        assert!(matches!(err, Error::Kind { .. }), "{err}");
        let err = parse("jud: 1\n").unwrap_err();
        assert!(matches!(err, Error::Missing { field: "kind" }), "{err}");
        let err = parse("jud: [\n").unwrap_err();
        assert!(matches!(err, Error::Syntax(_)), "{err}");
    }

    #[test]
    fn json_is_read_as_a_document_too() {
        let text = r#"{"jud": 1, "kind": "rubric", "id": "r", "questions": {"q": {"type": "noul", "instructions": "ok?"}}}"#;
        assert!(matches!(parse(text).unwrap(), Document::Rubric(_)));
    }
}
