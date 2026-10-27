//! Questions: the four System One primitives, a request builder, and the
//! typed handles that tie each question to the shape of its answer
//! (`docs/concepts/how-judgment-works.md` draws the mechanism).
//!
//! # A Box needs an image, and Jev 1.14
//!
//! A Box question (`Questions::r#box`) locates what its instructions
//! describe in an image the request carries ([`crate::asset`]); it takes
//! no criteria and its instructions cannot be null, since they are all
//! the question is. The hosted API answers it on Jev 1.14 or later
//! (`jev-latest` and `jev-preview` both point to `jev-1.14.0` as of
//! 2026-10-25; a pinned `jev-1.13.0` refuses it), under the images header
//! the client sets; the answer is
//! always a box, so the Box page pairs it with a Noul
//! ([`crate::BoundingBox`]).
//!
//! # Ids are for code, not for the model
//!
//! The API never shows the id to the model, so `instructions` must stand
//! alone; they may be `null` (`()`, [`Value::Null`], `None::<&str>`,
//! `Option<String>`) only when the criteria carry the question. A null is
//! sent, never omitted: the OpenAPI document allows it, the reference page
//! marks the field required and Laya reads the key unconditionally.
//!
//! # A Choice should carry a no-match option
//!
//! The model must pick one option even when none fits; an `other` option
//! gives the code a branch. [`options!`](crate::options) does not add one,
//! because what "none" means is part of the question.
//!
//! # Limits are checked here
//!
//! The Choice and Score pages allow at most 255 options per Choice and
//! 2 to 10 levels per Score; the OpenAPI document has the same upper
//! bounds and a lower bound of 1. The builder follows the pages and adds a
//! minimum of 2 options, since one decides nothing. The upper bounds are
//! the hosted server's own (a 422 from its schema since 2026-10-25, a 400
//! before); the lower ones are this crate's, as the server answers one
//! option or one level with probability 1
//! (`docs/project/verification/hosted-typesafe.md`). An empty id is refused as the
//! server refuses it, an empty option key because the server can choose
//! it, an answer the caller cannot name. An option key given twice is
//! refused, since one option would be lost to the other; so is a number or
//! a boolean as instructions, a Noul criterion, an option's description or
//! a Score level, which the API refuses (it takes text, an object, an array
//! or null). Refusing before sending costs no round trip and no 422;
//! `tests/contract.rs` pins the limits against the schema.

use indexmap::IndexMap;
use indexmap::map::Entry;
use std::hash::Hash;
use std::marker::PhantomData;

use serde::Serialize;
use serde_json::Value;

use crate::answer::{BoundingBox, Choice, FromAnswer, Noul, Score};
use crate::error::{Error, Result};

/// The reference page's maximum options per Choice (module docs, `# Limits are checked here`).
pub const MAX_CHOICE_OPTIONS: usize = 255;
/// The reference page's maximum levels per Score; the minimum is 2 (module docs).
pub const MAX_SCORE_LEVELS: usize = 10;

/// An instructions object goes out with its keys sorted, whatever
/// `serde_json`'s `preserve_order` (the `mcp` feature turns it on) did to the map.
fn sorted<S: serde::Serializer>(
    value: &Value,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    let mut value = value.clone();
    value.sort_all_objects();
    value.serialize(serializer)
}

/// One question, as sent on the wire.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Yes/no; the answer is the probability of yes.
    Noul {
        /// What to decide; `null` when the criteria say it all.
        #[serde(serialize_with = "sorted")]
        instructions: Value,
        /// Optional meaning of yes and no.
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// One option from a defined set.
    Choice {
        /// What to decide; `null` when the option descriptions say it all.
        #[serde(serialize_with = "sorted")]
        instructions: Value,
        /// Option key to description (`null` allowed), in the order the model sees them.
        criteria: IndexMap<String, Value>,
    },
    /// A position along ordered levels.
    Score {
        /// What to rate; `null` when the levels say it all.
        #[serde(serialize_with = "sorted")]
        instructions: Value,
        /// Ordered level descriptions, lowest first.
        criteria: Vec<Value>,
    },
    /// Where something is in an image (module docs, `# A Box needs an image`).
    Box {
        /// What to locate, and in which image; never `null`, there are no criteria.
        #[serde(serialize_with = "sorted")]
        instructions: Value,
    },
}

impl Question {
    /// The wire `type`: `noul`, `choice`, `score` or `box` ([`crate::Response::verify`]'s `expected`).
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
            Self::Box { .. } => "box",
        }
    }

    /// The instructions, whatever the primitive.
    pub const fn instructions(&self) -> &Value {
        match self {
            Self::Noul { instructions, .. }
            | Self::Choice { instructions, .. }
            | Self::Score { instructions, .. }
            | Self::Box { instructions } => instructions,
        }
    }

    /// [`Question::instructions`], mutable.
    pub const fn instructions_mut(&mut self) -> &mut Value {
        match self {
            Self::Noul { instructions, .. }
            | Self::Choice { instructions, .. }
            | Self::Score { instructions, .. }
            | Self::Box { instructions } => instructions,
        }
    }
}

/// What yes and no mean for a Noul question.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct NoulCriteria {
    /// Meaning of a value near 1.
    #[serde(rename = "true", skip_serializing_if = "Option::is_none")]
    pub yes: Option<Value>,
    /// Meaning of a value near 0.
    #[serde(rename = "false", skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,
}

impl NoulCriteria {
    /// Describe both outcomes.
    pub fn new(yes: impl Into<Value>, no: impl Into<Value>) -> Self {
        Self {
            yes: Some(yes.into()),
            no: Some(no.into()),
        }
    }
}

/// The closed set of options for a typed [`Choice`]: the wire `criteria` come
/// from [`Options::ALL`], the answer maps back through [`Options::from_key`].
pub trait Options: Copy + Eq + Hash + std::fmt::Debug + Send + Sync + 'static {
    /// Every option, in the order the model sees them.
    const ALL: &'static [Self];
    /// The key sent to and returned by the API.
    fn key(self) -> &'static str;
    /// The rubric description for this option, or `None` for no detail.
    fn describe(self) -> Option<&'static str>;
    /// Parse a key returned by the API.
    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|o| o.key() == key)
    }
}

/// Define an enum implementing [`Options`]; each variant is
/// `Name = "wire_key" => description`, the description a `&str` or `None`.
///
/// ```
/// judgment::options! {
///     /// Which team owns first response.
///     pub enum Team {
///         Platform = "platform" => "Kubernetes, CI/CD, internal developer platform",
///         Database = "database" => "PostgreSQL, Redis, storage",
///         None = "none_of_these" => "Not clearly any listed team",
///     }
/// }
/// # use judgment::Options;
/// assert_eq!(Team::from_key("database"), Some(Team::Database));
/// assert_eq!(Team::ALL.len(), 3);
/// ```
#[macro_export]
macro_rules! options {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $(#[$vmeta:meta])*
                $variant:ident = $key:literal => $desc:expr
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl $crate::Options for $name {
            const ALL: &'static [Self] = &[ $( Self::$variant, )+ ];

            fn key(self) -> &'static str {
                match self { $( Self::$variant => $key, )+ }
            }

            fn describe(self) -> Option<&'static str> {
                match self { $( Self::$variant => $crate::question::__desc($desc), )+ }
            }
        }
    };
}

/// Implementation detail of [`options!`]: accepts `&str` or `Option<&str>`.
#[doc(hidden)]
pub fn __desc(d: impl IntoDescription) -> Option<&'static str> {
    d.into_description()
}

/// Implementation detail of [`options!`].
#[doc(hidden)]
pub trait IntoDescription {
    /// Convert to an optional description.
    fn into_description(self) -> Option<&'static str>;
}
impl IntoDescription for &'static str {
    fn into_description(self) -> Option<&'static str> {
        Some(self)
    }
}
impl IntoDescription for Option<&'static str> {
    fn into_description(self) -> Option<&'static str> {
        self
    }
}

/// A typed reference to one question; `A` ([`Noul`], [`Choice<O>`], [`Score`]) is a marker only.
#[derive(Debug, Clone)]
pub struct Handle<A> {
    id: String,
    _answer: PhantomData<fn() -> A>,
}

impl<A> Handle<A> {
    /// The question id, as used in the request and response maps.
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// The `questions` map of one request, in the order the model sees them.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Questions {
    map: IndexMap<String, Question>,
}

impl Questions {
    /// Empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of questions.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True when no question was added.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Borrow a question by id.
    pub fn get(&self, id: &str) -> Option<&Question> {
        self.map.get(id)
    }

    /// The question ids, in wire order.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }

    /// The questions with their ids, in wire order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Question)> {
        self.map.iter().map(|(k, q)| (k.as_str(), q))
    }

    /// Add a yes/no question; [`Error::InvalidQuestion`] when neither
    /// `instructions` nor `criteria` say anything, or either holds a number
    /// or a boolean (module docs).
    pub fn noul(
        &mut self,
        id: impl Into<String>,
        instructions: impl Into<Value>,
        criteria: Option<NoulCriteria>,
    ) -> Result<Handle<Noul>> {
        self.push(
            id.into(),
            Question::Noul {
                instructions: instructions.into(),
                criteria,
            },
        )
    }

    /// Add a Choice over a Rust enum implementing [`Options`];
    /// [`Error::InvalidQuestion`] when two variants share a key.
    pub fn choice<O: Options>(
        &mut self,
        id: impl Into<String>,
        instructions: impl Into<Value>,
    ) -> Result<Handle<Choice<O>>> {
        let id = id.into();
        let criteria = option_map(
            &id,
            O::ALL
                .iter()
                .map(|o| (o.key().to_owned(), o.describe().map(str::to_owned))),
        )?;
        self.push(
            id,
            Question::Choice {
                instructions: instructions.into(),
                criteria,
            },
        )
    }

    /// Add a Choice from runtime `(key, description)` pairs, in their order;
    /// [`Error::InvalidQuestion`] when a key is given twice. The answer is a
    /// [`Choice<String>`]: checking that the returned key is known is the caller's.
    pub fn dynamic_choice(
        &mut self,
        id: impl Into<String>,
        instructions: impl Into<Value>,
        options: impl IntoIterator<Item = (String, Option<String>)>,
    ) -> Result<Handle<Choice<String>>> {
        let id = id.into();
        let criteria = option_map(&id, options)?;
        self.push(
            id,
            Question::Choice {
                instructions: instructions.into(),
                criteria,
            },
        )
    }

    /// Add a Score over ordered levels, lowest first; the order is the meaning of the [`Score`].
    /// [`Error::InvalidQuestion`] outside 2 to 10 levels, for a null level, or
    /// for instructions or a level that is a number or a boolean (module docs).
    pub fn score(
        &mut self,
        id: impl Into<String>,
        instructions: impl Into<Value>,
        levels: impl IntoIterator<Item = impl Into<Value>>,
    ) -> Result<Handle<Score>> {
        self.push(
            id.into(),
            Question::Score {
                instructions: instructions.into(),
                criteria: levels.into_iter().map(Into::into).collect(),
            },
        )
    }

    /// Add a Box: where what `instructions` describe is in an image the
    /// request carries, read as a [`BoundingBox`]. Name the target and the
    /// image ("the coffee mug in `photo`"); [`Error::InvalidQuestion`] for
    /// null instructions, since a Box has no criteria to carry the
    /// question, and for a number or a boolean. Needs Jev 1.14 or later on the hosted API (`jev-latest`,
    /// or `jev-1.14.0` pinned).
    pub fn r#box(
        &mut self,
        id: impl Into<String>,
        instructions: impl Into<Value>,
    ) -> Result<Handle<BoundingBox>> {
        self.push(
            id.into(),
            Question::Box {
                instructions: instructions.into(),
            },
        )
    }

    /// Add a wire-shaped question, same checks, no handle: read it via [`Questions::handle`].
    pub fn add(&mut self, id: impl Into<String>, question: Question) -> Result<()> {
        self.push::<()>(id.into(), question).map(drop)
    }

    /// A typed handle to a question added by id; `None` if absent, not of
    /// `A`'s primitive, or one `A` cannot always read ([`FromAnswer::fits`]:
    /// a [`Choice<O>`] needs the question to offer exactly `O`'s options).
    pub fn handle<A: FromAnswer>(&self, id: &str) -> Option<Handle<A>> {
        let question = self.map.get(id)?;
        (question.kind() == A::KIND && A::fits(question)).then(|| Handle {
            id: id.to_owned(),
            _answer: PhantomData,
        })
    }

    /// Validate, then insert: every way in goes through here.
    fn push<A>(&mut self, id: String, question: Question) -> Result<Handle<A>> {
        validate(&id, &question)?;
        self.insert(id, question)
    }

    fn insert<A>(&mut self, id: String, question: Question) -> Result<Handle<A>> {
        if id.is_empty() {
            return Err(Error::InvalidQuestion {
                id,
                reason: "a question id cannot be empty".to_owned(),
            });
        }
        match self.map.entry(id.clone()) {
            Entry::Occupied(_) => Err(Error::DuplicateQuestionId(id)),
            Entry::Vacant(slot) => {
                slot.insert(question);
                Ok(Handle {
                    id,
                    _answer: PhantomData,
                })
            }
        }
    }
}

/// The questions with their ids, in wire order, by value.
impl IntoIterator for Questions {
    type Item = (String, Question);
    type IntoIter = indexmap::map::IntoIter<String, Question>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}

/// A Choice's options in the order given, refusing a key given twice: a
/// map would keep one description and the model would never be offered the
/// other, so the option could never be chosen.
fn option_map(
    id: &str,
    options: impl IntoIterator<Item = (String, Option<String>)>,
) -> Result<IndexMap<String, Value>> {
    let mut criteria = IndexMap::new();
    for (key, description) in options {
        if criteria.contains_key(&key) {
            return Err(Error::InvalidQuestion {
                id: id.to_owned(),
                reason: format!("option key {key:?} appears twice"),
            });
        }
        criteria.insert(key, description.map_or(Value::Null, Value::from));
    }
    Ok(criteria)
}

/// The checks every question gets (module docs, `# Limits are checked here`).
pub(crate) fn validate(id: &str, question: &Question) -> Result<()> {
    validate_with(id, question, 2)
}

/// [`validate`] allowing `min_options`: the static part of a Choice whose other options come
/// per request (`crate::jud`) may hold fewer than the 2 the built request is checked for.
pub(crate) fn validate_with(id: &str, question: &Question, min_options: usize) -> Result<()> {
    let refuse = |reason: String| {
        Err(Error::InvalidQuestion {
            id: id.to_owned(),
            reason,
        })
    };
    // The API takes text, an object or an array (or null where a value may
    // be left out); a number or a boolean is refused there, so here.
    let scalar = |value: &Value| matches!(value, Value::Bool(_) | Value::Number(_));
    let instructions = match question {
        Question::Noul { instructions, .. }
        | Question::Choice { instructions, .. }
        | Question::Score { instructions, .. }
        | Question::Box { instructions } => instructions,
    };
    if scalar(instructions) {
        return refuse(format!(
            "instructions must be text, an object, an array or null, not {instructions}"
        ));
    }
    match question {
        Question::Noul {
            instructions,
            criteria,
        } => {
            let criteria_say_nothing = criteria.as_ref().is_none_or(|c| {
                c.yes.as_ref().is_none_or(Value::is_null)
                    && c.no.as_ref().is_none_or(Value::is_null)
            });
            if instructions.is_null() && criteria_say_nothing {
                return refuse(
                    "a Noul needs instructions or criteria: the id is never shown to the model"
                        .to_owned(),
                );
            }
            if let Some(criteria) = criteria {
                for (side, value) in [("true", &criteria.yes), ("false", &criteria.no)] {
                    if value.as_ref().is_some_and(scalar) {
                        return refuse(format!(
                            "criteria `{side}` must be text, an object, an array or null"
                        ));
                    }
                }
            }
        }
        Question::Choice { criteria, .. } => {
            if criteria.len() < min_options {
                return refuse("a Choice needs at least 2 options".to_owned());
            }
            if criteria.len() > MAX_CHOICE_OPTIONS {
                return refuse(format!(
                    "a Choice allows at most {MAX_CHOICE_OPTIONS} options"
                ));
            }
            if criteria.contains_key("") {
                return refuse("a Choice option key cannot be empty".to_owned());
            }
            if let Some(key) = criteria.iter().find_map(|(k, d)| scalar(d).then_some(k)) {
                return refuse(format!(
                    "option {key:?}'s description must be text, an object, an array or null"
                ));
            }
        }
        Question::Score { criteria, .. } => {
            // The API gives a null level no meaning; a lenient backend echoes it into the legend.
            if let Some(index) = criteria.iter().position(Value::is_null) {
                return refuse(format!(
                    "Score level {index} is null; every level needs a description"
                ));
            }
            if let Some(index) = criteria.iter().position(scalar) {
                return refuse(format!(
                    "Score level {index} must be text, an object or an array"
                ));
            }
            if criteria.len() < 2 || criteria.len() > MAX_SCORE_LEVELS {
                return refuse(format!(
                    "a Score needs between 2 and {MAX_SCORE_LEVELS} levels, got {}",
                    criteria.len()
                ));
            }
        }
        Question::Box { instructions } => {
            if instructions.is_null() {
                return refuse(
                    "a Box needs instructions naming what to locate and in which image: it has no criteria"
                        .to_owned(),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use serde_json::json;

    #[test]
    fn an_empty_question_id_is_refused_before_sending() {
        let mut q = Questions::new();
        let err = q.noul("", "Is `m` a greeting?", None).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidQuestion { id, reason } if id.is_empty() && reason.contains("empty")),
            "{err:?}"
        );
        assert!(q.is_empty(), "a refused question is not added");
    }

    #[test]
    fn an_empty_option_key_is_refused_before_sending() {
        let mut q = Questions::new();
        let err = q
            .dynamic_choice("c", "pick", [(String::new(), None), ("a".to_owned(), None)])
            .unwrap_err();
        assert!(
            matches!(&err, Error::InvalidQuestion { id, reason } if id == "c" && reason.contains("option key")),
            "{err:?}"
        );
        assert!(q.is_empty(), "a refused question is not added");
    }

    options! {
        enum Colour {
            Red = "red" => "Warm",
            Blue = "blue" => None,
        }
    }

    options! {
        enum Twice {
            A = "x" => "First",
            B = "x" => "Second",
            C = "y" => None,
        }
    }

    /// A key given twice is refused, not merged: the map would keep one
    /// description and the other option could never be chosen.
    #[test]
    fn a_repeated_option_key_is_refused() {
        let mut q = Questions::new();
        let err = q
            .dynamic_choice(
                "d",
                "Which?",
                [("a", "first"), ("b", "b"), ("a", "again")]
                    .map(|(k, d)| (k.to_owned(), Some(d.to_owned()))),
            )
            .unwrap_err();
        assert!(
            matches!(&err, Error::InvalidQuestion { id, reason } if id == "d" && reason == "option key \"a\" appears twice"),
            "{err:?}"
        );
        let err = q.choice::<Twice>("e", "Which?").unwrap_err();
        assert!(
            matches!(&err, Error::InvalidQuestion { id, reason } if id == "e" && reason.contains("\"x\" appears twice")),
            "{err:?}"
        );
        assert!(q.is_empty(), "a refused question is not added");
    }

    /// What the API refuses is refused here: a number or a boolean where it
    /// takes text, an object, an array or null.
    #[test]
    fn a_number_or_a_boolean_the_api_refuses_is_refused() {
        let mut q = Questions::new();
        let reason = |err: Error| match err {
            Error::InvalidQuestion { reason, .. } => reason,
            other => panic!("{other:?}"),
        };
        let err = q.noul("a", 42, None).unwrap_err();
        assert!(reason(err).starts_with("instructions must be text"));
        let err = q
            .noul("b", Value::Null, Some(NoulCriteria::new(true, "no")))
            .unwrap_err();
        assert_eq!(
            reason(err),
            "criteria `true` must be text, an object, an array or null"
        );
        let err = q
            .score("c", "How bad?", vec![json!("low"), json!(2)])
            .unwrap_err();
        assert_eq!(
            reason(err),
            "Score level 1 must be text, an object or an array"
        );
        let err = q
            .noul("b", Value::Null, Some(NoulCriteria::new("yes", 0)))
            .unwrap_err();
        assert_eq!(
            reason(err),
            "criteria `false` must be text, an object, an array or null"
        );
        for err in [
            q.score("c", true, ["low", "high"]).map(|_| ()),
            q.r#box("d", 7).map(|_| ()),
            q.dynamic_choice("e", 1.5, [("a".to_owned(), None), ("b".to_owned(), None)])
                .map(|_| ()),
        ] {
            assert!(reason(err.unwrap_err()).starts_with("instructions must be text"));
        }
        let mut described = Questions::new();
        let err = described
            .add(
                "f",
                Question::Choice {
                    instructions: json!("pick"),
                    criteria: IndexMap::from([
                        ("a".to_owned(), json!(1)),
                        ("b".to_owned(), json!("B")),
                    ]),
                },
            )
            .unwrap_err();
        assert_eq!(
            reason(err),
            "option \"a\"'s description must be text, an object, an array or null"
        );
        assert!(q.is_empty(), "a refused question is not added");
    }

    #[test]
    fn choice_criteria_come_from_the_enum() {
        let mut q = Questions::new();
        let h = q.choice::<Colour>("colour", "Which colour?").unwrap();
        assert_eq!(h.id(), "colour");
        let json = serde_json::to_value(&q).unwrap();
        assert_eq!(
            json["colour"],
            serde_json::json!({
                "type": "choice",
                "instructions": "Which colour?",
                "criteria": { "red": "Warm", "blue": null }
            })
        );
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let mut q = Questions::new();
        q.noul("a", "x?", None).unwrap();
        assert!(matches!(
            q.noul("a", "y?", None),
            Err(Error::DuplicateQuestionId(id)) if id == "a"
        ));
    }

    #[test]
    fn score_level_bounds_are_enforced() {
        let mut q = Questions::new();
        assert!(q.score("one", "?", ["only"]).is_err());
        assert!(q.score("many", "?", vec!["l"; 11]).is_err());
        assert!(q.score("ok", "?", ["low", "high"]).is_ok());
    }

    #[test]
    fn a_null_score_level_is_refused_before_the_wire() {
        let mut q = Questions::new();
        let err = q
            .score("s", "?", vec![json!("low"), json!(null), json!("high")])
            .unwrap_err();
        assert!(
            matches!(&err, Error::InvalidQuestion { id, reason } if id == "s" && reason.contains("level 1 is null")),
            "{err}"
        );
    }

    #[test]
    fn instructions_objects_are_sent_with_sorted_keys_in_every_build() {
        let mut q = Questions::new();
        q.noul("n", json!({"zebra": "z", "apple": {"y": 1, "b": 2}}), None)
            .unwrap();
        let wire = serde_json::to_string(&q).unwrap();
        assert!(
            wire.contains(r#""instructions":{"apple":{"b":2,"y":1},"zebra":"z"}"#),
            "{wire}"
        );
    }

    #[test]
    fn null_instructions_are_sent_as_null() {
        let mut q = Questions::new();
        q.choice::<Colour>("unit", ()).unwrap();
        q.score("none", None::<&str>, ["low", "high"]).unwrap();
        q.dynamic_choice(
            "value",
            Value::Null,
            [
                ("a".to_owned(), Some("A".to_owned())),
                ("b".to_owned(), None),
            ],
        )
        .unwrap();
        q.noul(
            "option",
            None::<String>,
            Some(NoulCriteria::new("spam", "not spam")),
        )
        .unwrap();
        let json = serde_json::to_value(&q).unwrap();
        for id in ["unit", "none", "value", "option"] {
            let question = json[id].as_object().unwrap();
            assert_eq!(
                question.get("instructions"),
                Some(&Value::Null),
                "{id}: the key is present and null"
            );
        }
        assert_eq!(
            json["none"],
            serde_json::json!({"criteria": ["low", "high"], "instructions": null, "type": "score"})
        );
    }

    #[test]
    fn a_noul_with_neither_instructions_nor_criteria_is_refused() {
        let mut q = Questions::new();
        let says_nothing = [
            ("none", None),
            ("empty", Some(NoulCriteria::default())),
            (
                "all_null",
                Some(NoulCriteria {
                    yes: Some(Value::Null),
                    no: Some(Value::Null),
                }),
            ),
            (
                "one_null",
                Some(NoulCriteria {
                    yes: Some(Value::Null),
                    no: None,
                }),
            ),
        ];
        for (form, instructions) in [("unit", Value::from(())), ("null", Value::Null)] {
            for (shape, criteria) in &says_nothing {
                let id = format!("{form}_{shape}");
                let err = q
                    .noul(id.clone(), instructions.clone(), criteria.clone())
                    .unwrap_err();
                assert!(
                    matches!(&err, Error::InvalidQuestion { id: got, reason }
                        if *got == id && reason == "a Noul needs instructions or criteria: the id is never shown to the model"),
                    "{id}: {err}"
                );
            }
        }
        assert!(q.is_empty(), "a refused question is not added");

        q.noul(
            "spam",
            (),
            Some(NoulCriteria::new(
                "unsolicited advertising",
                "a real message",
            )),
        )
        .unwrap();
        q.noul(
            "spam_yes_only",
            (),
            Some(NoulCriteria {
                yes: Some("unsolicited advertising".into()),
                no: None,
            }),
        )
        .unwrap();
        q.noul(
            "spam_no_only",
            (),
            Some(NoulCriteria {
                yes: Some(Value::Null),
                no: Some("a real message".into()),
            }),
        )
        .unwrap();
        q.noul("urgent", "Is `message` urgent?", None).unwrap();
        q.noul(
            "urgent_empty_criteria",
            "Is `message` urgent?",
            Some(NoulCriteria::default()),
        )
        .unwrap();
        assert_eq!(q.len(), 5);
    }

    #[test]
    fn question_kind_names_the_primitive() {
        let mut q = Questions::new();
        q.noul("n", "?", None).unwrap();
        q.choice::<Colour>("c", "?").unwrap();
        q.score("s", "?", ["low", "high"]).unwrap();
        q.r#box("b", "the mug in `photo`").unwrap();
        let json = serde_json::to_value(&q).unwrap();
        for (id, question) in q.iter() {
            assert_eq!(json[id]["type"], question.kind(), "{id}");
        }
        let kinds: Vec<&str> = q.iter().map(|(_, question)| question.kind()).collect();
        assert_eq!(kinds, ["noul", "choice", "score", "box"]);
        let ids: Vec<&str> = q.ids().collect();
        assert_eq!(ids, ["n", "c", "s", "b"]);
    }

    #[test]
    fn a_handle_is_refused_for_a_choice_its_enum_cannot_read() {
        crate::options! {
            enum Two {
                A = "a" => None,
                B = "b" => None,
                C = "c" => None,
            }
        }
        let mut q = Questions::new();
        let opt = |keys: &[&str]| {
            keys.iter()
                .map(|k| ((*k).to_owned(), None))
                .collect::<Vec<_>>()
        };
        q.dynamic_choice("same", "?", opt(&["a", "b", "c"]))
            .unwrap();
        q.dynamic_choice("extra", "?", opt(&["a", "b", "c", "d"]))
            .unwrap();
        q.dynamic_choice("subset", "?", opt(&["a", "b"])).unwrap();
        q.dynamic_choice("other", "?", opt(&["a", "b", "x"]))
            .unwrap();
        assert!(q.handle::<Choice<Two>>("same").is_some());
        for id in ["extra", "subset", "other"] {
            assert!(q.handle::<Choice<Two>>(id).is_none(), "{id}");
            assert!(q.handle::<Choice<String>>(id).is_some(), "{id}");
        }
    }

    #[test]
    fn a_box_is_its_instructions_alone_and_never_null() {
        let mut q = Questions::new();
        let handle = q.r#box("mug", "the coffee mug in `photo`").unwrap();
        assert_eq!(handle.id(), "mug");
        assert_eq!(
            serde_json::to_string(&q).unwrap(),
            r#"{"mug":{"type":"box","instructions":"the coffee mug in `photo`"}}"#,
            "no criteria key at all"
        );
        for null in [Value::Null, Value::from(())] {
            let err = q.r#box("none", null).unwrap_err();
            assert!(
                matches!(&err, Error::InvalidQuestion { id, reason } if id == "none" && reason.contains("no criteria")),
                "{err}"
            );
        }
        assert_eq!(q.len(), 1, "a refused question is not added");
        assert!(q.handle::<BoundingBox>("mug").is_some());
        assert!(q.handle::<Noul>("mug").is_none());
        assert_eq!(
            q.get("mug").unwrap().instructions(),
            "the coffee mug in `photo`"
        );
    }

    #[test]
    fn noul_criteria_serialise_with_true_false_keys() {
        let mut q = Questions::new();
        q.noul(
            "u",
            "Urgent?",
            Some(NoulCriteria::new("time-sensitive", "no urgency")),
        )
        .unwrap();
        let json = serde_json::to_value(&q).unwrap();
        assert_eq!(json["u"]["criteria"]["true"], "time-sensitive");
        assert_eq!(json["u"]["criteria"]["false"], "no urgency");
    }
}
