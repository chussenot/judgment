//! Answers: the wire shapes, the validated probabilities and the typed
//! views that [`Response::get`] reads through a [`crate::Handle`].
//!
//! # Two newtypes, not one `f64`
//!
//! A [`Probability`] is the model's estimate for one outcome; a
//! [`Confidence`] is how concentrated a whole Choice or Score distribution
//! is on one. A caller thresholds them differently and swapping them is a
//! silent bug, so they are distinct types; both refuse a value outside
//! `[0, 1]` on construction and on deserialisation ([`Error::Decode`]).
//! TypeSafe's [confidence page](https://docs.typesafe.ai/confidence) defines
//! `confidence` as a statistic of the answer's own `probabilities` (a Noul
//! carries none; [`crate::eval`] uses `max(p, 1 - p)`) and makes the
//! threshold the caller's risk tolerance, so the crate supplies
//! [`Confidence::at_least`] and no policy.
//!
//! # Decoding is tolerant, reading is strict
//!
//! A response is decoded whole, so a strict decoder loses every answer over
//! one it does not know, and TypeSafe and compatible servers add primitives
//! and fields. So the decoder keeps what it does not know, and reading
//! refuses it where it matters:
//!
//! * An answer whose `type` is an unknown string is [`Answer::Unknown`],
//!   logged by the client at `warn`. Under an asked question
//!   [`Response::verify`] refuses the response ([`Error::AnswerTypeMismatch`],
//!   [`Error::is_unfit`]) and [`Response::get`] fails that question; under
//!   an id nobody asked it is kept. The Python SDK skips such an answer and
//!   returns the rest; the crate is stricter here.
//! * A known kind still decodes strictly: `{"type": "noul", "noul": 1.2}`,
//!   a missing or non-string `type` and a non-object are [`Error::Decode`].
//! * An absent or `null` `usage`, or count in it, is zero ([`Usage`]).
//! * Undocumented top-level fields are kept in [`Response::extra`]; fields
//!   inside an answer are ignored, as in the Python SDK.
//!
//! # What `Response::verify` checks
//!
//! Nothing on the wire ties an answer to its question beyond the id, and
//! decoding does not know the questions. [`Response::verify`] holds a
//! response against the [`Questions`] it was sent for; every backend calls
//! it before returning, so afterwards [`Response::get`] with a handle from
//! the same questions cannot fail. In id order, stopping at the first
//! failure: an answer under every id, of the question's primitive, a Choice
//! naming only offered options (the chosen one, then every distribution
//! key), a Score on the scale sent: one legend entry per level keyed `"0"`
//! to `"n-1"`, each the level as sent, probability keys among those exact
//! indices (`"01"` is not one), a score within `0..=n-1` plus a float-error
//! margin, NaN excluded. Every error carries [`Response::request_id`].
//!
//! Left out on purpose: distribution sums (rounding, and the API promises no
//! tolerance); an offered option missing from a distribution (it reads as
//! zero, [`Choice::probability_of`]); which option is chosen (ties and
//! rounding make the argmax a poor check); whether a Score's value is its
//! distribution's expectation (the server rounds both); unasked ids,
//! [`Response::extra`], the model and the usage. An off-list option is an
//! error, never the no-match option: reading it as another option would
//! decide on an answer the model did not give, and nothing is snapped or
//! clamped either. Both official SDKs check an answer's shape and not
//! whether it answers the question it is filed under.
//!
//! A structured level (an object or an array) may be echoed as itself or as
//! a string that parses to it: servers differ (the hosted API echoes the
//! value, `laya-serve` the JSON text it showed the model; see
//! `docs/project/verification/`) and the contract allows either. The comparison is
//! on the parsed value, so spacing and key order do not matter. A string
//! level must come back as that exact string and is never parsed, or a
//! structured echo could pass for it. Either way [`Score::levels`] labels a
//! structured level by its compact JSON.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::Hash;

use serde::de::{self, Deserializer, Unexpected};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::question::{Options, Question, Questions};

/// Margin a Score's value may fall outside `0..=n-1` by: float error in the
/// server's arithmetic. Private, so no caller depends on a looser scale.
const SCORE_EPSILON: f64 = 1e-9;

/// The model's estimate for one outcome, in `[0, 1]`; a value outside fails
/// construction and deserialisation (module docs).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Probability(f64);

impl Probability {
    /// Validate a raw value.
    pub fn new(value: f64) -> Result<Self> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(Error::NotAProbability { value })
        }
    }

    /// The raw value.
    pub const fn value(self) -> f64 {
        self.0
    }

    /// True when at or above `threshold`.
    pub fn at_least(self, threshold: f64) -> bool {
        self.0 >= threshold
    }
}

impl TryFrom<f64> for Probability {
    type Error = Error;
    fn try_from(value: f64) -> Result<Self> {
        Self::new(value)
    }
}

impl From<Probability> for f64 {
    fn from(p: Probability) -> Self {
        p.0
    }
}

impl fmt::Display for Probability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// How concentrated a Choice or Score distribution is on one outcome, in
/// `[0, 1]`. Distinct from [`Probability`] because a caller thresholds the
/// two differently (module docs).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Confidence(f64);

impl Confidence {
    /// Validate a raw value.
    pub fn new(value: f64) -> Result<Self> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(Error::NotAProbability { value })
        }
    }

    /// The raw value.
    pub const fn value(self) -> f64 {
        self.0
    }

    /// True when at or above `threshold`.
    pub fn at_least(self, threshold: f64) -> bool {
        self.0 >= threshold
    }
}

impl TryFrom<f64> for Confidence {
    type Error = Error;
    fn try_from(value: f64) -> Result<Self> {
        Self::new(value)
    }
}

impl From<Confidence> for f64 {
    fn from(c: Confidence) -> Self {
        c.0
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// One answer as returned on the wire, discriminated by `type`.
///
/// Non-exhaustive: TypeSafe adds primitives, each learnt one becomes a
/// variant in a minor release, and until then it decodes as
/// [`Answer::Unknown`] (module docs, `# Decoding is tolerant, reading is
/// strict`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[non_exhaustive]
pub enum Answer {
    /// Probability of yes.
    Noul {
        /// 0 is no, 1 is yes.
        noul: Probability,
    },
    /// The chosen option and the full distribution.
    Choice {
        /// Highest-probability option key.
        choice: String,
        /// Option key to probability; sums to 1.
        probabilities: BTreeMap<String, Probability>,
        /// Distribution concentration.
        confidence: Confidence,
    },
    /// A probability-weighted position on the levels.
    Score {
        /// Weighted position; may fall between levels.
        score: f64,
        /// Level index (as string) to the level as it was sent. JSON, not a
        /// string: the API echoes a structured level as it was described.
        legend: BTreeMap<String, Value>,
        /// Level index (as string) to probability.
        probabilities: BTreeMap<String, Probability>,
        /// Distribution concentration.
        confidence: Confidence,
    },
    /// An answer whose `type` this release does not know, as the JSON
    /// object it came as; a handle reads it as [`Error::AnswerTypeMismatch`]
    /// and it serialises back as it came, so a recording keeps it. A later
    /// release that learns the kind stops decoding it as `Unknown`, so
    /// inspect [`Answer::kind`] rather than this variant.
    #[serde(untagged)]
    Unknown(Value),
}

impl Answer {
    /// The wire `type`: `noul`, `choice`, `score`, or an unknown answer's
    /// own `type` as the server sent it (`unknown` when a hand-built
    /// [`Answer::Unknown`] has none). Not sanitised: escape and cut it, as
    /// the crate does, before it reaches a message or a log line.
    pub fn kind(&self) -> &str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
            Self::Unknown(raw) => raw.get("type").and_then(Value::as_str).unwrap_or("unknown"),
        }
    }
}

/// The kinds [`KnownAnswer`] decodes; any other string `type` is
/// [`Answer::Unknown`].
const KNOWN_KINDS: [&str; 3] = ["noul", "choice", "score"];

/// Longest server-chosen string, in characters once escaped, that reaches an
/// error message, a log line, a span event or a graded judgment.
const SERVER_STR_MAX_CHARS: usize = 64;

/// A string the server chose, made safe to print: escaped with
/// `escape_debug` and cut to `SERVER_STR_MAX_CHARS`, so a hostile or
/// broken server cannot put a line break or an unbounded string into a log
/// line, an error message or an exported span event. The one bound for
/// every such string, so they all read the same way.
pub(crate) fn sanitize_server_str(text: &str) -> String {
    text.escape_debug().take(SERVER_STR_MAX_CHARS).collect()
}

/// The known kinds, decoded strictly by derive; [`Answer`]'s `Deserialize`
/// goes through it for a known `type`.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum KnownAnswer {
    Noul {
        noul: Probability,
    },
    Choice {
        choice: String,
        probabilities: BTreeMap<String, Probability>,
        confidence: Confidence,
    },
    Score {
        score: f64,
        legend: BTreeMap<String, Value>,
        probabilities: BTreeMap<String, Probability>,
        confidence: Confidence,
    },
}

impl From<KnownAnswer> for Answer {
    /// One arm per variant, each naming every field, so a field added to
    /// [`Answer`] and not to the mirror (or the reverse) does not compile.
    fn from(known: KnownAnswer) -> Self {
        match known {
            KnownAnswer::Noul { noul } => Self::Noul { noul },
            KnownAnswer::Choice {
                choice,
                probabilities,
                confidence,
            } => Self::Choice {
                choice,
                probabilities,
                confidence,
            },
            KnownAnswer::Score {
                score,
                legend,
                probabilities,
                confidence,
            } => Self::Score {
                score,
                legend,
                probabilities,
                confidence,
            },
        }
    }
}

/// Dispatches on `type` by hand rather than deriving an untagged fallback
/// variant, which would catch every answer the tagged variants refuse:
/// `{"type": "noul", "noul": 1.2}` would become an unknown kind whose
/// suggested remedy, upgrading, is wrong. A known `type` goes through the
/// strict derive and its errors stand. Buffering through a [`Value`] makes
/// the last of two duplicate keys win, `type` included; accepted, as
/// `kunobi-jev` accepts it, since JSON leaves duplicate keys undefined.
impl<'de> Deserialize<'de> for Answer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let Value::Object(object) = &raw else {
            return Err(de::Error::invalid_type(
                unexpected(&raw),
                &"an answer object",
            ));
        };
        match object.get("type") {
            None => Err(de::Error::missing_field("type")),
            Some(Value::String(kind)) if KNOWN_KINDS.contains(&kind.as_str()) => {
                KnownAnswer::deserialize(raw)
                    .map(Self::from)
                    .map_err(de::Error::custom)
            }
            Some(Value::String(_)) => Ok(Self::Unknown(raw)),
            Some(other) => Err(de::Error::invalid_type(
                unexpected(other),
                &"a string answer type",
            )),
        }
    }
}

/// What serde calls `value` in an "invalid type" message. A string is named
/// by its type alone, never quoted, so a message cannot grow with it.
fn unexpected(value: &Value) -> Unexpected<'_> {
    match value {
        Value::Null => Unexpected::Unit,
        Value::Bool(b) => Unexpected::Bool(*b),
        Value::Number(n) => n
            .as_u64()
            .map(Unexpected::Unsigned)
            .or_else(|| n.as_i64().map(Unexpected::Signed))
            .unwrap_or_else(|| Unexpected::Float(n.as_f64().unwrap_or(f64::NAN))),
        Value::String(_) => Unexpected::Other("string"),
        Value::Array(_) => Unexpected::Seq,
        Value::Object(_) => Unexpected::Map,
    }
}

/// An absent or `null` field as its default: a missing count is zero.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}

/// A string as itself, anything else as `None`: for a body `request_id`,
/// which the documented body does not have, so it may be of any type.
fn string_or_none<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(text) => Some(text),
        _ => None,
    })
}

/// A number as itself, anything else as `None`: for `usage.cost`, which the
/// documented body does not have, so a server's odd value costs the caller
/// the figure, never the response.
fn number_or_none<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    Ok(Value::deserialize(deserializer)?.as_f64())
}

/// Token usage for one request. Output tokens are free; input tokens are billed.
///
/// A count the server did not report reads as zero, which leaves a counter
/// or a sum right; more tolerant than the OpenAPI document (both counts
/// required) and the Python SDK (`usage` has no default). A negative,
/// fractional or string count is still [`Error::Decode`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Usage {
    /// Tokens in `state` plus all questions.
    #[serde(default, deserialize_with = "null_as_default")]
    pub input_tokens: u64,
    /// Tokens in the answers.
    #[serde(default, deserialize_with = "null_as_default")]
    pub output_tokens: u64,
    /// What the call cost in US dollars, when the server says: `OpenRouter`
    /// sends `usage.cost` on every System One response; TypeSafe's own API
    /// does not, and neither does the OpenAPI document. A value that is not
    /// a number reads as `None`. Serialised only when present, so a
    /// recording of a server without it is unchanged.
    #[serde(
        default,
        deserialize_with = "number_or_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub cost: Option<f64>,
}

/// The full response to one evaluation.
///
/// `model` and `answers` are required, although the Python SDK defaults
/// `answers` to empty: the OpenAPI document requires both, and a response
/// with nothing to read is a server error. Everything else is tolerated
/// (module docs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The versioned model that answered (e.g. `jev-1.13.0`), even when the
    /// request used an alias. Log it: thresholds are tuned per version.
    pub model: String,
    /// One answer per question id; an unknown kind is [`Answer::Unknown`],
    /// which [`Response::verify`] refuses under an asked question.
    pub answers: BTreeMap<String, Answer>,
    /// Token accounting; zero for whatever the server did not report.
    #[serde(default, deserialize_with = "null_as_default")]
    pub usage: Usage,
    /// TypeSafe's request id for this call, from the `x-typesafe-request-id`
    /// response header, the one link to TypeSafe's own logs
    /// (`docs/concepts/how-judgment-works.md`); the client sets it after decoding, over any body
    /// `request_id`. Without the header, the body's top-level `id` when it
    /// is a string: `OpenRouter`'s generation id (`gen-dec-…`), the handle
    /// its support asks for, which stays in [`Response::extra`] too. `None`
    /// from a [`crate::Fake`] or a server with neither; serialised only
    /// when present, so a recording keeps it. A body
    /// `request_id` that is not a string reads as `None` rather than failing
    /// the response, since the documented body has no such field.
    #[serde(
        default,
        deserialize_with = "string_or_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub request_id: Option<String>,
    /// Every top-level field beyond the documented ones, as it came (Laya's
    /// `routing`, the `id` and `provider` of `OpenRouter`'s decisions
    /// endpoint), so an operator can log it without a second decoder, and
    /// written back at the top level so a recording keeps it.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Response {
    /// Read the answer for `handle` as its typed view: [`Error::MissingAnswer`],
    /// [`Error::AnswerTypeMismatch`] (an unknown kind included),
    /// [`Error::UnknownOption`] for a typed Choice naming an option outside
    /// its enum (a [`Choice<String>`] passes its keys through), or
    /// [`Error::Decode`] for legend keys that are not indices, the first
    /// three with this response's request id. After [`Response::verify`], it
    /// cannot fail.
    pub fn get<A: FromAnswer>(&self, handle: &crate::Handle<A>) -> Result<A> {
        let id = handle.id();
        let answer = self.answers.get(id).ok_or_else(|| Error::MissingAnswer {
            id: id.to_owned(),
            request_id: self.request_id.clone(),
        })?;
        A::from_answer(id, answer).map_err(|e| e.with_request_id(self.request_id.as_deref()))
    }

    /// Check that this response answers `questions` as they were asked
    /// (module docs, `# What Response::verify checks`): the first failure in
    /// id order, as [`Error::MissingAnswer`], [`Error::AnswerTypeMismatch`],
    /// [`Error::UnknownOption`] or [`Error::InvalidAnswer`], with this
    /// response's request id. Every backend calls it; it is public for a
    /// response that came another way: read from a recording by case id
    /// ([`crate::eval::read_recording`]), a caller's own transport, or built
    /// by hand.
    pub fn verify(&self, questions: &Questions) -> Result<()> {
        for (id, question) in questions.iter() {
            self.verify_answer(id, question)?;
        }
        Ok(())
    }

    /// [`Response::verify`] for one question.
    fn verify_answer(&self, id: &str, question: &Question) -> Result<()> {
        let request_id = || self.request_id.clone();
        let Some(answer) = self.answers.get(id) else {
            return Err(Error::MissingAnswer {
                id: id.to_owned(),
                request_id: request_id(),
            });
        };
        match (question, answer) {
            (Question::Noul { .. }, Answer::Noul { .. }) => Ok(()),
            (
                Question::Choice { criteria, .. },
                Answer::Choice {
                    choice,
                    probabilities,
                    ..
                },
            ) => {
                let offered = |key: &&String| criteria.contains_key(key.as_str());
                match std::iter::once(choice)
                    .chain(probabilities.keys())
                    .find(|key| !offered(key))
                {
                    Some(option) => Err(Error::UnknownOption {
                        id: id.to_owned(),
                        option: option.clone(),
                        request_id: request_id(),
                    }),
                    None => Ok(()),
                }
            }
            (
                Question::Score {
                    criteria: levels, ..
                },
                Answer::Score {
                    score,
                    legend,
                    probabilities,
                    ..
                },
            ) => score_fits(levels, *score, legend, probabilities).map_err(|reason| {
                Error::InvalidAnswer {
                    id: id.to_owned(),
                    reason,
                    request_id: request_id(),
                }
            }),
            (question, answer) => {
                Err(mismatch(id, question.kind(), answer)
                    .with_request_id(self.request_id.as_deref()))
            }
        }
    }
}

/// Whether a Score answer is on the scale of `levels`, or why not. A reason
/// never quotes a level's text (the caller's own, possibly long; the index
/// says which) and quotes a probability key only through
/// [`sanitize_server_str`], since the server chose it.
fn score_fits(
    levels: &[Value],
    score: f64,
    legend: &BTreeMap<String, Value>,
    probabilities: &BTreeMap<String, Probability>,
) -> std::result::Result<(), String> {
    let n = levels.len();
    if legend.len() != n {
        return Err(format!(
            "its legend has {} levels but the question sent {n}",
            legend.len()
        ));
    }
    for (i, level) in levels.iter().enumerate() {
        let Some(echoed) = legend.get(&i.to_string()) else {
            return Err(format!("its legend has no level {i}"));
        };
        if !legend_matches(echoed, level) {
            return Err(format!(
                "legend level {i} is not the level the question sent"
            ));
        }
    }
    if let Some(key) = probabilities.keys().find(|key| !is_level_key(key, n)) {
        return Err(format!(
            "probability key \"{}\" is not a level of its question",
            sanitize_server_str(key)
        ));
    }
    // At most 10 levels (`Questions::score`), so the top index is exact.
    #[allow(clippy::cast_precision_loss)]
    let top = n.saturating_sub(1) as f64;
    // `contains` is false for NaN, so a NaN score is off the scale too.
    if !(-SCORE_EPSILON..=top + SCORE_EPSILON).contains(&score) {
        return Err(format!(
            "score {score} is outside 0..={top}, the scale the question sent"
        ));
    }
    Ok(())
}

/// Whether `key` is exactly the decimal index of one of `n` levels: `"01"`
/// and `"+1"` parse as 1 and are not what a server keys level 1 by.
fn is_level_key(key: &str, n: usize) -> bool {
    key.parse::<usize>()
        .is_ok_and(|i| i < n && i.to_string() == key)
}

/// Whether a legend entry echoes the level sent: a string level as that
/// string, a structured level as the same value or as text that parses to
/// it (module docs, `# What Response::verify checks`).
fn legend_matches(echoed: &Value, sent: &Value) -> bool {
    match sent {
        Value::String(_) => echoed == sent,
        structured => {
            echoed == structured
                || matches!(
                    echoed,
                    Value::String(text)
                        if serde_json::from_str::<Value>(text).is_ok_and(|parsed| parsed == *structured)
                )
        }
    }
}

/// Conversion from a wire [`Answer`] into a typed view. Sealed in practice:
/// implemented for [`Noul`], [`Choice<O>`] and [`Score`].
pub trait FromAnswer: Sized {
    /// Primitive name this view expects, for error messages.
    const KIND: &'static str;
    /// Convert, or explain why the answer does not fit.
    fn from_answer(id: &str, answer: &Answer) -> Result<Self>;
}

/// Typed view of a Noul answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Noul {
    /// Probability that the answer is yes.
    pub yes: Probability,
}

impl Noul {
    /// Convenience: `yes >= threshold`.
    pub fn is_yes(self, threshold: f64) -> bool {
        self.yes.at_least(threshold)
    }
}

impl FromAnswer for Noul {
    const KIND: &'static str = "noul";
    fn from_answer(id: &str, answer: &Answer) -> Result<Self> {
        match answer {
            Answer::Noul { noul } => Ok(Self { yes: *noul }),
            other => Err(mismatch(id, Self::KIND, other)),
        }
    }
}

/// Typed view of a Choice answer. `K` is the Rust option type: an enum
/// implementing [`Options`], or `String` for a dynamic choice.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice<K: Eq + Hash> {
    /// The highest-probability option.
    pub chosen: K,
    /// Every option's probability.
    pub probabilities: HashMap<K, Probability>,
    /// Distribution concentration.
    pub confidence: Confidence,
}

impl<K: Eq + Hash> Choice<K> {
    /// Probability of a specific option (0 if absent from the response).
    pub fn probability_of(&self, option: &K) -> f64 {
        self.probabilities.get(option).map_or(0.0, |p| p.value())
    }

    /// The confidence TypeSafe documents for a Choice, computed from
    /// `probabilities`: `(p_max − 1/n) / (1 − 1/n)` for `n` options, so an
    /// even split reads 0 and all on one option reads 1; one option is 1.
    /// The hosted API agrees to its two decimals
    /// (`docs/project/verification/hosted-typesafe.md`); a compatible server may
    /// define confidence otherwise (Laya: one minus the normalised entropy),
    /// which comparing the two shows. [`Response::verify`] does not check
    /// it: the formula is documentation, not the schema.
    #[allow(clippy::cast_precision_loss)]
    pub fn confidence_from_probabilities(&self) -> f64 {
        let n = self.probabilities.len();
        if n < 2 {
            return 1.0;
        }
        let p_max = self
            .probabilities
            .values()
            .map(|p| p.value())
            .fold(0.0, f64::max);
        let even = 1.0 / n as f64;
        ((p_max - even) / (1.0 - even)).clamp(0.0, 1.0)
    }
}

impl<O: Options> FromAnswer for Choice<O> {
    const KIND: &'static str = "choice";
    fn from_answer(id: &str, answer: &Answer) -> Result<Self> {
        let Answer::Choice {
            choice,
            probabilities,
            confidence,
        } = answer
        else {
            return Err(mismatch(id, Self::KIND, answer));
        };
        let parse = |key: &str| {
            O::from_key(key).ok_or_else(|| Error::UnknownOption {
                id: id.to_owned(),
                option: key.to_owned(),
                request_id: None,
            })
        };
        let chosen = parse(choice)?;
        let probabilities = probabilities
            .iter()
            .map(|(k, p)| Ok((parse(k)?, *p)))
            .collect::<Result<HashMap<O, Probability>>>()?;
        Ok(Self {
            chosen,
            probabilities,
            confidence: *confidence,
        })
    }
}

impl FromAnswer for Choice<String> {
    const KIND: &'static str = "choice";
    fn from_answer(id: &str, answer: &Answer) -> Result<Self> {
        let Answer::Choice {
            choice,
            probabilities,
            confidence,
        } = answer
        else {
            return Err(mismatch(id, Self::KIND, answer));
        };
        Ok(Self {
            chosen: choice.clone(),
            probabilities: probabilities.iter().map(|(k, p)| (k.clone(), *p)).collect(),
            confidence: *confidence,
        })
    }
}

/// Typed view of a Score answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    /// Probability-weighted position, `0.0 ..= levels.len() - 1`.
    pub value: f64,
    /// Level descriptions, lowest first: a string level as is, a structured
    /// level as its compact JSON whichever way the server echoed it, so a
    /// label reads the same from every server. A string level that is
    /// itself JSON text is re-spaced the same way, in the label only.
    pub levels: Vec<String>,
    /// Probability per level, same order as `levels`.
    pub probabilities: Vec<Probability>,
    /// Distribution concentration.
    pub confidence: Confidence,
}

/// The index of the level nearest to `value` on a scale of `len` levels:
/// rounded half away from zero and clamped to `0..len` (0 for no levels or
/// a NaN). Shared with the level sweep in [`crate::eval::tuning`].
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn nearest_index(value: f64, len: usize) -> usize {
    (value.round().max(0.0) as usize).min(len.saturating_sub(1))
}

impl Score {
    /// Index of the level nearest to the weighted value.
    pub fn nearest_level(&self) -> usize {
        nearest_index(self.value, self.levels.len())
    }

    /// Description of the nearest level.
    pub fn nearest_label(&self) -> &str {
        self.levels
            .get(self.nearest_level())
            .map_or("", String::as_str)
    }

    /// The probability-weighted level, `Σ i · p_i`, computed from
    /// `probabilities`. The OpenAPI document defines `score` as exactly this,
    /// so a difference means a server that defines `score` otherwise or a
    /// response edited by hand.
    #[allow(clippy::cast_precision_loss)]
    pub fn expected_value(&self) -> f64 {
        self.probabilities
            .iter()
            .enumerate()
            .map(|(i, p)| i as f64 * p.value())
            .sum()
    }

    /// The confidence TypeSafe documents for a Score, computed from
    /// `probabilities`: `max(0, 1 − spread / even_spread)`, `spread` the
    /// probability-weighted mean distance in levels from the most likely
    /// level and `even_spread` that of a flat distribution from its centre,
    /// so probability on a neighbouring level costs less than the same two
    /// levels away; one level is 1. As for
    /// [`Choice::confidence_from_probabilities`], not checked on the wire.
    #[allow(clippy::cast_precision_loss)]
    pub fn confidence_from_probabilities(&self) -> f64 {
        let n = self.probabilities.len();
        if n < 2 {
            return 1.0;
        }
        let peak = self
            .probabilities
            .iter()
            .enumerate()
            .fold((0, f64::MIN), |best, (i, p)| {
                if p.value() > best.1 {
                    (i, p.value())
                } else {
                    best
                }
            })
            .0;
        let spread: f64 = self
            .probabilities
            .iter()
            .enumerate()
            .map(|(i, p)| p.value() * (i as f64 - peak as f64).abs())
            .sum();
        let centre = (n - 1) as f64 / 2.0;
        let even_spread = (0..n).map(|i| (i as f64 - centre).abs()).sum::<f64>() / n as f64;
        (1.0 - spread / even_spread).max(0.0)
    }
}

/// The text of a legend entry: a string as is, anything else as compact
/// JSON. A string that is the JSON text of an object or an array is taken
/// for a structured level echoed as text and re-rendered compact, so a label
/// carries none of the server's spacing ([`Response::verify`] still holds a
/// string level to its exact text).
fn level_label(level: &Value) -> String {
    match level {
        Value::String(text) => match serde_json::from_str::<Value>(text) {
            Ok(parsed @ (Value::Object(_) | Value::Array(_))) => parsed.to_string(),
            _ => text.clone(),
        },
        other => other.to_string(),
    }
}

impl FromAnswer for Score {
    const KIND: &'static str = "score";
    fn from_answer(id: &str, answer: &Answer) -> Result<Self> {
        let Answer::Score {
            score,
            legend,
            probabilities,
            confidence,
        } = answer
        else {
            return Err(mismatch(id, Self::KIND, answer));
        };
        // Numerically, not lexically: `"10"` sorts before `"2"` as text.
        let mut indexed: Vec<(usize, &Value)> = legend
            .iter()
            .map(|(k, v)| {
                k.parse::<usize>()
                    .map(|i| (i, v))
                    .map_err(|_| Error::Decode {
                        source: serde::de::Error::custom(format!(
                            "score legend key {k:?} is not an index"
                        )),
                        request_id: None,
                    })
            })
            .collect::<Result<_>>()?;
        indexed.sort_by_key(|(i, _)| *i);
        let levels: Vec<String> = indexed.iter().map(|(_, v)| level_label(v)).collect();
        let probs = indexed
            .iter()
            .map(|(i, _)| {
                probabilities
                    .get(&i.to_string())
                    .copied()
                    .unwrap_or(Probability(0.0))
            })
            .collect();
        Ok(Self {
            value: *score,
            levels,
            probabilities: probs,
            confidence: *confidence,
        })
    }
}

/// [`Error::AnswerTypeMismatch`] with no request id (the caller holding the
/// response adds it). An unknown kind is the server's string, so it is
/// escaped and cut ([`sanitize_server_str`]); the known kinds pass through.
fn mismatch(id: &str, expected: &'static str, actual: &Answer) -> Error {
    Error::AnswerTypeMismatch {
        id: id.to_owned(),
        expected,
        actual: sanitize_server_str(actual.kind()),
        request_id: None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::Questions;

    #[test]
    fn the_documented_formulas_reproduce_the_wire_s_confidence_and_score() {
        // Wire values of one hosted Choice and Score
        // (`docs/project/verification/hosted-typesafe.md`), so the formulas are
        // checked against the server, not against themselves.
        let p = |v: f64| Probability::new(v).unwrap();
        let choice = Choice {
            chosen: "billing".to_owned(),
            probabilities: HashMap::from([
                ("billing".to_owned(), p(0.92)),
                ("technical".to_owned(), p(0.08)),
                ("none_of_these".to_owned(), p(0.0)),
            ]),
            confidence: Confidence::new(0.88).unwrap(),
        };
        assert!((choice.confidence_from_probabilities() - 0.88).abs() < 1e-6);
        let score = Score {
            value: 1.98,
            levels: vec!["cosmetic".into(), "degraded".into(), "blocked".into()],
            probabilities: vec![p(0.0), p(0.02), p(0.98)],
            confidence: Confidence::new(0.97).unwrap(),
        };
        assert!((score.expected_value() - 1.98).abs() < 1e-6);
        assert!((score.confidence_from_probabilities() - 0.97).abs() < 1e-6);

        let one = Choice {
            chosen: "only".to_owned(),
            probabilities: HashMap::from([("only".to_owned(), p(1.0))]),
            confidence: Confidence::new(1.0).unwrap(),
        };
        assert!((one.confidence_from_probabilities() - 1.0).abs() < 1e-6);
        let flat = Score {
            value: 1.0,
            levels: vec!["a".into(), "b".into(), "c".into()],
            probabilities: vec![p(1.0 / 3.0); 3],
            confidence: Confidence::new(0.0).unwrap(),
        };
        assert!(flat.confidence_from_probabilities().abs() < 1e-6);
        assert!((flat.expected_value() - 1.0).abs() < 1e-6);
    }
    use serde_json::json;

    crate::options! {
        enum Dept {
            Billing = "billing" => "Money",
            Technical = "technical" => "Bugs",
        }
    }

    fn response(answers: &serde_json::Value) -> Response {
        serde_json::from_value(json!({
            "model": "jev-1.13.0",
            "answers": answers,
            "usage": { "input_tokens": 10, "output_tokens": 2 }
        }))
        .unwrap()
    }

    #[test]
    fn probability_outside_unit_interval_fails_to_deserialise() {
        assert!(serde_json::from_str::<Probability>("1.2").is_err());
        assert!(serde_json::from_str::<Probability>("-0.1").is_err());
        assert!(serde_json::from_str::<Probability>("0.5").is_ok());
    }

    #[test]
    fn typed_choice_maps_back_to_the_enum() {
        let mut q = Questions::new();
        let dept = q.choice::<Dept>("dept", "Which?").unwrap();
        let r = response(&json!({
            "dept": { "type": "choice", "choice": "billing",
                      "probabilities": { "billing": 0.9, "technical": 0.1 }, "confidence": 0.8 }
        }));
        let c = r.get(&dept).unwrap();
        assert_eq!(c.chosen, Dept::Billing);
        assert!((c.probability_of(&Dept::Technical) - 0.1).abs() < 1e-9);
        assert!(c.confidence.at_least(0.8));
    }

    #[test]
    fn unknown_option_is_an_error_not_a_default() {
        let mut q = Questions::new();
        let dept = q.choice::<Dept>("dept", "Which?").unwrap();
        let r = response(&json!({
            "dept": { "type": "choice", "choice": "sales",
                      "probabilities": { "sales": 1.0 }, "confidence": 1.0 }
        }));
        assert!(
            matches!(r.get(&dept), Err(Error::UnknownOption { option, .. }) if option == "sales")
        );
    }

    #[test]
    fn handle_type_is_checked_against_the_answer() {
        let mut q = Questions::new();
        let h = q.noul("x", "?", None).unwrap();
        let r = response(&json!({
            "x": { "type": "score", "score": 1.0, "legend": {"0": "a", "1": "b"},
                   "probabilities": {"0": 0.0, "1": 1.0}, "confidence": 1.0 }
        }));
        assert!(matches!(
            r.get(&h),
            Err(Error::AnswerTypeMismatch {
                expected: "noul",
                actual,
                ..
            }) if actual == "score"
        ));
    }

    #[test]
    fn score_levels_are_ordered_numerically_not_lexically() {
        let mut q = Questions::new();
        let h = q.score("s", "?", vec!["l"; 10]).unwrap();
        let legend: BTreeMap<String, String> =
            (0..10).map(|i| (i.to_string(), format!("L{i}"))).collect();
        let mut probs: BTreeMap<String, f64> = (0..10).map(|i| (i.to_string(), 0.0)).collect();
        probs.insert("9".into(), 1.0);
        let r = response(&json!({
            "s": { "type": "score", "score": 9.0, "legend": legend, "probabilities": probs, "confidence": 1.0 }
        }));
        let s = r.get(&h).unwrap();
        assert_eq!(s.levels[9], "L9");
        assert_eq!(s.nearest_level(), 9);
        assert_eq!(s.nearest_label(), "L9");
    }

    #[test]
    fn a_structured_level_is_echoed_as_json_and_labelled_as_text() {
        let mut q = Questions::new();
        let h = q
            .score(
                "s",
                "?",
                vec![json!({"what": "low", "examples": ["a"]}), json!("high")],
            )
            .unwrap();
        let r = response(&json!({
            "s": { "type": "score", "score": 0.4,
                   "legend": {"0": {"what": "low", "examples": ["a"]}, "1": "high"},
                   "probabilities": {"0": 0.6, "1": 0.4}, "confidence": 0.2 }
        }));
        let s = r.get(&h).unwrap();
        assert_eq!(s.levels, vec![r#"{"examples":["a"],"what":"low"}"#, "high"]);
        assert_eq!(s.nearest_label(), r#"{"examples":["a"],"what":"low"}"#);
    }

    #[test]
    fn a_structured_level_echoed_as_text_is_labelled_by_its_compact_json() {
        let mut q = Questions::new();
        let h = q
            .score(
                "s",
                "?",
                vec![json!({"what": "low", "examples": ["a"]}), json!("high")],
            )
            .unwrap();
        let r = response(&json!({
            "s": { "type": "score", "score": 0.4,
                   "legend": {"0": "{\"examples\": [\"a\"], \"what\": \"low\"}", "1": "high"},
                   "probabilities": {"0": 0.6, "1": 0.4}, "confidence": 0.2 }
        }));
        r.verify(&q).unwrap();
        let s = r.get(&h).unwrap();
        assert_eq!(s.levels, vec![r#"{"examples":["a"],"what":"low"}"#, "high"]);
        assert_eq!(s.nearest_label(), r#"{"examples":["a"],"what":"low"}"#);
        // A string level that is JSON text: exact for `verify`, re-spaced in its label.
        let mut q = Questions::new();
        let h = q.score("s", "?", [r#"{ "what": "low" }"#, "high"]).unwrap();
        let r = response(&json!({
            "s": { "type": "score", "score": 0.4,
                   "legend": {"0": "{ \"what\": \"low\" }", "1": "high"},
                   "probabilities": {"0": 0.6, "1": 0.4}, "confidence": 0.2 }
        }));
        r.verify(&q).unwrap();
        assert_eq!(r.get(&h).unwrap().levels[0], r#"{"what":"low"}"#);
    }

    #[test]
    fn a_response_request_id_is_serialised_only_when_present() {
        let mut r = response(&json!({}));
        assert_eq!(r.request_id, None, "absent from the body reads as None");
        let text = serde_json::to_string(&r).unwrap();
        assert!(!text.contains("request_id"), "{text}");
        assert_eq!(serde_json::from_str::<Response>(&text).unwrap(), r);

        r.request_id = Some("req_abc".into());
        let value = serde_json::to_value(&r).unwrap();
        assert_eq!(value["request_id"], "req_abc");
        assert_eq!(serde_json::from_value::<Response>(value).unwrap(), r);
    }

    #[test]
    fn missing_answer_is_reported_by_id() {
        let mut q = Questions::new();
        let h = q.noul("absent", "?", None).unwrap();
        let r = response(&json!({}));
        assert!(matches!(r.get(&h), Err(Error::MissingAnswer { id, .. }) if id == "absent"));
    }

    /// Decode `body`'s JSON text, the way the client decodes a reply.
    fn decode<T: serde::de::DeserializeOwned>(body: &Value) -> serde_json::Result<T> {
        serde_json::from_str(&body.to_string())
    }

    #[test]
    fn an_unknown_answer_kind_is_kept_raw_and_serialises_back() {
        let raw = json!({ "type": "rank", "ranking": ["b", "a"], "confidence": 0.4 });
        let answer: Answer = decode(&raw).unwrap();
        assert_eq!(answer, Answer::Unknown(raw.clone()));
        assert_eq!(answer.kind(), "rank");
        assert_eq!(serde_json::to_value(&answer).unwrap(), raw);

        let r = response(&json!({ "later": raw }));
        assert!(r.extra.is_empty(), "{:?}", r.extra);
        let again: Response = decode(&serde_json::to_value(&r).unwrap()).unwrap();
        assert_eq!(again, r);
        assert_eq!(
            serde_json::to_value(&again).unwrap()["answers"]["later"],
            raw
        );

        let noul = Answer::Noul {
            noul: Probability::new(0.25).unwrap(),
        };
        assert_eq!(
            serde_json::to_value(&noul).unwrap(),
            json!({ "type": "noul", "noul": 0.25 })
        );

        let hand = Answer::Unknown(json!({ "type": "noul", "noul": 0.5 }));
        assert_eq!(hand.kind(), "noul");
        assert_eq!(
            decode::<Answer>(&serde_json::to_value(&hand).unwrap()).unwrap(),
            Answer::Noul {
                noul: Probability::new(0.5).unwrap()
            }
        );
        assert_eq!(Answer::Unknown(json!(5)).kind(), "unknown");
        assert_eq!(Answer::Unknown(json!({ "type": 5 })).kind(), "unknown");
    }

    #[test]
    fn a_known_kind_that_does_not_decode_is_still_an_error() {
        let cases = [
            (
                json!({ "type": "noul", "noul": 1.2 }),
                "is not a probability",
            ),
            (
                json!({ "type": "choice", "choice": "billing", "confidence": 0.5 }),
                "missing field `probabilities`",
            ),
            (
                json!({ "type": "score", "score": 1.0, "legend": { "0": "a", "1": "b" },
                        "probabilities": { "0": 0.0, "1": 1.0 } }),
                "missing field `confidence`",
            ),
        ];
        for (bad, message) in cases {
            let err = decode::<Answer>(&bad).unwrap_err();
            assert!(err.to_string().contains(message), "{bad}: {err}");
            let body = json!({ "model": "m", "answers": { "x": bad }, "usage": {} });
            let err = decode::<Response>(&body).unwrap_err();
            assert!(err.to_string().contains(message), "{body}: {err}");
        }
    }

    #[test]
    fn an_answer_without_a_string_type_is_an_error() {
        let cases = [
            (json!({ "noul": 0.5 }), "missing field `type`"),
            (
                json!({ "type": 3 }),
                "invalid type: integer `3`, expected a string answer type",
            ),
            (
                json!({ "type": null, "noul": 0.5 }),
                "invalid type: null, expected a string answer type",
            ),
            (
                json!(0.5),
                "invalid type: floating point `0.5`, expected an answer object",
            ),
            (
                json!(["noul", 0.5]),
                "invalid type: sequence, expected an answer object",
            ),
            // A string is named by its type, not quoted: the server chose it.
            (
                json!("noul"),
                "invalid type: string, expected an answer object",
            ),
        ];
        for (bad, message) in cases {
            let err = decode::<Answer>(&bad).unwrap_err();
            assert!(err.to_string().contains(message), "{bad}: {err}");
        }
        let long = json!({ "model": "m", "answers": { "x": "y".repeat(10_000) } });
        let err = decode::<Response>(&long).unwrap_err().to_string();
        assert!(err.len() < 200, "{} bytes: {err}", err.len());
    }

    #[test]
    fn duplicate_keys_in_an_answer_are_last_one_wins() {
        let answer: Answer =
            serde_json::from_str(r#"{"type":"noul","noul":0.2,"noul":0.9}"#).unwrap();
        assert_eq!(
            answer,
            Answer::Noul {
                noul: Probability::new(0.9).unwrap()
            }
        );
        // A duplicate `type` decides the kind by its last value, either way.
        let answer: Answer =
            serde_json::from_str(r#"{"type":"noul","noul":0.5,"type":"rank"}"#).unwrap();
        assert!(matches!(&answer, Answer::Unknown(_)), "{answer:?}");
        assert_eq!(answer.kind(), "rank");
        let answer: Answer =
            serde_json::from_str(r#"{"type":"rank","noul":0.5,"type":"noul"}"#).unwrap();
        assert_eq!(
            answer,
            Answer::Noul {
                noul: Probability::new(0.5).unwrap()
            }
        );
    }

    #[test]
    fn an_unknown_kind_does_not_fail_the_other_answers() {
        let mut q = Questions::new();
        let dept = q.choice::<Dept>("dept", "Which?").unwrap();
        let order = q.noul("order", "?", None).unwrap();
        let r = response(&json!({
            "dept": { "type": "choice", "choice": "billing",
                      "probabilities": { "billing": 0.9, "technical": 0.1 }, "confidence": 0.8 },
            "order": { "type": "rank", "ranking": ["billing", "technical"] }
        }));
        assert_eq!(r.get(&dept).unwrap().chosen, Dept::Billing);
        let err = r.get(&order).unwrap_err();
        assert!(
            matches!(
                &err,
                Error::AnswerTypeMismatch { id, expected: "noul", actual, .. }
                    if id == "order" && actual == "rank"
            ),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            r#"answer "order" is a rank but a noul was requested"#
        );
    }

    #[test]
    fn a_hostile_unknown_kind_is_escaped_in_the_mismatch() {
        let kind = format!("a\nb{}", "x".repeat(100));
        let r = response(&json!({ "x": { "type": kind } }));
        assert_eq!(r.answers["x"].kind(), kind, "kind() is the raw string");

        let mut q = Questions::new();
        let h = q.noul("x", "?", None).unwrap();
        let err = r.get(&h).unwrap_err();
        let Error::AnswerTypeMismatch { actual, .. } = &err else {
            panic!("{err:?}");
        };
        assert!(!actual.contains('\n'), "{actual:?}");
        assert!(actual.chars().count() <= 64, "{}", actual.chars().count());
        assert!(actual.starts_with(r"a\nb"), "{actual:?}");
        assert!(!err.to_string().contains('\n'), "{err}");
        assert_eq!(sanitize_server_str("rank"), "rank");
        assert_eq!(sanitize_server_str(&"r".repeat(64)), "r".repeat(64));
        assert_eq!(sanitize_server_str(&"r".repeat(65)), "r".repeat(64));
    }

    #[test]
    fn server_strings_in_fit_errors_are_escaped_and_bounded() {
        let long = "o".repeat(10_000);
        let bounded = |err: &Error| {
            let message = err.to_string();
            assert!(message.len() < 400, "{} bytes: {message}", message.len());
            assert!(!message.contains('\n'), "{message}");
            message
        };

        // The message is cut; the field keeps the server's string whole.
        let Asked { q, dept, .. } = asked();
        let err = with(
            "owner",
            json!({ "type": "choice", "choice": long, "probabilities": {}, "confidence": 1.0 }),
        )
        .verify(&q)
        .unwrap_err();
        assert!(
            matches!(&err, Error::UnknownOption { option, .. } if *option == long),
            "{err:?}"
        );
        let message = bounded(&err);
        assert!(
            message.contains(&format!("option \"{}\",", "o".repeat(64))),
            "{message}"
        );

        let r = with(
            "dept",
            json!({ "type": "choice", "choice": format!("a\nb{long}"),
                    "probabilities": {}, "confidence": 1.0 }),
        );
        let err = r.get(&dept).unwrap_err();
        assert!(matches!(&err, Error::UnknownOption { .. }), "{err:?}");
        assert!(bounded(&err).contains(r#"option "a\nb"#), "{err}");

        let mut probs = json!({ "0": 0.0, "1": 0.0, "2": 1.0 });
        probs[long.as_str()] = json!(0.0);
        let err = with("impact", score_answer(2.0, &legend(), &probs))
            .verify(&q)
            .unwrap_err();
        assert!(reason(&err).starts_with("probability key \""), "{err}");
        bounded(&err);

        // A short option that needs no escaping reads as `{:?}` would put it.
        let err = with(
            "owner",
            json!({ "type": "choice", "choice": "made-up-team", "probabilities": {},
                    "confidence": 1.0 }),
        )
        .verify(&q)
        .unwrap_err();
        assert!(
            err.to_string()
                .contains(&format!("names option {:?},", "made-up-team")),
            "{err}"
        );
    }

    #[test]
    fn usage_is_zero_when_absent_or_null() {
        let bodies = [
            json!({ "model": "m", "answers": {} }),
            json!({ "model": "m", "answers": {}, "usage": null }),
            json!({ "model": "m", "answers": {}, "usage": {} }),
            json!({ "model": "m", "answers": {},
                    "usage": { "input_tokens": null, "output_tokens": null, "cost": null } }),
        ];
        for body in bodies {
            let r: Response = decode(&body).unwrap();
            assert_eq!(r.usage, Usage::default(), "{body}");
            assert!(r.extra.is_empty(), "{body}: {:?}", r.extra);
        }
        let r: Response = decode(&json!({ "model": "m", "answers": {},
                                          "usage": { "input_tokens": 7 } }))
        .unwrap();
        assert_eq!(
            r.usage,
            Usage {
                input_tokens: 7,
                output_tokens: 0,
                cost: None,
            }
        );
        // Written back, a defaulted usage is explicit zeros.
        assert_eq!(
            serde_json::to_value(r.usage).unwrap(),
            json!({ "input_tokens": 7, "output_tokens": 0 })
        );
    }

    #[test]
    fn negative_or_string_usage_is_still_an_error() {
        for usage in [
            json!({ "input_tokens": -1 }),
            json!({ "input_tokens": "3" }),
            json!({ "output_tokens": 1.5 }),
        ] {
            let body = json!({ "model": "m", "answers": {}, "usage": usage });
            assert!(decode::<Response>(&body).is_err(), "{body}");
        }
        // `model` and `answers` stay required.
        assert!(decode::<Response>(&json!({ "answers": {} })).is_err());
        assert!(decode::<Response>(&json!({ "model": "m" })).is_err());
    }

    #[test]
    fn undocumented_top_level_fields_are_kept_and_written_back() {
        let laya = json!({
            "model": "laya-rl-agent",
            "answers": {
                "urgent": { "type": "noul", "noul": 0.8123, "confidence": 0.8123,
                            "action": { "act_probability": 0.4 } }
            },
            "usage": { "input_tokens": 120, "output_tokens": 0 },
            "routing": { "model": "typed-decisions", "reason": "explicit" }
        });
        let r: Response = decode(&laya).unwrap();
        assert_eq!(r.extra.keys().collect::<Vec<_>>(), ["routing"]);
        assert_eq!(r.extra["routing"]["model"], "typed-decisions");
        assert_eq!(
            r.answers["urgent"],
            Answer::Noul {
                noul: Probability::new(0.8123).unwrap()
            },
            "fields inside an answer are ignored"
        );
        let written = serde_json::to_value(&r).unwrap();
        assert_eq!(
            written["routing"], laya["routing"],
            "written back at the top"
        );
        assert!(written.get("extra").is_none(), "{written}");
        assert_eq!(decode::<Response>(&written).unwrap(), r);

        let routed = json!({
            "model": "typesafe/jev-1.13",
            "answers": {
                "department": { "type": "choice", "choice": "billing",
                                "probabilities": { "sales": 0, "billing": 0.9, "technical": 0.1 },
                                "confidence": 0.85 },
                "severity": { "type": "score", "score": 1.1,
                              "legend": { "0": "minor", "1": "major", "2": "outage" },
                              "probabilities": { "0": 0, "1": 0.9, "2": 0.1 },
                              "confidence": 0.8 }
            },
            "usage": { "input_tokens": 400, "output_tokens": 60, "cost": 0.000_02 },
            "id": "gen-dec-0001",
            "provider": "TypeSafe"
        });
        let r: Response = decode(&routed).unwrap();
        assert_eq!(r.extra.keys().collect::<Vec<_>>(), ["id", "provider"]);
        assert_eq!(
            r.usage,
            Usage {
                input_tokens: 400,
                output_tokens: 60,
                cost: Some(0.000_02),
            }
        );
        let Answer::Choice { probabilities, .. } = &r.answers["department"] else {
            panic!("{:?}", r.answers["department"]);
        };
        assert!(probabilities["sales"].value().abs() < f64::EPSILON);
        let mut q = Questions::new();
        let severity = q
            .score("severity", "?", ["minor", "major", "outage"])
            .unwrap();
        assert_eq!(r.get(&severity).unwrap().nearest_label(), "major");
        let written = serde_json::to_value(&r).unwrap();
        assert_eq!(written["id"], "gen-dec-0001");
        assert_eq!(written["provider"], "TypeSafe");
        assert_eq!(decode::<Response>(&written).unwrap(), r);

        let r: Response =
            decode(&json!({ "model": "m", "answers": {}, "request_id": "req_1" })).unwrap();
        assert_eq!(r.request_id.as_deref(), Some("req_1"));
        assert!(r.extra.is_empty(), "{:?}", r.extra);
        for id in [
            json!(123),
            json!({ "a": 1 }),
            json!(["req"]),
            json!(true),
            json!(null),
        ] {
            let body = json!({ "model": "m", "answers": {}, "request_id": id });
            let r: Response = decode(&body).unwrap();
            assert_eq!(r.request_id, None, "{body}");
            assert!(r.extra.is_empty(), "{body}: {:?}", r.extra);
        }
        // A response is an object, never serde's array form of a struct.
        assert!(serde_json::from_str::<Response>(r#"["m", {}, {}]"#).is_err());
    }

    // Response::verify

    const LEVELS: [&str; 4] = ["none", "minor", "major", "outage"];

    /// One question of each primitive, and a typed Choice, with handles.
    struct Asked {
        q: Questions,
        dept: crate::Handle<Choice<Dept>>,
        owner: crate::Handle<Choice<String>>,
        urgent: crate::Handle<Noul>,
        impact: crate::Handle<Score>,
    }

    fn asked() -> Asked {
        let mut q = Questions::new();
        let dept = q.choice::<Dept>("dept", "Which team?").unwrap();
        let owner = q
            .dynamic_choice(
                "owner",
                "Who owns it?",
                [
                    ("payments".to_owned(), Some("Checkout".to_owned())),
                    ("platform".to_owned(), None),
                    ("none_of_these".to_owned(), None),
                ],
            )
            .unwrap();
        let urgent = q.noul("urgent", "Urgent?", None).unwrap();
        let impact = q.score("impact", "How bad?", LEVELS).unwrap();
        Asked {
            q,
            dept,
            owner,
            urgent,
            impact,
        }
    }

    fn legend() -> Value {
        json!({ "0": "none", "1": "minor", "2": "major", "3": "outage" })
    }

    /// A response that answers [`asked`] as asked.
    fn fitting() -> Value {
        json!({
            "dept": { "type": "choice", "choice": "billing",
                      "probabilities": { "billing": 0.9, "technical": 0.1 }, "confidence": 0.8 },
            "owner": { "type": "choice", "choice": "payments",
                       "probabilities": { "payments": 0.7, "platform": 0.3 }, "confidence": 0.6 },
            "urgent": { "type": "noul", "noul": 0.8 },
            "impact": { "type": "score", "score": 2.0, "legend": legend(),
                        "probabilities": { "0": 0.0, "1": 0.1, "2": 0.8, "3": 0.1 },
                        "confidence": 0.7 }
        })
    }

    /// `fitting()` with `id`'s answer replaced.
    fn with(id: &str, answer: Value) -> Response {
        let mut answers = fitting();
        answers[id] = answer;
        response(&answers)
    }

    fn score_answer(score: f64, legend: &Value, probabilities: &Value) -> Value {
        json!({ "type": "score", "score": score, "legend": legend,
                "probabilities": probabilities, "confidence": 0.5 })
    }

    fn reason(err: &Error) -> &str {
        match err {
            Error::InvalidAnswer { reason, .. } => reason,
            other => panic!("not InvalidAnswer: {other:?}"),
        }
    }

    #[test]
    fn verify_accepts_a_response_that_answers_every_question_as_asked() {
        let Asked {
            q,
            dept,
            owner,
            urgent,
            impact,
        } = asked();
        let mut answers = fitting();
        // An offered option left out of the distribution reads as zero, and
        // an answer to a question nobody asked is ignored.
        answers["owner"]["probabilities"] = json!({ "payments": 0.7, "platform": 0.3 });
        answers["unasked"] = json!({ "type": "rank", "ranking": [] });
        let r = response(&answers);
        r.verify(&q).unwrap();
        r.verify(&q).unwrap();

        assert_eq!(r.get(&dept).unwrap().chosen, Dept::Billing);
        let owner = r.get(&owner).unwrap();
        assert_eq!(owner.chosen, "payments");
        assert!(owner.probability_of(&"none_of_these".to_owned()).abs() < f64::EPSILON);
        assert!(r.get(&urgent).unwrap().is_yes(0.5));
        let impact = r.get(&impact).unwrap();
        assert_eq!(impact.levels, LEVELS);
        assert_eq!(impact.nearest_label(), "major");
    }

    #[test]
    fn verify_reports_the_first_unanswered_question_in_wire_order() {
        let Asked { q, .. } = asked();
        let mut answers = fitting();
        let map = answers.as_object_mut().unwrap();
        map.remove("owner");
        map.remove("urgent");
        let err = response(&answers).verify(&q).unwrap_err();
        assert!(
            matches!(&err, Error::MissingAnswer { id, request_id: None } if id == "owner"),
            "{err:?}"
        );
        assert!(err.is_unfit());
        assert_eq!(err.to_string(), r#"no answer for question "owner""#);
    }

    #[test]
    fn verify_refuses_an_answer_of_another_kind() {
        let Asked { q, .. } = asked();
        let err = with(
            "urgent",
            json!({ "type": "choice", "choice": "billing",
                                         "probabilities": { "billing": 1.0 }, "confidence": 1.0 }),
        )
        .verify(&q)
        .unwrap_err();
        assert!(
            matches!(
                &err,
                Error::AnswerTypeMismatch { id, expected: "noul", actual, .. }
                    if id == "urgent" && actual == "choice"
            ),
            "{err:?}"
        );
        let err = with("impact", json!({ "type": "noul", "noul": 0.5 }))
            .verify(&q)
            .unwrap_err();
        assert!(
            matches!(&err, Error::AnswerTypeMismatch { expected: "score", actual, .. } if actual == "noul"),
            "{err:?}"
        );
    }

    #[test]
    fn an_unknown_answer_kind_for_an_asked_question_is_a_type_mismatch() {
        let Asked { q, .. } = asked();
        let err = with("urgent", json!({ "type": "rank", "ranking": ["a"] }))
            .verify(&q)
            .unwrap_err();
        assert!(
            matches!(
                &err,
                Error::AnswerTypeMismatch { id, expected: "noul", actual, .. }
                    if id == "urgent" && actual == "rank"
            ),
            "{err:?}"
        );
        // Its kind is escaped and cut, as it is for `get`.
        let hostile = format!("a\nb{}", "x".repeat(100));
        let err = with("urgent", json!({ "type": hostile }))
            .verify(&q)
            .unwrap_err();
        let Error::AnswerTypeMismatch { actual, .. } = &err else {
            panic!("{err:?}");
        };
        assert!(
            !actual.contains('\n') && actual.chars().count() <= 64,
            "{actual:?}"
        );

        // Under an id nobody asked, an unknown answer is only kept.
        let mut answers = fitting();
        answers["later"] = json!({ "type": "rank" });
        response(&answers).verify(&q).unwrap();
    }

    #[test]
    fn verify_refuses_a_chosen_option_the_question_did_not_offer() {
        let Asked { q, .. } = asked();
        let err = with(
            "owner",
            json!({ "type": "choice", "choice": "made-up-team",
                                        "probabilities": { "payments": 0.3 }, "confidence": 0.9 }),
        )
        .verify(&q)
        .unwrap_err();
        assert!(
            matches!(&err, Error::UnknownOption { id, option, .. }
                if id == "owner" && option == "made-up-team"),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            r#"answer "owner" names option "made-up-team", which its question does not offer"#
        );
        // The chosen option is checked before the distribution.
        let err = with("dept", json!({ "type": "choice", "choice": "sales",
                                       "probabilities": { "billing": 0.1, "zzz": 0.9 }, "confidence": 0.9 }))
        .verify(&q)
        .unwrap_err();
        assert!(
            matches!(&err, Error::UnknownOption { id, option, .. } if id == "dept" && option == "sales"),
            "{err:?}"
        );
    }

    #[test]
    fn verify_refuses_a_distribution_key_the_question_did_not_offer() {
        let Asked { q, .. } = asked();
        let err = with(
            "owner",
            json!({ "type": "choice", "choice": "payments",
                                        "probabilities": { "payments": 0.7, "INC-9999": 0.3 },
                                        "confidence": 0.6 }),
        )
        .verify(&q)
        .unwrap_err();
        assert!(
            matches!(&err, Error::UnknownOption { id, option, .. }
                if id == "owner" && option == "INC-9999"),
            "{err:?}"
        );
    }

    #[test]
    fn verify_does_not_check_that_probabilities_sum_to_one() {
        let Asked { q, .. } = asked();
        let short = with(
            "owner",
            json!({ "type": "choice", "choice": "payments",
                                          "probabilities": { "payments": 0.67, "platform": 0.3 },
                                          "confidence": 0.6 }),
        );
        short.verify(&q).unwrap();
        let long = with(
            "impact",
            score_answer(
                2.0,
                &legend(),
                &json!({ "0": 0.1, "1": 0.1, "2": 0.7002, "3": 0.1 }),
            ),
        );
        long.verify(&q).unwrap();
    }

    #[test]
    fn verify_refuses_a_legend_that_is_not_the_levels_sent() {
        let Asked { q, .. } = asked();
        let probs = json!({ "0": 0.0, "1": 0.0, "2": 1.0, "3": 0.0 });
        let cases = [
            (
                json!({ "0": "none", "1": "minor", "2": "major", "3": "outage", "4": "worse" }),
                "its legend has 5 levels but the question sent 4",
            ),
            (
                json!({ "1": "none", "2": "minor", "3": "major", "4": "outage" }),
                "its legend has no level 0",
            ),
            (
                json!({ "0": "none", "1": "minor", "2": "major (reworded)", "3": "outage" }),
                "legend level 2 is not the level the question sent",
            ),
            (
                json!({ "0": "none", "1": "a", "2": "major", "3": "outage" }),
                "legend level 1 is not the level the question sent",
            ),
        ];
        for (legend, expected) in cases {
            let err = with("impact", score_answer(2.0, &legend, &probs))
                .verify(&q)
                .unwrap_err();
            assert_eq!(reason(&err), expected, "{legend}");
            let message = err.to_string();
            assert!(
                message.starts_with(r#"answer "impact" does not fit its question: "#),
                "{message}"
            );
            for level in LEVELS {
                assert!(!message.contains(level), "{message} quotes {level:?}");
            }
        }
    }

    #[test]
    fn a_structured_level_may_be_echoed_as_itself_or_as_its_json_text() {
        let level = json!({ "what": "low", "examples": ["a typo"] });
        let mut q = Questions::new();
        q.score("s", "?", vec![level.clone(), json!("high")])
            .unwrap();
        let answer = |echo: Value| {
            response(&json!({ "s": score_answer(
                0.4,
                &json!({ "0": echo, "1": "high" }),
                &json!({ "0": 0.6, "1": 0.4 }),
            ) }))
        };
        // The value itself (the hosted API).
        answer(level.clone()).verify(&q).unwrap();
        answer(json!(r#"{"examples":["a typo"],"what":"low"}"#))
            .verify(&q)
            .unwrap();
        // `json.dumps` with Python's separators, keys as received: `laya-serve`.
        answer(json!(r#"{"what": "low", "examples": ["a typo"]}"#))
            .verify(&q)
            .unwrap();
        // Any other spacing: the comparison is on the parsed value.
        answer(json!(
            "{ \"examples\" : [ \"a typo\" ] ,\n \"what\" : \"low\" }"
        ))
        .verify(&q)
        .unwrap();
        // Another value, text that does not parse, one field's text: not it.
        for echo in [
            json!(r#"{"what": "low", "examples": ["a typo", "another"]}"#),
            json!(r#"{"what": "low""#),
            json!("low"),
        ] {
            let err = answer(echo.clone()).verify(&q).unwrap_err();
            assert_eq!(
                reason(&err),
                "legend level 0 is not the level the question sent",
                "{echo}"
            );
        }
        // An array level, both ways; a number level as text is the same
        // number only when it is written the same way.
        let mut q = Questions::new();
        q.score(
            "s",
            "?",
            vec![json!(["degraded", "some users cannot pay"]), json!(1)],
        )
        .unwrap();
        let answer = |first: Value, second: Value| {
            response(&json!({ "s": score_answer(
                0.4,
                &json!({ "0": first, "1": second }),
                &json!({ "0": 0.6, "1": 0.4 }),
            ) }))
        };
        answer(json!(["degraded", "some users cannot pay"]), json!(1))
            .verify(&q)
            .unwrap();
        answer(
            json!(r#"["degraded", "some users cannot pay"]"#),
            json!("1"),
        )
        .verify(&q)
        .unwrap();
        let err = answer(
            json!(r#"["degraded", "some users cannot pay"]"#),
            json!("1.0"),
        )
        .verify(&q)
        .unwrap_err();
        assert_eq!(
            reason(&err),
            "legend level 1 is not the level the question sent"
        );
        // A string level matches neither a structured echo nor equivalent text.
        let mut q = Questions::new();
        q.score("s", "?", [r#"{"what":"low"}"#, "high"]).unwrap();
        let answer = |echo: Value| {
            response(&json!({ "s": score_answer(
                0.4,
                &json!({ "0": echo, "1": "high" }),
                &json!({ "0": 0.6, "1": 0.4 }),
            ) }))
        };
        for echo in [json!({ "what": "low" }), json!(r#"{"what": "low"}"#)] {
            let err = answer(echo.clone()).verify(&q).unwrap_err();
            assert_eq!(
                reason(&err),
                "legend level 0 is not the level the question sent",
                "{echo}"
            );
        }
    }

    #[test]
    fn verify_refuses_a_probability_key_that_is_not_a_level() {
        let Asked { q, .. } = asked();
        for key in ["4", "01", "+1", "one"] {
            let mut probs = json!({ "0": 0.0, "1": 0.0, "2": 1.0 });
            probs[key] = json!(0.0);
            let err = with("impact", score_answer(2.0, &legend(), &probs))
                .verify(&q)
                .unwrap_err();
            assert_eq!(
                reason(&err),
                format!("probability key {key:?} is not a level of its question")
            );
        }
    }

    #[test]
    fn verify_is_strict_about_the_score_scale() {
        let Asked { q, .. } = asked();
        let probs = json!({ "0": 0.25, "1": 0.25, "2": 0.25, "3": 0.25 });
        for fits in [0.0, 3.0, 1.5, 3.0 + 1e-10, -1e-10] {
            with("impact", score_answer(fits, &legend(), &probs))
                .verify(&q)
                .unwrap();
        }
        for off in [3.001, -0.001] {
            let err = with("impact", score_answer(off, &legend(), &probs))
                .verify(&q)
                .unwrap_err();
            assert_eq!(
                reason(&err),
                format!("score {off} is outside 0..=3, the scale the question sent")
            );
        }
        // NaN never reaches the wire as JSON, but a hand-built response can carry one.
        let mut r = with("impact", score_answer(1.0, &legend(), &probs));
        if let Some(Answer::Score { score, .. }) = r.answers.get_mut("impact") {
            *score = f64::NAN;
        }
        let err = r.verify(&q).unwrap_err();
        assert!(reason(&err).contains("outside 0..=3"), "{err}");
    }

    #[test]
    fn verify_and_get_errors_carry_the_responses_request_id() {
        let Asked {
            q, dept, impact, ..
        } = asked();
        let mut r = with(
            "dept",
            json!({ "type": "choice", "choice": "sales",
                                         "probabilities": { "sales": 1.0 }, "confidence": 1.0 }),
        );
        r.request_id = Some("req_unfit".into());
        let err = r.verify(&q).unwrap_err();
        assert_eq!(err.request_id(), Some("req_unfit"));
        assert!(
            err.to_string().ends_with(" [request_id req_unfit]"),
            "{err}"
        );
        let err = r.get(&dept).unwrap_err();
        assert!(matches!(err, Error::UnknownOption { .. }), "{err:?}");
        assert_eq!(err.request_id(), Some("req_unfit"));

        let mut r = with("urgent", json!({ "type": "noul", "noul": 0.5 }));
        r.request_id = Some("req_missing".into());
        r.answers.remove("impact");
        for err in [r.verify(&q).unwrap_err(), r.get(&impact).unwrap_err()] {
            assert!(matches!(err, Error::MissingAnswer { .. }), "{err:?}");
            assert_eq!(err.request_id(), Some("req_missing"));
        }
        let mut other = Questions::new();
        let wrong = other.score("urgent", "?", LEVELS).unwrap();
        let err = r.get(&wrong).unwrap_err();
        assert!(matches!(err, Error::AnswerTypeMismatch { .. }), "{err:?}");
        assert_eq!(err.request_id(), Some("req_missing"));
        // A response with no id gives errors with none.
        r.request_id = None;
        assert_eq!(r.verify(&q).unwrap_err().request_id(), None);
    }
}

/// Bounded proof of the level index a Score resolves to, run with
/// `cargo kani` (docs/guides/record-replay-and-test.md, "Bounded proofs").
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// For any value (NaN and infinities included) and any `len`, the result
    /// indexes the scale (0 when empty), within half a level of a value on it.
    #[kani::proof]
    fn nearest_index_is_a_level_of_the_scale() {
        let value: f64 = kani::any();
        let len: usize = kani::any();

        let level = nearest_index(value, len);

        if len == 0 {
            assert_eq!(level, 0);
        } else {
            assert!(level < len);
            #[allow(clippy::cast_precision_loss)]
            let top = (len - 1) as f64;
            if (0.0..=top).contains(&value) {
                #[allow(clippy::cast_precision_loss)]
                let distance = (level as f64 - value).abs();
                assert!(distance <= 0.5);
            }
        }
        kani::cover!(len == 4 && value > 3.0 && level == 3);
        kani::cover!(value.is_nan() && level == 0);
    }
}
