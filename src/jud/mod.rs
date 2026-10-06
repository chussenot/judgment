//! The `.jud` document format: a rubric with its policy, the cases it is
//! graded on and the recordings of what a model answered, as YAML or JSON
//! any tool can read, write and name by content. The specification is
//! `docs/jud.md`; this module implements versions `1` and `1.1` of it:
//! [`parse`] for any kind, [`Rubric`], [`Cases`], and [`parse_recording`]
//! and [`recording_to_yaml`] for a [`crate::eval::Recording`]. Fingerprints
//! are [`crate::eval::canonical`].
//!
//! Behind the `jud` feature, off by default: a client that builds its
//! questions in code has no use for a YAML parser.

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::eval::Recording;

mod cases;
mod rubric;

pub use cases::{Case, Cases, Expect, FromTurn, Turn, grade, turns};
pub use rubric::{
    Band, Deferred, Gate, LevelRef, OptionsFrom, Policy, Rubric, RubricQuestion, Supplied, Tuning,
    Verdict,
};

/// The format's major version, the `1` of `jud: 1` and `jud: 1.1`; another
/// major version is refused.
pub const VERSION: u64 = 1;

/// The highest minor version this crate reads and writes: `jud: 1.1`.
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
    /// Not YAML, or not the kind's shape; the message names the line.
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
    /// A question the builder refused, as it would one built in code.
    #[error("question `{id}`: {source}")]
    Question {
        /// The question id.
        id: String,
        /// The builder's reason, boxed because [`crate::Error`] is large.
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
    /// What [`crate::Response::verify`] found wrong with a response.
    #[error("the response does not answer the rubric: {source}")]
    Response {
        /// The verification failure, boxed as [`Error::Question`]'s source is.
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

/// Parse a `recording` document: the fields of [`Recording`] under the
/// envelope.
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

/// YAML 1.2 core schema, as the reading rules require: an option called
/// `yes` is a string, and a duplicate key is an error, not an overwrite.
pub(crate) fn from_text<T: DeserializeOwned>(text: &str) -> Result<T> {
    let mut options = serde_saphyr::Options::default();
    options.strict_booleans = true;
    serde_saphyr::from_str_with_options(text, options).map_err(|e| Error::Syntax(e.to_string()))
}

pub(crate) fn to_yaml<T: Serialize>(value: &T) -> Result<String> {
    serde_saphyr::to_string(value).map_err(|e| Error::Syntax(e.to_string()))
}

/// The minor version declared: 0 for `jud: 1` or `1.0`, 1 for `jud: 1.1`.
pub(crate) fn check_version(found: Option<&Value>) -> Result<u64> {
    let Some(value) = found else {
        return Err(Error::Missing { field: "jud" });
    };
    if value.as_u64() == Some(VERSION) {
        return Ok(0);
    }
    // Every spelling of the literal parses to one `f64`, so this is exact.
    match value.as_f64() {
        Some(1.0) => Ok(0),
        Some(1.1) if MINOR >= 1 => Ok(1),
        _ => Err(Error::Version {
            found: value.to_string(),
        }),
    }
}

/// For a 1.1 field, which counts by its presence: `Some` whatever the value,
/// `None` when absent (with `#[serde(default)]`), `null` refused. Read via a
/// JSON value, because a YAML reader may otherwise take `null` for an empty map.
pub(crate) fn some<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    if value.is_null() {
        return Err(serde::de::Error::custom(
            "null is not a value of this field; leave the field out instead",
        ));
    }
    T::deserialize(value)
        .map(Some)
        .map_err(serde::de::Error::custom)
}

/// The `jud` value a writer puts on a document that needs `minor`.
pub(crate) fn version_value(minor: u64) -> Value {
    if minor == 0 {
        Value::from(VERSION)
    } else {
        Value::from(1.1)
    }
}

/// The top-level keys beyond the kind's fields: `x-` or refused.
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

/// Refuse a 1.1 feature (a field path) under `jud: 1`: the declared version
/// is how a reader knows before it tries.
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

/// Whether `path` is present in `state`: the one test `when` and `part_when`
/// make (`docs/jud.md`, State paths). Dot-separated keys, a canonical decimal
/// indexing an array (`turns.0.text`); present is not `null` and not an empty
/// string, array or object, so `false` and `0` are present.
pub fn present(state: &Value, path: &str) -> bool {
    let mut value = state;
    for segment in path.split('.') {
        let next = match value {
            Value::Object(map) => map.get(segment),
            Value::Array(items) => array_index(segment).and_then(|i| items.get(i)),
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

/// A canonical decimal, as a JSON Pointer's: `01` and `+0` index nothing.
fn array_index(segment: &str) -> Option<usize> {
    let canonical = segment == "0"
        || (segment.starts_with(|c: char| ('1'..='9').contains(&c))
            && segment.bytes().all(|b| b.is_ascii_digit()));
    if canonical {
        segment.parse().ok()
    } else {
        None
    }
}

/// A well-formed state path: non-empty segments, none padded with space.
pub(crate) fn check_path(field: &str, path: &str) -> Result<()> {
    if path.is_empty() || path.split('.').any(|s| s.is_empty() || s.trim() != s) {
        return Err(Error::Invalid {
            field: field.to_owned(),
            reason: format!(
                "`{path}` is not a state path: dot-separated keys, as `customer.account`"
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
            "customer": {
                "account": {"plan": "pro"},
                "open_tickets": [],
                "name": "",
                "vip": false,
                "orders": 0,
                "none": null,
                "history": [{"text": "hi"}, {"text": "again"}]
            }
        });
        for (path, expected) in [
            ("customer.account", true),
            ("customer.account.plan", true),
            ("customer.open_tickets", false),
            ("customer.name", false),
            ("customer.vip", true),
            ("customer.orders", true),
            ("customer.none", false),
            ("customer.missing", false),
            ("customer.history.0.text", true),
            ("customer.history.1", true),
            ("customer.history.2", false),
            ("customer.history.01", false),
            ("customer.history.+1", false),
            ("customer.account.plan.deeper", false),
        ] {
            assert_eq!(present(&state, path), expected, "{path}");
        }
        assert!(check_path("f", "customer.account").is_ok());
        for bad in ["", "customer.", ".customer", "customer..x", "customer. x"] {
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
