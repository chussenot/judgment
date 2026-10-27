//! The `cases` kind: labelled states a rubric is graded on.

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use std::path::Path;

use super::rubric::level_index;
use super::{
    API_VERSION, AssetSource, Envelope, Error, MINOR, Metadata, Result, Rubric, Supplied,
    check_assets, check_name, check_scalars, from_text, load_assets, refuse_extra, some, to_yaml,
};
use crate::answer::{Answer, BoxEdges, Confidence, Response};
use crate::asset::Assets;
use crate::eval::{DEFAULT_IOU_AT_LEAST, Judgment, canonical};
use crate::question::{Question, Questions};

/// Labelled states a rubric is graded on ([`grade`]) and its gates are
/// tuned on (`docs/reference/jud-format.md`, cases).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cases {
    /// `metadata.name`: a name for the set; [`fingerprint`](Self::fingerprint) is exact.
    pub name: String,
    /// `metadata.description`: where the cases came from, for the person reading them.
    pub description: Option<String>,
    /// `metadata.labels`, kept as read and part of no fingerprint.
    pub labels: IndexMap<String, String>,
    /// `metadata.annotations`, kept as read and part of no fingerprint.
    pub annotations: IndexMap<String, String>,
    /// `spec.rubric`: the rubric the labels are for, by name or fingerprint ([`Cases::bind`]).
    pub rubric: Option<String>,
    /// `spec.cases`, in document order.
    pub cases: Vec<Case>,
}

/// One labelled state.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// Options supplied for this case's request, as [`Rubric::lower`]
    /// takes them: with them, a case is a complete request.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub options: Supplied,
    /// The images the state refers to, by reference ([`Case::load_assets`]).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub assets: IndexMap<String, AssetSource>,
    /// Free labels for slicing a report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Why the label is what it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// What a case expects of one question; which form fits which primitive is
/// `docs/reference/jud-format.md`, cases, checked by [`Cases::bind`] and [`grade`].
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// A Box's answer: the labelled box, and the intersection over union it must reach.
    Box(BoxLabel),
}

/// `{box: {top, bottom, left, right}, iou_at_least: 0.5}`: where the thing
/// is, as fractions of the image, and how much of the answer must overlap
/// it to count as right ([`crate::eval::Judgment::located`];
/// [`DEFAULT_IOU_AT_LEAST`] when left out).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoxLabel {
    /// The labelled box.
    #[serde(rename = "box")]
    pub edges: BoxEdges,
    /// The intersection over union that counts as right, 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iou_at_least: Option<f64>,
}

/// `{from_turn: n}`: a Noul over a conversation is true from turn `n`
/// (zero-based) on; `{from_turn: null}` says never.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FromTurn {
    /// The zero-based turn the answer becomes true at; `None` for never.
    #[serde(deserialize_with = "nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
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
            assets: IndexMap::new(),
            tags: Vec::new(),
            note: None,
        }
    }

    /// [`Rubric::lower`] over the case's state and options.
    pub fn request(&self, rubric: &Rubric) -> Result<Questions> {
        rubric.lower(&self.state, &self.options)
    }

    /// The images this case declares, loaded relative to `base` (the cases
    /// document's directory) into `into`, after the rubric's own
    /// ([`Rubric::load_assets`]); a reference both declare must name the
    /// same bytes. A file that moved fails by name.
    pub fn load_assets(&self, base: &Path, into: &mut Assets) -> Result<()> {
        load_assets("assets", &self.assets, base, into)
    }

    /// The case's id, or `#index` for a case without one.
    pub fn name(&self, index: usize) -> String {
        self.id.clone().unwrap_or_else(|| format!("#{index}"))
    }

    /// A conversation case at each of its turns (`docs/reference/jud-format.md`,
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
                        assets: self.assets.clone(),
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
    #[serde(rename = "apiVersion")]
    _api_version: String,
    #[serde(rename = "kind")]
    _kind: String,
    metadata: Metadata,
    spec: RawCasesSpec,
    #[serde(flatten)]
    rest: IndexMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCasesSpec {
    #[serde(default)]
    rubric: Option<String>,
    cases: Vec<RawCase>,
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
    assets: IndexMap<String, AssetSource>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Serialize)]
struct CasesDoc<'a> {
    #[serde(rename = "apiVersion")]
    api_version: &'static str,
    kind: &'static str,
    metadata: Metadata,
    spec: CasesSpec<'a>,
}

#[derive(Serialize)]
struct CasesSpec<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    rubric: Option<&'a str>,
    cases: &'a [Case],
}

impl Cases {
    /// Parse a `cases` document, YAML or JSON. The labels are checked by
    /// [`Cases::bind`], since the document need not name its rubric.
    pub fn parse(text: &str) -> Result<Self> {
        from_text::<Envelope>(text)?.expect_kind("Cases")?;
        check_scalars(text, "Cases")?;
        let raw: RawCases = from_text(text)?;
        refuse_extra(&raw.rest)?;
        raw.metadata.refuse("version", "Cases")?;
        let cases = Self {
            name: raw.metadata.name,
            description: raw.metadata.description,
            labels: raw.metadata.labels,
            annotations: raw.metadata.annotations,
            rubric: raw.spec.rubric,
            cases: raw
                .spec
                .cases
                .into_iter()
                .map(|c| Case {
                    id: c.id,
                    state: c.state,
                    expect: c.expect,
                    options: c.options.unwrap_or_default(),
                    assets: c.assets,
                    tags: c.tags,
                    note: c.note,
                })
                .collect(),
        };
        cases.check()?;
        Ok(cases)
    }

    /// What a reader refuses of a cases document, held to a document built
    /// in code before it is written ([`Cases::to_yaml`]) as to one read.
    fn check(&self) -> Result<()> {
        check_name("metadata.name", &self.name)?;
        if self.cases.is_empty() {
            return Err(Error::Invalid {
                field: "spec.cases".to_owned(),
                reason: "a cases document needs at least one case".to_owned(),
            });
        }
        for (index, case) in self.cases.iter().enumerate() {
            if let Some(id) = &case.id {
                check_name(&format!("spec.cases.{index}.id"), id).map_err(|e| Error::Case {
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
            // A case's images are declared well and referenced in its state.
            check_assets("assets", &case.assets, &case.state).map_err(|e| Error::Case {
                case: case.name(index),
                reason: e.to_string(),
            })?;
            // A Box label is a region and its bar a fraction, as at bind.
            for (question, expect) in &case.expect {
                let Expect::Box(label) = expect else { continue };
                let reason = if let Err(reason) = label.edges.check() {
                    format!("`expect.{question}.box` is not a region: {reason}")
                } else if let Some(bar) = label.iou_at_least
                    && !(0.0..=1.0).contains(&bar)
                {
                    format!("`expect.{question}.iou_at_least` must be between 0 and 1, got {bar}")
                } else {
                    continue;
                };
                return Err(Error::Case {
                    case: case.name(index),
                    reason,
                });
            }
            if let Some(dup) = self.cases[..index]
                .iter()
                .find(|other| other.id.is_some() && other.id == case.id)
            {
                return Err(Error::Case {
                    case: case.name(index),
                    reason: format!("the same id as case {}", dup.name(index)),
                });
            }
        }
        Ok(())
    }

    /// The cases as a YAML document, after the checks [`Cases::parse`] makes.
    pub fn to_yaml(&self) -> Result<String> {
        self.check()?;
        to_yaml(&CasesDoc {
            api_version: API_VERSION,
            kind: "Cases",
            metadata: Metadata {
                name: self.name.clone(),
                minor: Some(MINOR),
                version: None,
                description: self.description.clone(),
                labels: self.labels.clone(),
                annotations: self.annotations.clone(),
            },
            spec: CasesSpec {
                rubric: self.rubric.as_deref(),
                cases: &self.cases,
            },
        })
    }

    /// The fingerprint of the `cases` array alone ([`canonical::fingerprint`]).
    pub fn fingerprint(&self) -> String {
        canonical::fingerprint(&serde_json::to_value(&self.cases).unwrap_or(Value::Null))
    }

    /// Check every case against `rubric`: its request lowers
    /// ([`Case::request`]) and every label names a question that request
    /// asks and fits it (`docs/reference/jud-format.md`, cases), for the
    /// state as a whole and at each turn of a conversation ([`Case::per_turn`]);
    /// the document's `rubric`, when named, is the rubric's id or fingerprint.
    pub fn bind(&self, rubric: &Rubric) -> Result<()> {
        if let Some(named) = &self.rubric
            && *named != rubric.name
            && *named != rubric.fingerprint()
        {
            return Err(Error::Invalid {
                field: "spec.rubric".to_owned(),
                reason: format!(
                    "the cases are for `{named}`, not for `{}` ({})",
                    rubric.name,
                    rubric.fingerprint()
                ),
            });
        }
        for (index, case) in self.cases.iter().enumerate() {
            // The whole state, then each turn of a conversation: what
            // per-turn grading ([`Case::per_turn`]) will ask and label.
            let turns = case.per_turn().into_iter().map(|turn| turn.case);
            for case in std::iter::once(case.clone()).chain(turns) {
                let asked = request(rubric, &case, index)?;
                for (id, expect) in &case.expect {
                    expected_label(rubric, &asked, &case, index, id, expect)?;
                }
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

/// A label as it is graded: in [`Judgment::of_answer`]'s vocabulary
/// (`yes`/`no`, an option key, a level index as a string), or a box with
/// the intersection over union it must reach ([`Judgment::located`]).
enum Label {
    Text(String),
    Box { edges: BoxEdges, iou_at_least: f64 },
}

/// The label checked against the request asked ([`Label`]).
fn expected_label(
    rubric: &Rubric,
    asked: &Questions,
    case: &Case,
    index: usize,
    id: &str,
    expect: &Expect,
) -> Result<Label> {
    let text = |label: String| Ok(Label::Text(label));
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
        (Question::Noul { .. }, Expect::Bool(yes)) => text(yes_no(*yes)),
        (Question::Noul { .. }, Expect::FromTurn(FromTurn { from_turn })) => {
            let Some(all) = turns(&case.state) else {
                return refuse(
                    "`from_turn` labels a conversation, and `state` is not an array".to_owned(),
                );
            };
            match from_turn {
                None => text(yes_no(false)),
                Some(n) => match usize::try_from(*n) {
                    Ok(n) if n < all.len() => text(yes_no(true)),
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
                text(key.clone())
            } else {
                refuse(format!("`{key}` is not one of the offered options"))
            }
        }
        (Question::Choice { .. }, _) => refuse("a Choice expects an option key".to_owned()),
        (Question::Score { criteria, .. }, Expect::Index(level)) => match usize::try_from(*level) {
            Ok(level) if level < criteria.len() => text(level.to_string()),
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
                |level| text(level.to_string()),
            ),
        (Question::Score { .. }, _) => {
            refuse("a Score expects a level index or a level's text".to_owned())
        }
        (Question::Box { .. }, Expect::Box(label)) => {
            if let Err(reason) = label.edges.check() {
                return refuse(format!("`box` is not a region: {reason}"));
            }
            let iou_at_least = label.iou_at_least.unwrap_or(DEFAULT_IOU_AT_LEAST);
            if !(0.0..=1.0).contains(&iou_at_least) {
                return refuse(format!(
                    "`iou_at_least` must be between 0 and 1, got {iou_at_least}"
                ));
            }
            Ok(Label::Box {
                edges: label.edges,
                iou_at_least,
            })
        }
        (Question::Box { .. }, _) => refuse(
            "a Box expects {box: {top, bottom, left, right}} with an optional iou_at_least"
                .to_owned(),
        ),
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
            let judgment = match (&expected, answer) {
                (Label::Text(label), answer) => Judgment::of_answer(answer, Some(label)),
                (
                    Label::Box {
                        edges,
                        iou_at_least,
                    },
                    Answer::Box {
                        edges: answered,
                        confidence,
                        ..
                    },
                ) => Judgment::located(
                    answered,
                    Some(edges),
                    *iou_at_least,
                    confidence.map(Confidence::value),
                ),
                // Verified above: a Box question has a Box answer.
                (Label::Box { .. }, other) => Judgment::of_answer(other, Some("box")),
            };
            Ok((id.clone(), judgment))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;

    /// A fragment written at top level, moved under `spec.cases[0]`.
    fn under_spec(fragment: &str) -> String {
        fragment
            .lines()
            .map(|l| {
                if l.is_empty() {
                    "\n".to_owned()
                } else {
                    format!("  {l}\n")
                }
            })
            .collect()
    }

    use crate::Fake;
    use crate::backend::SystemOne;
    use serde_json::json;
    const RUBRIC: &str = r"
apiVersion: jud/v1.3
kind: Rubric
metadata:
  name: triage
spec:
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
apiVersion: jud/v1.3
kind: Cases
metadata:
  name: cases-2026-10
spec:
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
            matches!(&err, Error::Invalid { field, .. } if field == "spec.rubric"),
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
                "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - id: c\n      state: {{m: x}}\n      {expect}\n"
            );
            let err = Cases::parse(&text).unwrap().bind(&rubric).unwrap_err();
            assert!(
                matches!(&err, Error::Case { case, reason } if case == "c" && reason.contains(needle)),
                "{expect}: {err}"
            );
        }
        let text = "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - id: c\n      state: [a, b]\n      expect: {handoff: {from_turn: 2}}\n";
        let err = Cases::parse(text).unwrap().bind(&rubric).unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("past the last turn")),
            "{err}"
        );
        let text = "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - id: c\n      state: 1\n    - id: c\n      state: 2\n";
        let err = Cases::parse(text).unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("same id")),
            "{err}"
        );
        let text = "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: 1\n      expects: {}\n";
        let err = Cases::parse(text).unwrap_err();
        assert!(
            matches!(err, Error::Syntax(_)),
            "a misspelt field is refused: {err}"
        );
    }

    #[tokio::test]
    async fn a_box_label_is_a_region_graded_by_iou() {
        let mut questions = Questions::new();
        questions.r#box("mug", "the mug in `photo`").unwrap();
        let rubric = Rubric::new("mugs", questions);
        let mut cases = Cases {
            name: "crops".to_owned(),
            ..Cases::default()
        };
        let mut labelled = Case::new(json!({ "photo": "img-1" }));
        labelled.expect.insert(
            "mug".to_owned(),
            Expect::Box(BoxLabel {
                edges: BoxEdges::new(0.2, 0.8, 0.2, 0.8),
                iou_at_least: None,
            }),
        );
        cases.cases.push(labelled);
        cases.bind(&rubric).unwrap();
        let yaml = cases.to_yaml().unwrap();
        assert!(yaml.contains("box:") && yaml.contains("top: 0.2"), "{yaml}");
        assert_eq!(Cases::parse(&yaml).unwrap(), cases);
        let asked = cases.cases[0].request(&rubric).unwrap();
        let response = Fake::new()
            .r#box("mug", BoxEdges::new(0.25, 0.85, 0.25, 0.85), Some(0.8))
            .unwrap()
            .answer(&cases.cases[0].state, "m", &asked)
            .await
            .unwrap();
        let graded: IndexMap<_, _> = grade(&rubric, &cases.cases[0], 0, &response)
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(graded["mug"].correct, Some(true));
        assert!(graded["mug"].iou.unwrap() > 0.5);
        // The label's own, stricter bar.
        let mut strict = cases.clone();
        strict.cases[0].expect["mug"] = Expect::Box(BoxLabel {
            edges: BoxEdges::new(0.2, 0.8, 0.2, 0.8),
            iou_at_least: Some(0.95),
        });
        strict.bind(&rubric).unwrap();
        let graded: IndexMap<_, _> = grade(&rubric, &strict.cases[0], 0, &response)
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(graded["mug"].correct, Some(false));
        // What does not fit: a label that is not a region, a bar outside 0..1,
        // a word where a box is expected, a box where a Noul is.
        let bad = [
            (
                "expect: {mug: {box: {top: 0.8, bottom: 0.2, left: 0.2, right: 0.8}}}",
                "not a region",
            ),
            (
                "expect: {mug: {box: {top: 0.2, bottom: 0.8, left: 0.2, right: 0.8}, iou_at_least: 1.5}}",
                "between 0 and 1",
            ),
            ("expect: {mug: true}", "a Box expects"),
        ];
        for (expect, needle) in bad {
            let text = format!(
                "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: crops\nspec:\n  cases:\n    - id: c\n      state: {{photo: img-1}}\n      {expect}\n"
            );
            // The region and the bar are checked as the document is read; the
            // kind, when the label is bound to its question.
            let err = match Cases::parse(&text) {
                Ok(cases) => cases.bind(&rubric).unwrap_err(),
                Err(err) => err,
            };
            assert!(
                matches!(&err, Error::Case { case, reason } if case == "c" && reason.contains(needle)),
                "{expect}: {err}"
            );
        }
        // A field a box label does not have is refused by the reader.
        let err = Cases::parse(
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: crops\nspec:\n  cases:\n    - id: c\n      state: {photo: img-1}\n      expect: {mug: {box: {top: 0.2, bottom: 0.8, left: 0.2, right: 0.8}, until: 1}}\n",
        )
        .unwrap_err();
        assert!(matches!(err, Error::Syntax(_)), "{err}");
        let noul_rubric = Rubric::parse(RUBRIC).unwrap();
        let text = "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: crops\nspec:\n  cases:\n    - id: c\n      state: {m: x}\n      expect: {actionable: {box: {top: 0.2, bottom: 0.8, left: 0.2, right: 0.8}}}\n";
        let err = Cases::parse(text).unwrap().bind(&noul_rubric).unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("a Noul expects")),
            "{err}"
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
        renamed.name = "other".to_owned();
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
apiVersion: jud/v1.3
kind: Rubric
metadata:
  name: support-routing
spec:
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
        let text = "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\n  annotations: {export: \"2026-10-04\"}\nspec:\n  cases:\n    - id: refund\n      state: {message: {text: refund please}, customer: {recent_orders: [1042]}}\n      options:\n        desk: {billing: Invoices, technical: Errors}\n      expect: {desk: billing, refund_request: true}\n";
        let cases = Cases::parse(text).unwrap();
        cases.bind(&rubric).unwrap();
        assert_eq!(cases.annotations["export"], "2026-10-04");
        let asked = cases.cases[0].request(&rubric).unwrap();
        let ids: Vec<&str> = asked.ids().collect();
        assert_eq!(ids, ["desk", "refund_request"]);
        let yaml = cases.to_yaml().unwrap();
        assert!(
            yaml.starts_with("apiVersion: jud/v1\nkind: Cases\n"),
            "{yaml}"
        );
        assert_eq!(Cases::parse(&yaml).unwrap(), cases);
        let mut bare = cases.clone();
        bare.annotations.clear();
        assert_eq!(bare.fingerprint(), cases.fingerprint());
    }

    #[test]
    fn a_label_must_name_a_question_the_case_asks() {
        let rubric = Rubric::parse(ROUTING).unwrap();
        let bind = |case: &str| {
            Cases::parse(&format!("apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - id: c\n{}", under_spec(case)))
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
            format!(
                "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - id: c\n      state: s\n  {options}"
            )
        };
        Cases::parse(&case("    options: {}\n")).unwrap();
        let err = Cases::parse(&case("    options: null\n")).unwrap_err();
        assert!(
            matches!(&err, Error::Invalid { field, .. } if field == "spec.cases[0].options"),
            "{err}"
        );
        let v11 = |options: &str| case(options);
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

    /// A label that is no region is refused when read, with no rubric to
    /// bind to, as the schemas refuse an edge outside 0..1.
    #[test]
    fn a_box_labels_region_is_checked_when_read() {
        for (edges, needle) in [
            (
                "top: 0.2, bottom: 1.5, left: 0.2, right: 0.8",
                "outside 0..=1",
            ),
            ("top: 0.2, bottom: 0.8, left: 0.8, right: 0.2", "not below"),
        ] {
            let text = format!(
                "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: c\n  minor: 4\nspec:\n  cases:\n    - id: c\n      state: {{photo: p}}\n      expect: {{b: {{box: {{{edges}}}}}}}\n"
            );
            let err = Cases::parse(&text).unwrap_err();
            assert!(
                matches!(&err, Error::Case { case, reason } if case == "c" && reason.contains("not a region") && reason.contains(needle)),
                "{edges}: {err}"
            );
        }
    }

    /// Per-turn grading asks each turn's own request: a label bind let
    /// through for the whole conversation is refused if a turn cannot take it.
    #[test]
    fn bind_checks_each_turn_of_a_conversation() {
        let rubric = Rubric::parse(
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    always:\n      type: noul\n      instructions: ok?\n    late:\n      type: noul\n      instructions: ok?\n      when: \"1\"\n",
        )
        .unwrap();
        let bind = |expect: &str| {
            Cases::parse(&format!(
                "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - id: c\n      state: [a, b]\n      expect: {expect}\n"
            ))
            .unwrap()
            .bind(&rubric)
        };
        // Asked at turn 1 only: a label at the last turn is graded there alone.
        bind("{late: true}").unwrap();
        bind("{always: {from_turn: 1}}").unwrap();
        // A `from_turn` label is graded at turn 0 too, where `late` is not asked.
        let err = bind("{late: {from_turn: 1}}").unwrap_err();
        assert!(
            matches!(&err, Error::Case { case, reason } if case == "c" && reason.contains("not asked for this state")),
            "{err}"
        );
    }

    /// A writer refuses what its reader refuses, so what it writes reads back.
    #[test]
    fn cases_are_checked_before_they_are_written() {
        let mut cases = Cases::parse(CASES).unwrap();
        Cases::parse(&cases.to_yaml().unwrap()).unwrap();
        let mut dup = cases.clone();
        dup.cases[1].id = Some("refund".to_owned());
        let err = dup.to_yaml().unwrap_err();
        assert!(
            matches!(&err, Error::Case { reason, .. } if reason.contains("same id")),
            "{err}"
        );
        let mut unnamed = cases.clone();
        unnamed.cases[0].id = Some("../x".to_owned());
        assert!(unnamed.to_yaml().is_err());
        let mut bad_label = cases.clone();
        bad_label.cases[0].expect.insert(
            "b".to_owned(),
            Expect::Box(BoxLabel {
                edges: BoxEdges::new(0.8, 0.2, 0.2, 0.8),
                iou_at_least: None,
            }),
        );
        assert!(bad_label.to_yaml().is_err());
        let mut empty = cases.clone();
        empty.cases.clear();
        assert!(empty.to_yaml().is_err());
        cases.name = "a b".to_owned();
        assert!(cases.to_yaml().is_err());
    }

    /// A reference is in a string of the state, never only in its syntax.
    #[test]
    fn a_case_reference_is_found_in_a_string_of_the_state() {
        let parse = |state: &str| {
            Cases::parse(&format!(
                "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: c\n  minor: 4\nspec:\n  cases:\n    - state: {state}\n      assets:\n        photo: {{path: p.png}}\n"
            ))
        };
        parse("{image: photo}").unwrap();
        parse("{text: see the photo here}").unwrap();
        parse("[a, {image: x-photo-y}]").unwrap();
        // Only a key, a number, or the JSON syntax around the values.
        for state in [
            "{photo: x}",
            "{image: 1}",
            "{image: [\"{\", \"}\"]}",
            "[\"\", 3]",
        ] {
            let err = parse(state).unwrap_err();
            assert!(
                matches!(&err, Error::Case { reason, .. } if reason.contains("referenced nowhere")),
                "{state}: {err}"
            );
        }
    }
}
