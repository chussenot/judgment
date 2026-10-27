//! The `.jud` document format: a rubric with its policy, the cases it is
//! graded on and the recordings of what a model answered, as YAML or JSON
//! any tool can read, write and name by content. The specification is
//! `docs/reference/jud-format.md`; this module reads and writes one line of it,
//! [`API_VERSION`] up to [`MINOR`]: [`parse`] for any kind, [`Rubric`], [`Cases`], and [`parse_recording`]
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
#[cfg(feature = "schema")]
pub mod schema;

pub use cases::{BoxLabel, Case, Cases, Expect, FromTurn, Turn, grade, turns};

use std::path::Path;

use crate::asset::{Assets, Image, ImageFormat};
pub use rubric::{
    Band, Deferred, Gate, LevelRef, OptionsFrom, Policy, Rubric, RubricQuestion, Supplied, Tuning,
    Verdict,
};

/// The `apiVersion` this crate reads and writes: the stable `v1` line of
/// the format, whose minor is `metadata.minor` ([`MINOR`]; decision 0020).
/// Any other string is refused by name, but for [`EARLIER_SPELLINGS`].
pub const API_VERSION: &str = "jud/v1";

/// The highest `metadata.minor` this crate reads, and the one it writes. A
/// minor only adds (`docs/reference/stability.md`), so every lower minor
/// is read too; a document declaring a higher one needs a later release
/// and is refused by number ([`Error::Minor`]).
pub const MINOR: u32 = 4;

/// How a minor was spelled before `metadata.minor` existed, with the
/// minor it is: `jud/v1.3` wrote its minor into the `apiVersion` string.
/// Read as that minor, with no `metadata.minor` allowed beside it, for at
/// least twelve months from the release that first reads `jud/v1`
/// (`docs/reference/stability.md`); never written.
pub const EARLIER_SPELLINGS: [(&str, u32); 1] = [("jud/v1.3", 3)];

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
        "`apiVersion: {found}` is not a version this crate reads; it reads `apiVersion: jud/v1` with `metadata.minor`, and the earlier spelling `jud/v1.3`"
    )]
    Version {
        /// What the document said.
        found: String,
    },
    /// A `metadata.minor` above the one this crate reads: the document
    /// uses something a later minor added, and a later release reads it.
    #[error(
        "`metadata.minor: {found}` is past the minor this crate reads, {reads}; a later release of the crate reads it"
    )]
    Minor {
        /// What the document declared.
        found: u64,
        /// The highest minor this crate reads.
        reads: u32,
    },
    /// A `kind:` this crate does not know.
    #[error("`kind: {found}` is not a document kind; the kinds are Rubric, Cases and Recording")]
    Kind {
        /// What the document said.
        found: String,
    },
    /// A kind this crate knows, read where another was expected: a Cases
    /// document handed to [`Rubric::parse`], say.
    #[error("a {found} document, not a {expected}; this reads `kind: {expected}`")]
    OtherKind {
        /// What the document said.
        found: String,
        /// The kind the reader takes.
        expected: &'static str,
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

/// Where a document's image comes from (`docs/reference/jud-format.md`, Assets;
/// decision 0025): a file beside the document, or the bytes inline. The
/// map an entry sits in is keyed by the reference written where the image
/// belongs, as the wire's `assets` is.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetSource {
    /// `image`, the one kind; written always, read optionally.
    #[serde(rename = "type", default = "image_type")]
    pub kind: String,
    /// A path relative to the document's directory, never absolute and never
    /// leaving it; the format is read from the file and checked against
    /// `format` when one is written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `png`, `jpg` or `jpeg`: required with `data_base64`, optional with a path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// The file's bytes as the wire carries them, for a small image a
    /// document keeps to itself; a path is the usual choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
}

fn image_type() -> String {
    "image".to_owned()
}

impl AssetSource {
    /// An image at `path`, relative to the document.
    pub fn path(path: impl Into<String>) -> Self {
        Self {
            kind: image_type(),
            path: Some(path.into()),
            format: None,
            data_base64: None,
        }
    }

    /// An image inline, as `format` and its base64.
    pub fn inline(image: &Image) -> Self {
        Self {
            kind: image_type(),
            path: None,
            format: Some(image.format().wire_name().to_owned()),
            data_base64: Some(image.to_base64()),
        }
    }

    /// The shape checks a reader makes without touching a file: the kind is
    /// `image`, exactly one of `path` and `data_base64`, a format the wire
    /// takes, `format` beside `data_base64`, a relative path that stays in
    /// the directory. `field` names the entry in a refusal.
    pub(crate) fn check(&self, field: &str) -> Result<()> {
        let refuse = |reason: String| {
            Err(Error::Invalid {
                field: field.to_owned(),
                reason,
            })
        };
        if self.kind != "image" {
            return refuse(format!(
                "`type: {}` is not an asset kind; the one kind is `image`",
                self.kind
            ));
        }
        match (&self.path, &self.data_base64) {
            (Some(_), Some(_)) => {
                return refuse(
                    "has both `path` and `data_base64`; an image comes one way".to_owned(),
                );
            }
            (None, None) => {
                return refuse(
                    "needs `path` (relative to the document) or `data_base64`".to_owned(),
                );
            }
            (None, Some(_)) if self.format.is_none() => {
                return refuse(
                    "`format` (png, jpg or jpeg) is required with `data_base64`".to_owned(),
                );
            }
            _ => {}
        }
        if let Some(format) = &self.format
            && ImageFormat::from_wire_name(format).is_none()
        {
            return refuse(format!(
                "`format: {format}` is not one the API takes: png, jpg or jpeg"
            ));
        }
        if let Some(path) = &self.path {
            // Read as text, the same on every platform and as the schema's
            // pattern: a leading `/` or `\`, a drive (`C:`) or a `..` segment
            // on either separator. `Path` would see `\x.png` as relative on
            // Windows, where joining it leaves the directory, and `..\x.png`
            // as one name on Unix.
            let bytes = path.as_bytes();
            // A line break is refused too: the schema's `.*` stops at one.
            let leaves = path.starts_with(['/', '\\'])
                || path.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
                || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
                || path.split(['/', '\\']).any(|segment| segment == "..");
            if path.is_empty() || leaves {
                return refuse(format!(
                    "`path: {path}` must be relative to the document and stay under its directory"
                ));
            }
        }
        Ok(())
    }

    /// The image: read from `base` joined with the path, or decoded from
    /// the base64. [`Error::Invalid`] names the entry and what went wrong,
    /// the path included, so a file that moved fails loudly by name.
    pub fn load(&self, field: &str, base: &Path) -> Result<Image> {
        self.check(field)?;
        let invalid = |reason: String| Error::Invalid {
            field: field.to_owned(),
            reason,
        };
        let image = match (&self.path, &self.data_base64) {
            (Some(path), _) => {
                let full = base.join(path);
                let image = Image::read(&full)
                    .map_err(|e| invalid(format!("cannot load the image at {path}: {e}")))?;
                if let Some(format) = &self.format
                    && ImageFormat::from_wire_name(format) != Some(image.format())
                {
                    return Err(invalid(format!(
                        "{path} is a {} but the entry says `format: {format}`",
                        image.format()
                    )));
                }
                image
            }
            (None, Some(text)) => Image::from_base64(text, self.format.as_deref())
                .map_err(|e| invalid(e.to_string()))?,
            (None, None) => unreachable!("checked above"),
        };
        Ok(image)
    }
}

/// The reference and shape checks over a document's asset map, and that
/// every reference appears in a string of `referenced` (the questions' or the
/// state's value, as or inside the text of a string, never in a key or in
/// the JSON syntax): the server refuses an unreferenced asset with a 400,
/// and a reference that appears nowhere is a typo somewhere.
pub(crate) fn check_assets(
    field: &str,
    assets: &IndexMap<String, AssetSource>,
    referenced: &Value,
) -> Result<()> {
    let mut total = 0;
    for (reference, source) in assets {
        let entry = format!("{field}.{reference}");
        if reference.trim().is_empty() || reference.len() > crate::asset::MAX_REFERENCE_BYTES {
            return Err(Error::Invalid {
                field: entry,
                reason: format!(
                    "a reference is text, not empty and at most {} bytes",
                    crate::asset::MAX_REFERENCE_BYTES
                ),
            });
        }
        total += reference.len();
        source.check(&entry)?;
        if !mentions(referenced, reference) {
            return Err(Error::Invalid {
                field: entry,
                reason: "is referenced nowhere; write the reference where the image belongs, or drop the entry".to_owned(),
            });
        }
    }
    if total > crate::asset::MAX_REFERENCES_BYTES {
        return Err(Error::Invalid {
            field: field.to_owned(),
            reason: format!(
                "the references weigh {total} bytes together, over the {} the API allows",
                crate::asset::MAX_REFERENCES_BYTES
            ),
        });
    }
    Ok(())
}

/// Whether `reference` is in a string value of `value`, as the whole or a part.
fn mentions(value: &Value, reference: &str) -> bool {
    match value {
        Value::String(text) => text.contains(reference),
        Value::Array(items) => items.iter().any(|item| mentions(item, reference)),
        Value::Object(map) => map.values().any(|item| mentions(item, reference)),
        _ => false,
    }
}

/// Load a document's declared images into the request's [`Assets`], each
/// path relative to `base`, the document's directory. A reference already
/// in `into` must name the same bytes ([`Error::Invalid`] otherwise), so a
/// rubric's images and a case's merge into one request.
pub fn load_assets(
    field: &str,
    declared: &IndexMap<String, AssetSource>,
    base: &Path,
    into: &mut Assets,
) -> Result<()> {
    for (reference, source) in declared {
        let entry = format!("{field}.{reference}");
        let image = source.load(&entry, base)?;
        into.insert(reference.clone(), image)
            .map_err(|e| Error::Invalid {
                field: entry,
                reason: e.to_string(),
            })?;
    }
    Ok(())
}

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

/// The keys read before anything else, so a document of another version,
/// minor or kind is refused by name and never half-read: `apiVersion`,
/// `kind` and `metadata.minor`.
#[derive(Deserialize)]
pub(crate) struct Envelope {
    #[serde(default, rename = "apiVersion")]
    pub(crate) api_version: Option<Value>,
    #[serde(default)]
    pub(crate) kind: Option<String>,
    #[serde(default)]
    pub(crate) metadata: Option<EnvelopeMetadata>,
}

/// The one metadata field the envelope check reads; the rest is each
/// kind's reader's.
#[derive(Deserialize, Default)]
pub(crate) struct EnvelopeMetadata {
    #[serde(default)]
    pub(crate) minor: Option<Value>,
}

impl Envelope {
    pub(crate) fn check(&self) -> Result<()> {
        let minor = self.metadata.as_ref().and_then(|m| m.minor.as_ref());
        match &self.api_version {
            None => {
                return Err(Error::Missing {
                    field: "apiVersion",
                });
            }
            Some(Value::String(s)) if s == API_VERSION => check_minor(minor)?,
            Some(Value::String(s)) if EARLIER_SPELLINGS.iter().any(|(e, _)| e == s) => {
                if minor.is_some() {
                    return Err(Error::Invalid {
                        field: "metadata.minor".to_owned(),
                        reason: format!(
                            "not a field of a `{s}` document, whose minor is in its apiVersion; write `apiVersion: {API_VERSION}` to declare one"
                        ),
                    });
                }
            }
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

    /// Refuse a kind other than `expected`, after the envelope's own checks
    /// (so an unknown kind is [`Error::Kind`] and a known other kind is
    /// [`Error::OtherKind`], which names both).
    pub(crate) fn expect_kind(&self, expected: &'static str) -> Result<()> {
        self.check()?;
        match self.kind.as_deref() {
            Some(kind) if kind == expected => Ok(()),
            Some(other) => Err(Error::OtherKind {
                found: other.to_owned(),
                expected,
            }),
            None => Err(Error::Missing { field: "kind" }),
        }
    }
}

/// `metadata.minor` under `apiVersion: jud/v1`: required, an integer, at
/// most [`MINOR`]. Checked before anything else in the document.
fn check_minor(minor: Option<&Value>) -> Result<()> {
    let Some(minor) = minor else {
        return Err(Error::Missing {
            field: "metadata.minor",
        });
    };
    // Past `u64` a number reads as a float, every one of them whole: still a
    // minor above ours, refused by number (as large as the error holds).
    let found = minor.as_u64().or_else(|| {
        minor
            .as_f64()
            .filter(|f| *f >= 18_446_744_073_709_551_616.0 && f.is_finite())
            .map(|_| u64::MAX)
    });
    let Some(found) = found else {
        return Err(Error::Invalid {
            field: "metadata.minor".to_owned(),
            reason: format!(
                "{minor} is not a minor version: a non-negative integer, {MINOR} today"
            ),
        });
    };
    if found > u64::from(MINOR) {
        return Err(Error::Minor {
            found,
            reads: MINOR,
        });
    }
    Ok(())
}

/// The `metadata` of any kind: a name, the minor the document needs, and
/// what a tool keeps beside the format. Which optional fields a kind takes
/// is checked by its reader; `minor` was checked by the envelope and is
/// written as [`MINOR`].
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Metadata {
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) minor: Option<u32>,
    /// Text or a number, read as its source text (`1.10` is not `1.1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
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
    const FIELDS: [&str; 7] = [
        "response",
        "elapsed_ms",
        "request_hash",
        "fingerprint",
        "server",
        "recorded_at",
        "rubric",
    ];
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
    check_scalars(text, "Recording")?;
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
    // `Recording` ignores a key it does not know; the format refuses one, so
    // a misspelt `fingerprint` is not read as a recording with none.
    if let Some(key) = spec.keys().find(|k| !FIELDS.contains(&k.as_str())) {
        return Err(Error::Invalid {
            field: format!("spec.{key}"),
            reason: format!("not a field of a Recording: {}", FIELDS.join(", ")),
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
            minor: Some(MINOR),
            ..Metadata::default()
        },
        spec: &spec,
    })
}

/// YAML 1.2 core schema, as the reading rules require (`docs/reference/jud-format.md`,
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

/// Where `null` is a value ([`check_scalars`]).
fn null_allowed(kind: &str, path: &[&str]) -> bool {
    match (kind, path) {
        (
            "Rubric",
            ["spec", "questions", _, "instructions", ..]
            | ["spec", "questions", _, "criteria", _, ..],
        )
        | (
            "Cases",
            ["spec", "cases", _, "state", ..]
            | ["spec", "cases", _, "options", _, _, ..]
            | ["spec", "cases", _, "expect", _, "from_turn"],
        ) => true,
        ("Rubric", ["spec", "tuning", key, ..]) => {
            !matches!(*key, "cases" | "model" | "server" | "tuned_at")
        }
        ("Recording", ["spec", "response", "answers", _, key, ..]) => *key != "type",
        ("Recording", ["spec", "response", "usage", key, ..]) => {
            !matches!(*key, "input_tokens" | "output_tokens")
        }
        ("Recording", ["spec", "response", key, ..]) => {
            !matches!(*key, "model" | "answers" | "usage" | "request_id")
        }
        _ => false,
    }
}
/// The fields `schemas/jud/` types as a string and nothing else.
fn text_only(kind: &str, path: &[&str]) -> bool {
    matches!(
        path,
        ["metadata", "name" | "description"] | ["metadata", "labels" | "annotations", _]
    ) || matches!(
        (kind, path),
        (
            "Rubric",
            ["spec", "questions", _, "type" | "when" | "options_from"]
                | ["spec", "questions", _, "part_when", _]
                | ["spec", "assets", _, _]
                | ["spec", "policy", _, "fallback" | "note"]
                | ["spec", "policy", _, "bands", _, "verdict"]
                | ["spec", "tuning", "cases" | "model" | "server" | "tuned_at"],
        ) | (
            "Cases",
            ["spec", "rubric"]
                | ["spec", "cases", _, "id" | "note"]
                | ["spec", "cases", _, "tags", _]
                | ["spec", "cases", _, "assets", _, _],
        ) | (
            "Recording",
            [
                "spec",
                "fingerprint" | "request_hash" | "rubric" | "server" | "recorded_at",
            ] | ["spec", "response", "model" | "request_id"]
                | ["spec", "response", "answers", _, "type"],
        )
    )
}

/// Refuse, naming the field, what the schemas refuse and a typed read would
/// take anyway: a number or a boolean in a field that takes only text
/// (`name: 2026` would read as "2026"), and `null` wherever the format gives
/// it no meaning (`threshold: null` would read as absent, the default bar).
/// `null` is a value only of instructions, an option's or a Noul
/// criterion's description and a turn label's `from_turn`, and inside what
/// is free: a case's state, a recording's answers, and the fields a
/// recording's `response` or `usage`, or a tuning block, keeps beyond the
/// ones the format defines. Read over the document as a plain tree, where a
/// key is always text, as it is in JSON.
pub(crate) fn check_scalars(text: &str, kind: &str) -> Result<()> {
    fn walk(kind: &str, value: &Value, path: &mut Vec<String>) -> Result<()> {
        let segments = || path.iter().map(String::as_str).collect::<Vec<&str>>();
        let refuse = |reason: &str| {
            let field = path.iter().fold(String::new(), |mut field, segment| {
                if !field.is_empty() && !segment.starts_with('[') {
                    field.push('.');
                }
                field.push_str(segment);
                field
            });
            Err(Error::Invalid {
                field,
                reason: reason.to_owned(),
            })
        };
        match value {
            Value::Null if !null_allowed(kind, &segments()) => {
                refuse("null is not a value of this field; leave the field out instead")
            }
            Value::Bool(_) | Value::Number(_) if text_only(kind, &segments()) => {
                refuse("a number or a boolean where the format takes text; quote it")
            }
            // `metadata.version` takes text or a number, read as its text.
            Value::Bool(_) if segments() == ["metadata", "version"] => {
                refuse("a boolean where the format takes text or a number")
            }
            Value::Array(items) => items.iter().enumerate().try_for_each(|(i, item)| {
                path.push(format!("[{i}]"));
                let checked = walk(kind, item, path);
                path.pop();
                checked
            }),
            Value::Object(map) => map.iter().try_for_each(|(key, item)| {
                path.push(key.clone());
                let checked = walk(kind, item, path);
                path.pop();
                checked
            }),
            _ => Ok(()),
        }
    }
    walk(kind, &from_text::<Value>(text)?, &mut Vec::new())
}

/// Refuse an id that is not a name (`docs/reference/jud-format.md`, Names): every
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
/// make (`docs/reference/jud-format.md`, State paths). Dot-separated keys, a canonical decimal
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
        for other in ["jud/v1.2", "jud/v1.4", "jud/v2", "v1.3", "1.3", "jud/v1.0"] {
            let err = parse(&format!("apiVersion: {other}\nkind: Rubric\n")).unwrap_err();
            assert!(matches!(err, Error::Version { .. }), "{other}: {err}");
        }
        // `jud/v1` needs a minor, an integer at most the one this crate reads.
        let err = parse("apiVersion: jud/v1\nkind: Rubric\nmetadata: {name: r}\n").unwrap_err();
        assert!(
            matches!(
                err,
                Error::Missing {
                    field: "metadata.minor"
                }
            ),
            "{err}"
        );
        let err = parse(&format!(
            "apiVersion: jud/v1\nkind: Rubric\nmetadata: {{name: r, minor: {}}}\n",
            MINOR + 1
        ))
        .unwrap_err();
        assert!(
            matches!(err, Error::Minor { found, reads } if found == u64::from(MINOR + 1) && reads == MINOR),
            "{err}"
        );
        // Past `u64` is still a minor above ours, refused by number.
        for big in ["18446744073709551616", "99999999999999999999999"] {
            let err = parse(&format!(
                "apiVersion: jud/v1\nkind: Rubric\nmetadata: {{name: r, minor: {big}}}\n"
            ))
            .unwrap_err();
            assert!(
                matches!(err, Error::Minor { found: u64::MAX, reads } if reads == MINOR),
                "{big}: {err}"
            );
        }
        // `4.0` is a float to the reader, though JSON Schema's integer takes it.
        for bad in ["\"3\"", "3.5", "4.0", "-1", "null", "-18446744073709551616"] {
            let err = parse(&format!(
                "apiVersion: jud/v1\nkind: Rubric\nmetadata: {{name: r, minor: {bad}}}\n"
            ))
            .unwrap_err();
            assert!(
                matches!(&err, Error::Invalid { field, .. } if field == "metadata.minor")
                    || matches!(
                        err,
                        Error::Missing {
                            field: "metadata.minor"
                        }
                    ),
                "{bad}: {err}"
            );
        }
        // The earlier spelling reads as its minor, and takes no `minor`.
        parse("apiVersion: jud/v1.3\nkind: Rubric\nmetadata: {name: r}\nspec:\n  questions:\n    q: {type: noul, instructions: ok?}\n").unwrap();
        let err = parse("apiVersion: jud/v1.3\nkind: Rubric\nmetadata: {name: r, minor: 3}\n")
            .unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "metadata.minor"),
            "{err}"
        );
        // A lower minor is read: a minor only adds.
        parse("apiVersion: jud/v1\nkind: Rubric\nmetadata: {name: r, minor: 0}\nspec:\n  questions:\n    q: {type: noul, instructions: ok?}\n").unwrap();
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

    /// A kind's reader refuses the other kinds by name too, and says which
    /// kind it reads, so the message never calls a known kind unknown.
    #[test]
    fn a_kind_reader_refuses_another_kind() {
        let err = Cases::parse(RUBRIC).unwrap_err();
        assert!(
            matches!(&err, Error::OtherKind { found, expected: "Cases" } if found == "Rubric"),
            "{err}"
        );
        assert_eq!(
            err.to_string(),
            "a Rubric document, not a Cases; this reads `kind: Cases`"
        );
        let err = parse_recording(RUBRIC).unwrap_err();
        assert!(
            matches!(&err, Error::OtherKind { found, expected: "Recording" } if found == "Rubric"),
            "{err}"
        );
        // An unknown kind is still the other error, through the same path.
        let err = Rubric::parse("apiVersion: jud/v1\nkind: verdicts\nmetadata: {minor: 3}\n")
            .unwrap_err();
        assert!(
            matches!(&err, Error::Kind { found } if found == "verdicts"),
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

    /// A number where the format takes only text is refused by field, and the
    /// refusal quotes nothing; a key is text whatever YAML reads it as, as in
    /// JSON, so a number as a key is read wherever the schemas take it.
    #[test]
    fn a_number_where_only_text_goes_is_refused_and_a_number_key_is_read() {
        let rubric = |metadata: &str, extra: &str| {
            format!(
                "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\n{metadata}spec:\n  questions:\n    n: {{type: noul, instructions: ok?, when: x}}\n{extra}"
            )
        };
        for (text, field) in [
            (
                "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: 20261026\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n".to_owned(),
                "metadata.name",
            ),
            (rubric("  version: true\n", ""), "metadata.version"),
            (
                rubric("", "  policy:\n    n: {threshold: 0.5, note: 3}\n"),
                "spec.policy.n.note",
            ),
            (
                "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?, when: 5}\n".to_owned(),
                "spec.questions.n.when",
            ),
        ] {
            let err = parse(&text).unwrap_err();
            assert!(
                matches!(&err, Error::Invalid { field: f, .. } if f == field),
                "{err}"
            );
            assert!(!err.to_string().contains("20261026"), "{err}");
        }
        // A number key in labels, a state, a recording's legend; a number
        // version; a usage field the format does not define, null.
        parse(&rubric("  version: 3\n  labels: {2024: one}\n", "")).unwrap();
        parse("apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: c\nspec:\n  cases:\n    - state: {2024: revenue, true: yes}\n").unwrap();
        parse("apiVersion: jud/v1.3\nkind: Recording\nmetadata:\n  name: c\nspec:\n  elapsed_ms: 1\n  response:\n    model: m\n    answers:\n      s: {type: score, score: 0.2, confidence: 0.9, legend: {0: low, 1: high}, probabilities: {0: 0.8, 1: 0.2}}\n    usage: {input_tokens: 1, output_tokens: 2, cached_tokens: null}\n").unwrap();
    }

    /// An image's path is read as text, so a path is refused or read the same
    /// on every platform and as the schema's pattern has it.
    #[test]
    fn an_asset_path_is_checked_the_same_on_every_platform() {
        let check = |path: &str| {
            AssetSource {
                kind: "image".to_owned(),
                path: Some(path.to_owned()),
                format: None,
                data_base64: None,
            }
            .check("assets.photo")
        };
        for refused in [
            "",
            "/x.png",
            "\\x.png",
            "..\\secret.png",
            "\\\\server\\share\\logo.png",
            "C:/Windows/logo.png",
            "c:logo.png",
            "a/../../x.png",
            "a\\..\\x.png",
            "..",
            "\n/x.png",
        ] {
            assert!(check(refused).is_err(), "{refused}");
        }
        for read in [
            "photo.png",
            "images/a.png",
            "./a.png",
            "a..b.png",
            "..a.png",
        ] {
            assert!(check(read).is_ok(), "{read}");
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
        let err = parse_recording(&format!(
            "{base}metadata: {{name: c}}\n{spec}  fingreprint: sha256:00\n"
        ))
        .unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "spec.fingreprint"),
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
