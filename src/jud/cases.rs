//! The `cases` kind: labelled states a rubric is graded on.

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::rubric::level_index;
use super::{
    Error, Result, Rubric, Supplied, check_name, check_version, extensions, from_text,
    require_minor, some, to_yaml, version_value,
};
use crate::answer::Response;
use crate::eval::{Judgment, canonical};
use crate::question::{Question, Questions};

/// Labelled states a rubric is graded on ([`grade`]) and its gates are
/// tuned on (`docs/jud.md`, cases).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cases {
    /// A name for the set; [`fingerprint`](Self::fingerprint) is exact.
    pub id: Option<String>,
    /// The rubric the labels are for, by id or fingerprint ([`Cases::bind`]).
    pub rubric: Option<String>,
    /// Where the cases came from, for the person reading them.
    pub description: Option<String>,
    /// The cases, in document order.
    pub cases: Vec<Case>,
    /// The top-level `x-` keys (1.1), kept as read and part of no fingerprint.
    pub extensions: IndexMap<String, Value>,
}

/// One labelled state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// A name stable across edits; without one, the position ([`Case::name`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Any JSON; an array is a conversation, one element per turn ([`turns`]).
    pub state: Value,
    /// The right answer, by question id; a question left out is not graded.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub expect: IndexMap<String, Expect>,
    /// Options supplied for this case's request (1.1), as [`Rubric::lower`]
    /// takes them: with them, a case is a complete request.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub options: Supplied,
    /// Free labels for slicing a report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Why the label is what it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// What a case expects of one question; which form fits which primitive is
/// `docs/jud.md`, cases, checked by [`Cases::bind`] and [`grade`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Expect {
    /// A Noul's answer.
    Bool(bool),
    /// A Score's level, by index.
    Index(u64),
    /// A Choice's option key, or a Score's level by its text.
    Text(String),
    /// A Noul over a conversation: true from a turn on, or never.
    FromTurn(FromTurn),
}

/// `{from_turn: n}`: a Noul over a conversation is true from turn `n`
/// (zero-based) on; `{from_turn: null}` says never.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FromTurn {
    /// The zero-based turn the answer becomes true at; `None` for never.
    #[serde(deserialize_with = "nullable")]
    pub from_turn: Option<u64>,
}

/// Required even when `null`: serde would otherwise read a missing key as `None`.
fn nullable<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<u64>, D::Error> {
    Option::<u64>::deserialize(deserializer)
}

/// One turn of a conversation case ([`Case::per_turn`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    /// Zero-based index of the last turn included.
    pub index: usize,
    /// The case as a model sees it at this turn.
    pub case: Case,
}

/// The turns of a conversation: the state's elements when it is an array.
pub fn turns(state: &Value) -> Option<&[Value]> {
    state.as_array().map(Vec::as_slice)
}

impl Case {
    /// A case over `state` with no labels yet.
    pub fn new(state: Value) -> Self {
        Self {
            id: None,
            state,
            expect: IndexMap::new(),
            options: IndexMap::new(),
            tags: Vec::new(),
            note: None,
        }
    }

    /// [`Rubric::lower`] over the case's state and options.
    pub fn request(&self, rubric: &Rubric) -> Result<Questions> {
        rubric.lower(&self.state, &self.options)
    }

    /// The case's id, or `#index` for a case without one.
    pub fn name(&self, index: usize) -> String {
        self.id.clone().unwrap_or_else(|| format!("#{index}"))
    }

    /// A conversation case at each of its turns (`docs/jud.md`,
    /// Conversations); empty when the state is not a conversation.
    pub fn per_turn(&self) -> Vec<Turn> {
        let Some(all) = turns(&self.state) else {
            return Vec::new();
        };
        let last = all.len().saturating_sub(1);
        (0..all.len())
            .map(|index| {
                let expect = self
                    .expect
                    .iter()
                    .filter_map(|(id, expect)| match expect {
                        Expect::FromTurn(FromTurn { from_turn }) => Some((
                            id.clone(),
                            Expect::Bool(
                                from_turn
                                    .is_some_and(|n| usize::try_from(n).is_ok_and(|n| index >= n)),
                            ),
                        )),
                        other if index == last => Some((id.clone(), other.clone())),
                        _ => None,
                    })
                    .collect();
                Turn {
                    index,
                    case: Self {
                        id: self.id.clone(),
                        state: Value::Array(all[..=index].to_vec()),
                        expect,
                        options: self.options.clone(),
                        tags: self.tags.clone(),
                        note: self.note.clone(),
                    },
                }
            })
            .collect()
    }
}

#[derive(Deserialize)]
struct RawCases {
    #[serde(default)]
    jud: Option<Value>,
    kind: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    rubric: Option<String>,
    #[serde(default)]
    description: Option<String>,
    cases: Vec<RawCase>,
    #[serde(flatten)]
    rest: IndexMap<String, Value>,
}

/// A case as read; `options` counts by its presence ([`some`]).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCase {
    #[serde(default)]
    id: Option<String>,
    state: Value,
    #[serde(default)]
    expect: IndexMap<String, Expect>,
    #[serde(default, deserialize_with = "some")]
    options: Option<Supplied>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Serialize)]
struct CasesDoc<'a> {
    jud: Value,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rubric: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    cases: &'a [Case],
    #[serde(flatten)]
    extensions: &'a IndexMap<String, Value>,
}

impl Cases {
    /// Parse a `cases` document, YAML or JSON. The labels are checked by
    /// [`Cases::bind`], since the document need not name its rubric.
    pub fn parse(text: &str) -> Result<Self> {
        let raw: RawCases = from_text(text)?;
        let declared = check_version(raw.jud.as_ref())?;
        if raw.kind != "cases" {
            return Err(Error::Kind { found: raw.kind });
        }
        let extensions = extensions(raw.rest, "cases")?;
        if raw.cases.is_empty() {
            return Err(Error::Invalid {
                field: "cases".to_owned(),
                reason: "a cases document needs at least one case".to_owned(),
            });
        }
        let feature = extensions.keys().next().cloned().or_else(|| {
            raw.cases
                .iter()
                .position(|c| c.options.is_some())
                .map(|i| format!("cases.{i}.options"))
        });
        require_minor(declared, feature)?;
        let cases = Self {
            id: raw.id,
            rubric: raw.rubric,
            description: raw.description,
            cases: raw
                .cases
                .into_iter()
                .map(|c| Case {
                    id: c.id,
                    state: c.state,
                    expect: c.expect,
                    options: c.options.unwrap_or_default(),
                    tags: c.tags,
                    note: c.note,
                })
                .collect(),
            extensions,
        };
        if let Some(id) = &cases.id {
            check_name("id", id)?;
        }
        for (index, case) in cases.cases.iter().enumerate() {
            if let Some(id) = &case.id {
                check_name(&format!("cases.{index}.id"), id).map_err(|e| Error::Case {
                    case: format!("#{index}"),
                    reason: e.to_string(),
                })?;
            }
            // A supplied option has the shape of a Choice's criteria entry.
            for (question, options) in &case.options {
                if let Some((key, _)) = options.iter().find(|(key, description)| {
                    key.is_empty() || matches!(description, Value::Bool(_) | Value::Number(_))
                }) {
                    return Err(Error::Case {
                        case: case.name(index),
                        reason: format!(
                            "`options.{question}`: an option needs a non-empty key and a description that is text, an object, an array or null (`{key}`)"
                        ),
                    });
                }
            }
            if let Some(dup) = cases.cases[..index]
                .iter()
                .find(|other| other.id.is_some() && other.id == case.id)
            {
                return Err(Error::Case {
                    case: case.name(index),
                    reason: format!("the same id as case {}", dup.name(index)),
                });
            }
        }
        Ok(cases)
    }

    /// The first 1.1 feature used, as a field path.
    fn feature(&self) -> Option<String> {
        self.extensions.keys().next().cloned().or_else(|| {
            self.cases
                .iter()
                .position(|c| !c.options.is_empty())
                .map(|i| format!("cases.{i}.options"))
        })
    }

    /// The cases as YAML, declaring the lowest version that reads it.
    pub fn to_yaml(&self) -> Result<String> {
        to_yaml(&CasesDoc {
            jud: version_value(u64::from(self.feature().is_some())),
            kind: "cases",
            id: self.id.as_deref(),
            rubric: self.rubric.as_deref(),
            description: self.description.as_deref(),
            cases: &self.cases,
            extensions: &self.extensions,
        })
    }

    /// The fingerprint of the `cases` array alone ([`canonical::fingerprint`]).
    pub fn fingerprint(&self) -> String {
        canonical::fingerprint(&serde_json::to_value(&self.cases).unwrap_or(Value::Null))
    }

    /// Check every case against `rubric`: its request lowers
    /// ([`Case::request`]) and every label names a question that request
    /// asks and fits it (`docs/jud.md`, cases); the document's `rubric`,
    /// when named, is the rubric's id or fingerprint.
    pub fn bind(&self, rubric: &Rubric) -> Result<()> {
        if let Some(named) = &self.rubric
            && *named != rubric.id
            && *named != rubric.fingerprint()
        {
            return Err(Error::Invalid {
                field: "rubric".to_owned(),
                reason: format!(
                    "the cases are for `{named}`, not for `{}` ({})",
                    rubric.id,
                    rubric.fingerprint()
                ),
            });
        }
        for (index, case) in self.cases.iter().enumerate() {
            let asked = request(rubric, case, index)?;
            for (id, expect) in &case.expect {
                expected_label(rubric, &asked, case, index, id, expect)?;
            }
        }
        Ok(())
    }
}

/// A case's request, its failure named by the case.
fn request(rubric: &Rubric, case: &Case, index: usize) -> Result<Questions> {
    case.request(rubric).map_err(|e| Error::Case {
        case: case.name(index),
        reason: format!("its request does not lower: {e}"),
    })
}

/// The label in [`Judgment::of_answer`]'s vocabulary (`yes`/`no`, an option
/// key, a level index as a string), checked against the request asked.
fn expected_label(
    rubric: &Rubric,
    asked: &Questions,
    case: &Case,
    index: usize,
    id: &str,
    expect: &Expect,
) -> Result<String> {
    let refuse = |reason: String| {
        Err(Error::Case {
            case: case.name(index),
            reason: format!("`expect.{id}`: {reason}"),
        })
    };
    let Some(question) = asked.get(id) else {
        let reason = match rubric.questions.get(id).and_then(|q| q.when.as_deref()) {
            Some(when) => format!(
                "the question is not asked for this state (`when: {when}` does not hold), so it cannot be labelled"
            ),
            None => "the rubric has no such question".to_owned(),
        };
        return refuse(reason);
    };
    match (question, expect) {
        (Question::Noul { .. }, Expect::Bool(yes)) => Ok(yes_no(*yes)),
        (Question::Noul { .. }, Expect::FromTurn(FromTurn { from_turn })) => {
            let Some(all) = turns(&case.state) else {
                return refuse(
                    "`from_turn` labels a conversation, and `state` is not an array".to_owned(),
                );
            };
            match from_turn {
                None => Ok(yes_no(false)),
                Some(n) => match usize::try_from(*n) {
                    Ok(n) if n < all.len() => Ok(yes_no(true)),
                    _ => refuse(format!(
                        "`from_turn: {n}` is past the last turn, {}; use null for never",
                        all.len().saturating_sub(1)
                    )),
                },
            }
        }
        (Question::Noul { .. }, _) => {
            refuse("a Noul expects true, false or {from_turn: n}".to_owned())
        }
        (Question::Choice { criteria, .. }, Expect::Text(key)) => {
            if criteria.contains_key(key) {
                Ok(key.clone())
            } else {
                refuse(format!("`{key}` is not one of the offered options"))
            }
        }
        (Question::Choice { .. }, _) => refuse("a Choice expects an option key".to_owned()),
        (Question::Score { criteria, .. }, Expect::Index(level)) => match usize::try_from(*level) {
            Ok(level) if level < criteria.len() => Ok(level.to_string()),
            _ => refuse(format!(
                "level {level} is out of range; the levels are 0 to {}",
                criteria.len() - 1
            )),
        },
        (Question::Score { criteria, .. }, Expect::Text(label)) => level_index(criteria, label)
            .map_or_else(
                || {
                    refuse(format!(
                        "`{label}` is neither a level's text nor a level index"
                    ))
                },
                |level| Ok(level.to_string()),
            ),
        (Question::Score { .. }, _) => {
            refuse("a Score expects a level index or a level's text".to_owned())
        }
    }
}

fn yes_no(yes: bool) -> String {
    if yes { "yes" } else { "no" }.to_owned()
}

/// Grade a response to `case` against its labels: one [`Judgment`] per
/// labelled question, in the case's order, after [`Response::verify`]
/// ([`Error::Response`]); `index` names a case without an id in errors. A
/// `from_turn` label grades the whole conversation; [`Case::per_turn`]
/// gives the turns to grade one by one.
pub fn grade(
    rubric: &Rubric,
    case: &Case,
    index: usize,
    response: &Response,
) -> Result<Vec<(String, Judgment)>> {
    let asked = request(rubric, case, index)?;
    response.verify(&asked).map_err(|source| Error::Response {
        source: Box::new(source),
    })?;
    case.expect
        .iter()
        .map(|(id, expect)| {
            let expected = expected_label(rubric, &asked, case, index, id, expect)?;
            let answer = response.answers.get(id).ok_or_else(|| Error::Response {
                source: Box::new(crate::Error::MissingAnswer {
                    id: id.clone(),
                    request_id: response.request_id.clone(),
                }),
            })?;
            Ok((id.clone(), Judgment::of_answer(answer, Some(&expected))))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;
    use crate::Fake;
    use crate::backend::SystemOne;
    const RUBRIC: &str = r"
jud: 1
kind: rubric
id: triage
questions:
  actionable:
    type: noul
    instructions: Does `message` ask for something to be done?
  owner:
    type: choice
    instructions: Who should handle `message`?
    criteria: {billing: null, support: null, none_of_these: null}
  tone:
    type: score
    instructions: How upset is the writer?
    criteria: [calm, annoyed, angry]
  handoff:
    type: noul
    instructions: Has the user asked for a person?
";

    const CASES: &str = r"
jud: 1
kind: cases
id: cases-2026-10
rubric: triage
cases:
  - id: refund
    state: {message: I want my money back, now.}
    expect:
      actionable: true
      owner: billing
      tone: angry
    tags: [billing]
  - state: {message: thanks, all good}
    expect:
      actionable: false
      tone: 0
  - id: chat
    state:
      - {role: user, text: hi}
      - {role: assistant, text: hello, how can I help?}
      - {role: user, text: get me a human}
    expect:
      handoff: {from_turn: 2}
      owner: support
  - id: never
    state: [{role: user, text: hi}]
    expect:
      handoff: {from_turn: null}
";

    #[test]
    fn cases_parse_and_bind_to_their_rubric() {
        let rubric = Rubric::parse(RUBRIC).unwrap();
        let cases = Cases::parse(CASES).unwrap();
        assert_eq!(cases.cases.len(), 4);
        assert_eq!(cases.cases[1].name(1), "#1");
        assert_eq!(
            cases.cases[0].expect["tone"],
            Expect::Text("angry".to_owned())
        );
        assert_eq!(cases.cases[1].expect["tone"], Expect::Index(0));
        assert_eq!(
            cases.cases[2].expect["handoff"],
            Expect::FromTurn(FromTurn { from_turn: Some(2) })
        );
        assert_eq!(
            cases.cases[3].expect["handoff"],
            Expect::FromTurn(FromTurn { from_turn: None })
        );
        cases.bind(&rubric).unwrap();
        let mut by_fingerprint = cases.clone();
        by_fingerprint.rubric = Some(rubric.fingerprint());
        by_fingerprint.bind(&rubric).unwrap();
        let mut other = cases.clone();
        other.rubric = Some("other".to_owned());
        let err = other.bind(&rubric).unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "rubric"),
            "{err}"
        );
    }

    #[test]
    fn a_label_must_fit_its_question() {
        let rubric = Rubric::parse(RUBRIC).unwrap();
        let bad = [
            ("expect: {actionable: yes}", "a Noul expects"),
            ("expect: {actionable: 1}", "a Noul expects"),
            ("expect: {owner: legal}", "not one of the offered"),
            ("expect: {owner: true}", "a Choice expects"),
            ("expect: {tone: 3}", "out of range"),
            ("expect: {tone: furious}", "neither a level"),
            ("expect: {tone: false}", "a Score expects"),
            ("expect: {nope: true}", "no such question"),
            ("expect: {handoff: {from_turn: 0}}", "not an array"),
        ];
        for (expect, needle) in bad {
            let text = format!(
                "jud: 1\nkind: cases\ncases:\n  - id: c\n    state: {{m: x}}\n    {expect}\n"
            );
            let err = Cases::parse(&text).unwrap().bind(&rubric).unwrap_err();
            assert!(
                matches!(&err, Error::Case { case, reason } if case == "c" && reason.contains(needle)),
                "{expect}: {err}"
            );
        }
        let text = "jud: 1\nkind: cases\ncases:\n  - id: c\n    state: [a, b]\n    expect: {handoff: {from_turn: 2}}\n";
        let err = Cases::parse(text).unwrap().bind(&rubric).unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("past the last turn")),
            "{err}"
        );
        let text =
            "jud: 1\nkind: cases\ncases:\n  - id: c\n    state: 1\n  - id: c\n    state: 2\n";
        let err = Cases::parse(text).unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("same id")),
            "{err}"
        );
        let text = "jud: 1\nkind: cases\ncases:\n  - state: 1\n    expects: {}\n";
        let err = Cases::parse(text).unwrap_err();
        assert!(
            matches!(err, Error::Syntax(_)),
            "a misspelt field is refused: {err}"
        );
    }

    #[test]
    fn a_conversation_unfolds_turn_by_turn() {
        let cases = Cases::parse(CASES).unwrap();
        let chat = &cases.cases[2];
        let per_turn = chat.per_turn();
        assert_eq!(per_turn.len(), 3);
        for (i, turn) in per_turn.iter().enumerate() {
            assert_eq!(turn.index, i);
            assert_eq!(turns(&turn.case.state).unwrap().len(), i + 1);
            assert_eq!(
                turn.case.expect["handoff"],
                Expect::Bool(i >= 2),
                "turn {i}"
            );
        }
        assert!(!per_turn[0].case.expect.contains_key("owner"));
        assert_eq!(
            per_turn[2].case.expect["owner"],
            Expect::Text("support".to_owned())
        );
        for turn in cases.cases[3].per_turn() {
            assert_eq!(turn.case.expect["handoff"], Expect::Bool(false));
        }
        assert!(
            cases.cases[0].per_turn().is_empty(),
            "an object is not a conversation"
        );
    }

    #[test]
    fn yaml_round_trips_and_the_fingerprint_is_the_cases_alone() {
        let cases = Cases::parse(CASES).unwrap();
        let yaml = cases.to_yaml().unwrap();
        let again = Cases::parse(&yaml).unwrap();
        assert_eq!(again, cases);
        let mut renamed = cases.clone();
        renamed.id = Some("other".to_owned());
        renamed.description = Some("x".to_owned());
        assert_eq!(renamed.fingerprint(), cases.fingerprint());
        let mut edited = cases.clone();
        edited.cases[0]
            .expect
            .insert("tone".to_owned(), Expect::Index(1));
        assert_ne!(edited.fingerprint(), cases.fingerprint());
    }

    #[tokio::test]
    async fn grade_reads_labels_in_the_judgment_vocabulary() {
        let rubric = Rubric::parse(RUBRIC).unwrap();
        let cases = Cases::parse(CASES).unwrap();
        let fake = Fake::new()
            .noul("actionable", 0.9)
            .unwrap()
            .choice(
                "owner",
                [("billing", 0.7), ("support", 0.2), ("none_of_these", 0.1)],
                0.55,
            )
            .unwrap()
            .score("tone", [0.1, 0.3, 0.6], 0.5)
            .unwrap()
            .noul("handoff", 0.2)
            .unwrap();
        let refund = &cases.cases[0];
        let response = fake
            .answer(&refund.state, "m", &refund.request(&rubric).unwrap())
            .await
            .unwrap();
        let graded = grade(&rubric, refund, 0, &response).unwrap();
        let ids: Vec<&str> = graded.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["actionable", "owner", "tone"]);
        let by_id: IndexMap<_, _> = graded.into_iter().collect();
        assert_eq!(by_id["actionable"].expected.as_deref(), Some("yes"));
        assert_eq!(by_id["actionable"].correct, Some(true));
        assert_eq!(by_id["owner"].expected.as_deref(), Some("billing"));
        // `angry` is level 2: the answer's distribution is keyed by index.
        assert_eq!(by_id["tone"].expected.as_deref(), Some("2"));
        assert_eq!(by_id["tone"].correct, Some(true));
        assert_eq!(by_id["tone"].p_expected, Some(0.6));

        let chat = &cases.cases[2];
        let response = fake
            .answer(&chat.state, "m", &chat.request(&rubric).unwrap())
            .await
            .unwrap();
        let graded: IndexMap<_, _> = grade(&rubric, chat, 2, &response)
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(graded["handoff"].expected.as_deref(), Some("yes"));
        assert_eq!(graded["handoff"].correct, Some(false));
    }

    const ROUTING: &str = r"
jud: 1.1
kind: rubric
id: support-routing
questions:
  desk:
    type: choice
    instructions: Which desk should take `message`?
    criteria: {none_of_these: Not clearly any desk}
    options_from: request
  refund_request:
    type: noul
    instructions: Does `message` ask for a refund for one of `customer.recent_orders`?
    when: customer.recent_orders
";

    #[test]
    fn a_case_supplies_the_options_its_request_needs() {
        let rubric = Rubric::parse(ROUTING).unwrap();
        let text = "jud: 1.1\nkind: cases\nx-source: {export: 2026-10-04}\ncases:\n  - id: refund\n    state: {message: {text: refund please}, customer: {recent_orders: [1042]}}\n    options:\n      desk: {billing: Invoices, technical: Errors}\n    expect: {desk: billing, refund_request: true}\n";
        let cases = Cases::parse(text).unwrap();
        cases.bind(&rubric).unwrap();
        assert_eq!(cases.extensions["x-source"]["export"], "2026-10-04");
        let asked = cases.cases[0].request(&rubric).unwrap();
        let ids: Vec<&str> = asked.ids().collect();
        assert_eq!(ids, ["desk", "refund_request"]);
        let yaml = cases.to_yaml().unwrap();
        assert!(yaml.starts_with("jud: 1.1\n"), "{yaml}");
        assert_eq!(Cases::parse(&yaml).unwrap(), cases);
        let mut bare = cases.clone();
        bare.extensions.clear();
        assert_eq!(bare.fingerprint(), cases.fingerprint());
        let err = Cases::parse(&text.replacen("jud: 1.1", "jud: 1", 1).replacen(
            "x-source: {export: 2026-10-04}\n",
            "",
            1,
        ))
        .unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "cases.0.options"),
            "{err}"
        );
    }

    #[test]
    fn a_label_must_name_a_question_the_case_asks() {
        let rubric = Rubric::parse(ROUTING).unwrap();
        let bind = |case: &str| {
            Cases::parse(&format!("jud: 1.1\nkind: cases\ncases:\n  - id: c\n{case}"))
                .unwrap()
                .bind(&rubric)
        };
        let err = bind("    state: {message: {text: x}}\n    options: {desk: {billing: B}}\n    expect: {refund_request: false}\n").unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("not asked for this state")),
            "{err}"
        );
        bind("    state: {message: {text: x}}\n    options: {desk: {billing: B}}\n    expect: {desk: billing}\n").unwrap();
        let err = bind("    state: {message: {text: x}}\n    options: {desk: {billing: B}}\n    expect: {desk: technical}\n").unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("not one of the offered options")),
            "{err}"
        );
        let err = bind("    state: {message: {text: x}}\n    expect: {desk: none_of_these}\n")
            .unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("does not lower")),
            "{err}"
        );
        let err = bind("    state: {message: {text: x}}\n    options: {refund_request: {a: A}}\n")
            .unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("options.refund_request")),
            "{err}"
        );
    }

    #[test]
    fn case_options_count_by_presence_and_have_the_shape_of_criteria() {
        let case = |options: &str| {
            format!("jud: 1\nkind: cases\ncases:\n  - id: c\n    state: s\n{options}")
        };
        let err = Cases::parse(&case("    options: {}\n")).unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "cases.0.options"),
            "{err}"
        );
        let err = Cases::parse(&case("    options: null\n")).unwrap_err();
        assert!(matches!(err, Error::Syntax(_)), "{err}");
        let v11 = |options: &str| case(options).replacen("jud: 1", "jud: 1.1", 1);
        for bad in [
            "    options: {desk: {billing: 3}}\n",
            "    options: {desk: {\"\": B}}\n",
            "    options: {desk: {billing: true}}\n",
        ] {
            let err = Cases::parse(&v11(bad)).unwrap_err();
            assert!(
                matches!(&err, Error::Case { reason, .. } if reason.contains("options.desk")),
                "{bad}: {err}"
            );
        }
        Cases::parse(&v11(
            "    options: {desk: {billing: Invoices, technical: null, other: {what: x}}}\n",
        ))
        .unwrap();
    }
}
