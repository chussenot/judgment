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
//! Three kinds of document share one envelope, `jud` (the format's version,
//! `1` or `1.1`) and `kind`:
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
//! Version 1.1 adds, without changing what a `jud: 1` document means:
//! declarations that make a rubric's request depend on the state (`when`,
//! `part_when`, `options_from: request`, and a case's `options`), gates
//! with bands, a level threshold and strict comparison, and top-level `x-`
//! keys every reader ignores. A document that uses one says `jud: 1.1`; a
//! writer declares `1.1` only when it has to, so a `jud: 1` reader reads
//! every document it can.
//!
//! This module is behind the `jud` feature, off by default: a client that
//! builds its questions in code has no use for a YAML parser.

use indexmap::IndexMap;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::eval::Recording;

mod cases;
mod rubric;

pub use cases::{Case, Cases, Expect, FromTurn, Turn, grade, turns};
pub use rubric::{
    Band, Deferred, Gate, LevelRef, OptionsFrom, Policy, Rubric, RubricQuestion, Supplied, Tuning,
    Verdict,
};

/// The format's major version: the `1` of `jud: 1` and `jud: 1.1`. A
/// document of another major version is refused.
pub const VERSION: u64 = 1;

/// The highest minor version this crate reads and writes: `jud: 1.1`. A
/// minor version only adds, so this crate reads every `jud: 1` document as
/// it always meant.
pub const MINOR: u64 = 1;

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
    #[error("`jud: {found}` is not a version this crate reads; it reads `jud: 1` and `jud: 1.1`")]
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
/// envelope. A field [`Recording`] does not define is refused, except a
/// top-level `x-` key (1.1), which is ignored.
pub fn parse_recording(text: &str) -> Result<Recording> {
    #[derive(serde::Deserialize)]
    struct Doc {
        #[serde(default)]
        jud: Option<Value>,
        kind: String,
        #[serde(flatten)]
        recording: Recording,
        #[serde(flatten)]
        rest: IndexMap<String, Value>,
    }
    let doc: Doc = from_text(text)?;
    let declared = check_version(doc.jud.as_ref())?;
    if doc.kind != "recording" {
        return Err(Error::Kind { found: doc.kind });
    }
    let extensions = extensions(doc.rest, "recording")?;
    require_minor(declared, extensions.keys().next().cloned())?;
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

/// The minor version a document declares: 0 for `jud: 1` (or `1.0`), 1
/// for `jud: 1.1`. A string, another number or no `jud` is refused.
pub(crate) fn check_version(found: Option<&Value>) -> Result<u64> {
    let Some(value) = found else {
        return Err(Error::Missing { field: "jud" });
    };
    if value.as_u64() == Some(VERSION) {
        return Ok(0);
    }
    // Number literals parse to the same `f64` whichever document spelt
    // them, so comparing the parsed values is exact.
    match value.as_f64() {
        Some(1.0) => Ok(0),
        Some(1.1) => Ok(1),
        _ => Err(Error::Version {
            found: value.to_string(),
        }),
    }
}

/// The `jud` value a writer puts on a document that needs `minor`.
pub(crate) fn version_value(minor: u64) -> Value {
    if minor == 0 {
        Value::from(VERSION)
    } else {
        Value::from(1.1)
    }
}

/// A document's top-level keys beyond its kind's fields: every `x-` key is
/// an extension, any other key is a field the kind does not define.
pub(crate) fn extensions(
    rest: IndexMap<String, Value>,
    kind: &str,
) -> Result<IndexMap<String, Value>> {
    if let Some(key) = rest.keys().find(|k| !k.starts_with("x-")) {
        return Err(Error::Invalid {
            field: key.clone(),
            reason: format!("not a field of a {kind}; a document may add only `x-` keys (jud 1.1)"),
        });
    }
    Ok(rest)
}

/// Refuse a document that uses a 1.1 feature, named by its field path,
/// without declaring `jud: 1.1`: a `jud: 1` reader would refuse it, and the
/// declared version is how a reader knows before it tries.
pub(crate) fn require_minor(declared: u64, feature: Option<String>) -> Result<()> {
    match feature {
        Some(field) if declared < 1 => Err(Error::Invalid {
            field,
            reason: "is a jud 1.1 feature; the document says `jud: 1`, so declare `jud: 1.1`"
                .to_owned(),
        }),
        _ => Ok(()),
    }
}

/// Whether `path` is present in `state`: what a rubric's `when` and
/// `part_when` test (1.1). A path is dot-separated object keys, with a
/// number indexing an array (`alert.related_alerts`, `turns.0.text`). It is
/// present when it leads to a value that is not `null`, not an empty
/// string, not an empty array and not an empty object; `false` and `0` are
/// present. There is no other test: deterministic logic beyond presence
/// belongs to the caller, which can put what it decided into the state.
pub fn present(state: &Value, path: &str) -> bool {
    let mut value = state;
    for segment in path.split('.') {
        let next = match value {
            Value::Object(map) => map.get(segment),
            Value::Array(items) => segment.parse::<usize>().ok().and_then(|i| items.get(i)),
            _ => None,
        };
        match next {
            Some(v) => value = v,
            None => return false,
        }
    }
    match value {
        Value::Null => false,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

/// A state path is non-empty dot-separated segments, none empty and none
/// with surrounding space.
pub(crate) fn check_path(field: &str, path: &str) -> Result<()> {
    if path.is_empty() || path.split('.').any(|s| s.is_empty() || s.trim() != s) {
        return Err(Error::Invalid {
            field: field.to_owned(),
            reason: format!(
                "`{path}` is not a state path: dot-separated keys, as `alert.component`"
            ),
        });
    }
    Ok(())
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
        let err = parse("jud: 1.2\nkind: rubric\n").unwrap_err();
        assert!(matches!(err, Error::Version { .. }), "{err}");
        let err = parse("jud: \"1\"\nkind: rubric\n").unwrap_err();
        assert!(
            matches!(err, Error::Version { .. }),
            "a string is not a version: {err}"
        );
        let err = parse("jud: 1\nkind: verdicts\n").unwrap_err();
        assert!(matches!(err, Error::Kind { .. }), "{err}");
        let err = parse("jud: 1\n").unwrap_err();
        assert!(matches!(err, Error::Missing { field: "kind" }), "{err}");
        let err = parse("jud: [\n").unwrap_err();
        assert!(matches!(err, Error::Syntax(_)), "{err}");
    }

    #[test]
    fn both_versions_are_read() {
        assert_eq!(check_version(Some(&Value::from(1))).unwrap(), 0);
        assert_eq!(check_version(Some(&Value::from(1.0))).unwrap(), 0);
        assert_eq!(check_version(Some(&Value::from(1.1))).unwrap(), 1);
        assert_eq!(version_value(0), Value::from(1));
        assert_eq!(version_value(1), Value::from(1.1));
    }

    #[test]
    fn presence_is_the_one_test_on_the_state() {
        let state = serde_json::json!({
            "alert": {
                "component": {"name": "api"},
                "related": [],
                "title": "",
                "flag": false,
                "count": 0,
                "none": null,
                "turns": [{"text": "hi"}]
            }
        });
        for (path, expected) in [
            ("alert.component", true),
            ("alert.component.name", true),
            ("alert.related", false),
            ("alert.title", false),
            ("alert.flag", true),
            ("alert.count", true),
            ("alert.none", false),
            ("alert.missing", false),
            ("alert.turns.0.text", true),
            ("alert.turns.1", false),
            ("alert.component.name.deeper", false),
        ] {
            assert_eq!(present(&state, path), expected, "{path}");
        }
        assert!(check_path("f", "alert.component").is_ok());
        for bad in ["", "alert.", ".alert", "alert..x", "alert. x"] {
            assert!(check_path("f", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn extension_keys_are_1_1_and_other_keys_are_refused() {
        let base = "kind: recording\ncase: c\nresponse: {model: m, answers: {q: {type: noul, noul: 0.5}}}\nelapsed_ms: 1\n";
        assert!(parse_recording(&format!("jud: 1.1\nx-tool: {{run: 3}}\n{base}")).is_ok());
        let err = parse_recording(&format!("jud: 1\nx-tool: 3\n{base}")).unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "x-tool"),
            "{err}"
        );
        let err = parse_recording(&format!("jud: 1.1\ncomment: 3\n{base}")).unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "comment"),
            "{err}"
        );
    }

    #[test]
    fn json_is_read_as_a_document_too() {
        let text = r#"{"jud": 1, "kind": "rubric", "id": "r", "questions": {"q": {"type": "noul", "instructions": "ok?"}}}"#;
        assert!(matches!(parse(text).unwrap(), Document::Rubric(_)));
    }
}
