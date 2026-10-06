//! Questions: the three System One primitives, a request builder, and the
//! typed handles that tie each question to the shape of its answer
//! (`docs/design.md` draws the mechanism).
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
//! The HTTP API reference page allows at most 255 options per Choice and
//! 2 to 10 levels per Score; the OpenAPI document is looser. The builder
//! follows the page and adds a minimum of 2 options, since one decides
//! nothing. The upper bounds are the hosted server's own (a 400 past
//! them); the lower ones are this crate's, as the server answers one
//! option or one level with probability 1
//! (`docs/verification/hosted-typesafe.md`). An empty id is refused as the
//! server refuses it, an empty option key because the server can choose
//! it, an answer the caller cannot name. Refusing before sending costs no
//! round trip; `tests/contract.rs` pins the limits against the schema.

use indexmap::IndexMap;
use indexmap::map::Entry;
use std::hash::Hash;
use std::marker::PhantomData;

use serde::Serialize;
use serde_json::Value;

use crate::answer::{Choice, FromAnswer, Noul, Score};
use crate::error::{Error, Result};

/// The reference page's maximum options per Choice (module docs, `# Limits are checked here`).
pub const MAX_CHOICE_OPTIONS: usize = 255;
/// The reference page's maximum levels per Score; the minimum is 2 (module docs).
pub const MAX_SCORE_LEVELS: usize = 10;

/// One question, as sent on the wire.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Yes/no; the answer is the probability of yes.
    Noul {
        /// What to decide; `null` when the criteria say it all.
        instructions: Value,
        /// Optional meaning of yes and no.
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// One option from a defined set.
    Choice {
        /// What to decide; `null` when the option descriptions say it all.
        instructions: Value,
        /// Option key to description (`null` allowed), in the order the model sees them.
        criteria: IndexMap<String, Value>,
    },
    /// A position along ordered levels.
    Score {
        /// What to rate; `null` when the levels say it all.
        instructions: Value,
        /// Ordered level descriptions, lowest first.
        criteria: Vec<Value>,
    },
}

impl Question {
    /// The wire `type`: `noul`, `choice` or `score` ([`crate::Response::verify`]'s `expected`).
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
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
    /// `instructions` nor `criteria` say anything (module docs).
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

    /// Add a Choice over a Rust enum implementing [`Options`].
    pub fn choice<O: Options>(
        &mut self,
        id: impl Into<String>,
        instructions: impl Into<Value>,
    ) -> Result<Handle<Choice<O>>> {
        let criteria: IndexMap<String, Value> = O::ALL
            .iter()
            .map(|o| {
                (
                    o.key().to_owned(),
                    o.describe().map_or(Value::Null, Value::from),
                )
            })
            .collect();
        self.push(
            id.into(),
            Question::Choice {
                instructions: instructions.into(),
                criteria,
            },
        )
    }

    /// Add a Choice from runtime `(key, description)` pairs. The answer is a
    /// [`Choice<String>`]: checking that the returned key is known is the caller's.
    pub fn dynamic_choice(
        &mut self,
        id: impl Into<String>,
        instructions: impl Into<Value>,
        options: impl IntoIterator<Item = (String, Option<String>)>,
    ) -> Result<Handle<Choice<String>>> {
        let criteria: IndexMap<String, Value> = options
            .into_iter()
            .map(|(k, d)| (k, d.map_or(Value::Null, Value::from)))
            .collect();
        self.push(
            id.into(),
            Question::Choice {
                instructions: instructions.into(),
                criteria,
            },
        )
    }

    /// Add a Score over ordered levels, lowest first; the order is the meaning of the [`Score`].
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

    /// Add a wire-shaped question, same checks, no handle: read it via [`Questions::handle`].
    pub fn add(&mut self, id: impl Into<String>, question: Question) -> Result<()> {
        self.push::<()>(id.into(), question).map(drop)
    }

    /// A typed handle to a question added by id; `None` if absent or not of `A`'s primitive.
    pub fn handle<A: FromAnswer>(&self, id: &str) -> Option<Handle<A>> {
        let question = self.map.get(id)?;
        (question.kind() == A::KIND).then(|| Handle {
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
        }
        Question::Score { criteria, .. } => {
            // The API gives a null level no meaning; a lenient backend echoes it into the legend.
            if let Some(index) = criteria.iter().position(Value::is_null) {
                return refuse(format!(
                    "Score level {index} is null; every level needs a description"
                ));
            }
            if criteria.len() < 2 || criteria.len() > MAX_SCORE_LEVELS {
                return refuse(format!(
                    "a Score needs between 2 and {MAX_SCORE_LEVELS} levels, got {}",
                    criteria.len()
                ));
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
            serde_json::to_string(&json["none"]).unwrap(),
            r#"{"criteria":["low","high"],"instructions":null,"type":"score"}"#
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
        let json = serde_json::to_value(&q).unwrap();
        for (id, question) in q.iter() {
            assert_eq!(json[id]["type"], question.kind(), "{id}");
        }
        let kinds: Vec<&str> = q.iter().map(|(_, question)| question.kind()).collect();
        assert_eq!(kinds, ["noul", "choice", "score"]);
        let ids: Vec<&str> = q.ids().collect();
        assert_eq!(ids, ["n", "c", "s"]);
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
