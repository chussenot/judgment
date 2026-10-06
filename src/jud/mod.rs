//! The `.jud` document format: a rubric with its policy, the cases it is
//! graded on and the recordings of what a model answered, as YAML or JSON
//! any tool can read, write and name by content. The specification is
//! `docs/jud.md`; this module reads and writes one version of it,
//! [`API_VERSION`]: [`parse`] for any kind, [`Rubric`], [`Cases`], and [`parse_recording`]
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

/// The one `apiVersion` this crate reads and writes. A document of any
/// other version is refused by name: the format has no compatibility
/// between versions, and a change takes a new apiVersion.
pub const API_VERSION: &str = "jud/v1.3";

/// The three kinds, as the `kind` field spells them.
pub const KINDS: [&str; 3] = ["Rubric", "Cases", "Recording"];

/// The file extension of a document of any kind.
pub const EXTENSION: &str = "jud";

/// One document of any kind, as [`parse`] returns it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
#[allow(
    clippy::large_enum_variant,
    reason = "one value per file read; a rubric is the kind with the most to hold"
)]
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
    /// An `apiVersion` this crate does not read.
    #[error(
        "`apiVersion: {found}` is not a version this crate reads; it reads `apiVersion: jud/v1.3`"
    )]
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
    let envelope: Envelope = from_text(text)?;
    envelope.check()?;
    match envelope.kind.as_deref() {
        Some("Rubric") => Rubric::parse(text).map(Document::Rubric),
        Some("Cases") => Cases::parse(text).map(Document::Cases),
        Some("Recording") => parse_recording(text).map(Document::Recording),
        _ => unreachable!("checked by Envelope::check"),
    }
}

/// The two keys read before anything else, so a document of another
/// version or kind is refused by name and never half-read.
#[derive(Deserialize)]
pub(crate) struct Envelope {
    #[serde(default, rename = "apiVersion")]
    pub(crate) api_version: Option<Value>,
    #[serde(default)]
    pub(crate) kind: Option<String>,
}

impl Envelope {
    pub(crate) fn check(&self) -> Result<()> {
        match &self.api_version {
            None => {
                return Err(Error::Missing {
                    field: "apiVersion",
                });
            }
            Some(Value::String(s)) if s == API_VERSION => {}
            Some(other) => {
                return Err(Error::Version {
                    found: other.to_string(),
                });
            }
        }
        match self.kind.as_deref() {
            None => Err(Error::Missing { field: "kind" }),
            Some(kind) if KINDS.contains(&kind) => Ok(()),
            Some(other) => Err(Error::Kind {
                found: other.to_owned(),
            }),
        }
    }

    /// Refuse a kind other than `expected`, after the envelope's own checks.
    pub(crate) fn expect_kind(&self, expected: &str) -> Result<()> {
        self.check()?;
        match self.kind.as_deref() {
            Some(kind) if kind == expected => Ok(()),
            Some(other) => Err(Error::Kind {
                found: other.to_owned(),
            }),
            None => Err(Error::Missing { field: "kind" }),
        }
    }
}

/// The `metadata` of any kind: a name, and what a tool keeps beside the
/// format. Which optional fields a kind takes is checked by its reader.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Metadata {
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub(crate) labels: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub(crate) annotations: IndexMap<String, String>,
}

impl Metadata {
    /// A metadata field a kind does not take is refused by name.
    pub(crate) fn refuse(&self, field: &str, kind: &str) -> Result<()> {
        let present = match field {
            "version" => self.version.is_some(),
            "description" => self.description.is_some(),
            _ => false,
        };
        if present {
            return Err(Error::Invalid {
                field: format!("metadata.{field}"),
                reason: format!("not a metadata field of a {kind}"),
            });
        }
        Ok(())
    }
}

/// A top-level key beyond the four of the envelope is refused with its name;
/// what a tool wants to keep goes under `metadata.annotations`.
pub(crate) fn refuse_extra(rest: &IndexMap<String, Value>) -> Result<()> {
    if let Some(key) = rest.keys().next() {
        return Err(Error::Invalid {
            field: key.clone(),
            reason: "not a field of a document: apiVersion, kind, metadata and spec; a tool keeps its own data under `metadata.annotations`"
                .to_owned(),
        });
    }
    Ok(())
}

/// Parse a `Recording` document: `metadata.name` is the case it answers, the
/// fields of [`Recording`] sit under `spec`.
pub fn parse_recording(text: &str) -> Result<Recording> {
    #[derive(Deserialize)]
    struct Doc {
        #[serde(rename = "apiVersion")]
        _api_version: String,
        #[serde(rename = "kind")]
        _kind: String,
        metadata: Metadata,
        spec: serde_json::Map<String, Value>,
        #[serde(flatten)]
        rest: IndexMap<String, Value>,
    }
    from_text::<Envelope>(text)?.expect_kind("Recording")?;
    let doc: Doc = from_text(text)?;
    refuse_extra(&doc.rest)?;
    doc.metadata.refuse("version", "Recording")?;
    doc.metadata.refuse("description", "Recording")?;
    check_name("metadata.name", &doc.metadata.name)?;
    let mut spec = doc.spec;
    if spec.contains_key("case") {
        return Err(Error::Invalid {
            field: "spec.case".to_owned(),
            reason: "the case a recording answers is `metadata.name`".to_owned(),
        });
    }
    spec.insert("case".to_owned(), Value::String(doc.metadata.name));
    serde_json::from_value(Value::Object(spec)).map_err(|e| Error::Invalid {
        field: "spec".to_owned(),
        reason: e.to_string(),
    })
}

/// A recording as a `.jud` document, YAML.
pub fn recording_to_yaml(recording: &Recording) -> Result<String> {
    #[derive(Serialize)]
    struct Doc<'a> {
        #[serde(rename = "apiVersion")]
        api_version: &'static str,
        kind: &'static str,
        metadata: Metadata,
        spec: &'a Value,
    }
    let mut spec = serde_json::to_value(recording).map_err(|e| Error::Syntax(e.to_string()))?;
    if let Value::Object(map) = &mut spec {
        map.remove("case");
    }
    to_yaml(&Doc {
        api_version: API_VERSION,
        kind: "Recording",
        metadata: Metadata {
            name: recording.case.clone(),
            ..Metadata::default()
        },
        spec: &spec,
    })
}

/// YAML 1.2 core schema, as the reading rules require (`docs/jud.md`,
/// Reading rules): an option called `yes` is a string, a duplicate key is an
/// error, and what a reviewer cannot see is refused: a merge key would fold
/// another mapping's fields in, a tag the schema does not know is refused,
/// and a `!!binary` scalar is its text, never decoded into other text. An
/// error names its line and column and quotes nothing, because a document's
/// state can be someone's data and the message lands in a log.
pub(crate) fn from_text<T: DeserializeOwned>(text: &str) -> Result<T> {
    let mut options = serde_saphyr::Options::default();
    options.strict_booleans = true;
    options.merge_keys = serde_saphyr::MergeKeyPolicy::Error;
    options.reject_unsupported_tags = true;
    options.ignore_binary_tag_for_string = true;
    options.with_snippet = false;
    serde_saphyr::from_str_with_options(text, options).map_err(|e| Error::Syntax(e.to_string()))
}

/// Refuse an id that is not a name (`docs/jud.md`, Names): every
/// `metadata.name`, a case's `id`. A name can name a file and a reference;
/// a path or a blank cannot.
pub(crate) fn check_name(field: &str, name: &str) -> Result<()> {
    if crate::eval::is_name(name) {
        return Ok(());
    }
    Err(Error::Invalid {
        field: field.to_owned(),
        reason: format!(
            "{:?} is not a name: letters, digits, `.`, `_` and `-`, starting with a letter or a digit",
            crate::answer::sanitize_server_str(name)
        ),
    })
}

pub(crate) fn to_yaml<T: Serialize>(value: &T) -> Result<String> {
    serde_saphyr::to_string(value).map_err(|e| Error::Syntax(e.to_string()))
}

/// A field that counts by its presence: `Some` whatever the value, `None`
/// when absent (with `#[serde(default)]`), `null` refused. Read via a JSON
/// value, because a YAML reader may otherwise take `null` for an empty map.
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

    const RUBRIC: &str = "apiVersion: jud/v1.3\nkind: Rubric\nmetadata: {name: r}\nspec:\n  questions:\n    q:\n      type: noul\n      instructions: ok?\n";

    #[test]
    fn the_kind_picks_the_document() {
        assert!(matches!(parse(RUBRIC).unwrap(), Document::Rubric(_)));
        let cases = "apiVersion: jud/v1.3\nkind: Cases\nmetadata: {name: c}\nspec:\n  cases:\n    - state: s\n      expect: {q: true}\n";
        assert!(matches!(parse(cases).unwrap(), Document::Cases(_)));
    }

    /// The envelope is checked before anything else: a document of another
    /// version or kind is refused by name, never half-read.
    #[test]
    fn the_envelope_is_checked_first() {
        let err = parse("kind: Rubric\nmetadata: {name: r}\nspec: {questions: {}}\n").unwrap_err();
        assert!(
            matches!(
                err,
                Error::Missing {
                    field: "apiVersion"
                }
            ),
            "{err}"
        );
        for other in ["jud/v1.2", "jud/v1.4", "jud/v2", "v1.3", "1.3"] {
            let err = parse(&format!("apiVersion: {other}\nkind: Rubric\n")).unwrap_err();
            assert!(matches!(err, Error::Version { .. }), "{other}: {err}");
        }
        let err = parse("apiVersion: 1.3\nkind: Rubric\n").unwrap_err();
        assert!(
            matches!(err, Error::Version { .. }),
            "a number is not a version: {err}"
        );
        let err = parse("jud: 1.2\nkind: rubric\n").unwrap_err();
        assert!(
            matches!(
                err,
                Error::Missing {
                    field: "apiVersion"
                }
            ),
            "the old envelope is refused by name: {err}"
        );
        for kind in ["rubric", "verdicts", "RUBRIC"] {
            let err = parse(&format!("apiVersion: jud/v1.3\nkind: {kind}\n")).unwrap_err();
            assert!(matches!(err, Error::Kind { .. }), "{kind}: {err}");
        }
        let err = parse("apiVersion: jud/v1.3\n").unwrap_err();
        assert!(matches!(err, Error::Missing { field: "kind" }), "{err}");
        let err = parse("apiVersion: [\n").unwrap_err();
        assert!(matches!(err, Error::Syntax(_)), "{err}");
    }

    /// A kind's reader refuses the other kinds by name too.
    #[test]
    fn a_kind_reader_refuses_another_kind() {
        let err = Cases::parse(RUBRIC).unwrap_err();
        assert!(
            matches!(&err, Error::Kind { found } if found == "Rubric"),
            "{err}"
        );
        let err = parse_recording(RUBRIC).unwrap_err();
        assert!(
            matches!(&err, Error::Kind { found } if found == "Rubric"),
            "{err}"
        );
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

    /// What a tool keeps beside the format goes under `metadata.annotations`
    /// or `metadata.labels`; any other top-level key, and any metadata field
    /// the kind does not take, is refused by name.
    #[test]
    fn metadata_holds_what_the_format_does_not_name() {
        let spec = "spec:\n  response: {model: m, answers: {q: {type: noul, noul: 0.5}}}\n  elapsed_ms: 1\n";
        let base = "apiVersion: jud/v1.3\nkind: Recording\n";
        let ok = format!(
            "{base}metadata:\n  name: c\n  labels: {{team: support}}\n  annotations: {{run: \"3\"}}\n{spec}"
        );
        assert_eq!(parse_recording(&ok).unwrap().case, "c");
        let err = parse_recording(&format!("{base}x-tool: 3\nmetadata: {{name: c}}\n{spec}"))
            .unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "x-tool"),
            "{err}"
        );
        let err = parse_recording(&format!(
            "{base}metadata: {{name: c, description: d}}\n{spec}"
        ))
        .unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "metadata.description"),
            "{err}"
        );
        let err = parse_recording(&format!("{base}metadata: {{name: c, comment: 3}}\n{spec}"))
            .unwrap_err();
        assert!(
            matches!(err, Error::Syntax(_)),
            "an unknown metadata field: {err}"
        );
        let err = parse_recording(&format!("{base}metadata: {{name: c}}\nspec:\n  case: c\n  response: {{model: m, answers: {{}}}}\n  elapsed_ms: 1\n")).unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "spec.case"),
            "{err}"
        );
        let err = parse_recording(&format!("{base}metadata: {{name: ../c}}\n{spec}")).unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "metadata.name"),
            "{err}"
        );
    }

    #[test]
    fn json_is_read_as_a_document_too() {
        let text = r#"{"apiVersion": "jud/v1.3", "kind": "Rubric", "metadata": {"name": "r"}, "spec": {"questions": {"q": {"type": "noul", "instructions": "ok?"}}}}"#;
        assert!(matches!(parse(text).unwrap(), Document::Rubric(_)));
    }
}
