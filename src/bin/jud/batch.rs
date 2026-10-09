//! A rubric and its labelled cases, run together: the part `jud record`,
//! `jud eval` and `jud tune` share (decision 0021). Loading binds the cases
//! to the rubric; [`units`] says which requests the cases ask (a
//! conversation labelled with `from_turn` is asked turn by turn); answering
//! asks a [`Backend`] and grades each response against its case's labels.
//! Every step is the library's: `Cases::bind`, `Case::request`,
//! `Case::per_turn`, `jud::grade`, `Rubric::apply`.

use indexmap::IndexMap;
use judgment::Questions;
use judgment::Response;
use judgment::eval::{Judgment, is_name};
use judgment::jud::{Case, Cases, Expect, Rubric, Verdict};

use crate::backend::{Backend, Failure};
use crate::tools;

/// A rubric and the cases bound to it.
pub(crate) struct Loaded {
    pub rubric: Rubric,
    pub cases: Cases,
}

/// Read both documents and bind the cases to the rubric, so a label that
/// does not fit fails before any call. Every failure is a usage failure
/// naming the file.
pub(crate) fn load(rubric_path: &str, cases_path: &str) -> Result<Loaded, Failure> {
    let rubric =
        tools::read_document(rubric_path, "Rubric", Rubric::parse).map_err(Failure::Usage)?;
    let cases = tools::read_document(cases_path, "Cases", Cases::parse).map_err(Failure::Usage)?;
    cases
        .bind(&rubric)
        .map_err(|e| Failure::Usage(format!("{cases_path} does not fit {rubric_path}: {e}")))?;
    Ok(Loaded { rubric, cases })
}

/// One request a case asks: the case itself, or one turn of a conversation.
#[derive(Debug, Clone)]
pub(crate) struct Unit {
    /// The case's id, `#3` for a case without one; `<id>-turn-<n>` for a
    /// turn, which is also the name its recording takes.
    pub name: String,
    /// Position of the case in the document, which names a case without an
    /// id in the library's errors.
    pub index: usize,
    /// The case as asked: for a turn, the state cut after that turn and the
    /// labels resolved at it.
    pub case: Case,
    /// True for one turn of a conversation, a state cut short of the
    /// document's.
    pub by_turn: bool,
}

impl Unit {
    /// The name a recording of this request takes, `None` for a case
    /// without an id: `#3` is not a name (`eval::is_name`), so the file
    /// system never sees it.
    pub(crate) fn recording_name(&self) -> Option<&str> {
        is_name(&self.name).then_some(self.name.as_str())
    }
}

/// The requests the cases ask, in document order. A conversation (a state
/// that is an array) with a `from_turn` label is asked turn by turn, since
/// the label says where the answer changes; every other case is one request.
pub(crate) fn units(cases: &Cases) -> Vec<Unit> {
    let mut out = Vec::new();
    for (index, case) in cases.cases.iter().enumerate() {
        let name = case.name(index);
        let by_turn = case
            .expect
            .values()
            .any(|e| matches!(e, Expect::FromTurn(_)));
        let turns = if by_turn { case.per_turn() } else { Vec::new() };
        if turns.is_empty() {
            out.push(Unit {
                name,
                index,
                case: case.clone(),
                by_turn: false,
            });
        } else {
            for turn in turns {
                out.push(Unit {
                    name: format!("{name}-turn-{}", turn.index),
                    index,
                    case: turn.case,
                    by_turn: true,
                });
            }
        }
    }
    out
}

/// One unit answered and graded.
pub(crate) struct Answered {
    pub unit: Unit,
    pub questions: Questions,
    pub response: Response,
    /// What the rubric's policy makes of each answer, per question asked.
    pub verdicts: IndexMap<String, Verdict>,
    /// One judgment per labelled question, in the case's order.
    pub judgments: Vec<(String, Judgment)>,
}

/// A unit with its request lowered: what [`plan`] hands to the commands.
pub(crate) struct Planned {
    pub unit: Unit,
    pub questions: Questions,
}

/// Lower a unit's request, a usage failure when the rubric does not lower
/// for it or asks nothing: a state that satisfies none of the rubric's
/// `when` declarations lowers to an empty request, which the wire refuses
/// (a request needs at least one question), so a call for it can only fail.
pub(crate) fn request(rubric: &Rubric, unit: &Unit) -> Result<Questions, Failure> {
    let questions = unit.case.request(rubric).map_err(|e| {
        Failure::Usage(format!(
            "the rubric does not lower for case {}: {e}",
            unit.name
        ))
    })?;
    if questions.is_empty() {
        return Err(Failure::Usage(format!(
            "the rubric asks no question for case {}: every `when` fails for its state, so there is nothing to ask",
            unit.name
        )));
    }
    Ok(questions)
}

/// Everything about the cases that can be known before a call is made, so
/// that a document problem is a usage failure (status 2) found before any
/// call is paid for, never a backend failure after some were: every unit's
/// request lowers and asks something, and every label of a conversation's
/// turn names a question that turn asks. `Cases::bind` has checked the
/// labels at the whole state; a turn is a shorter state, where a `when` can
/// fail that held at the end.
pub(crate) fn plan(loaded: &Loaded) -> Result<Vec<Planned>, Failure> {
    let units = units(&loaded.cases);
    let mut planned = Vec::with_capacity(units.len());
    for unit in units {
        let questions = request(&loaded.rubric, &unit)?;
        planned.push(Planned { unit, questions });
    }
    let turns: Vec<Case> = planned
        .iter()
        .filter(|p| p.unit.by_turn)
        .map(|p| p.unit.case.clone())
        .collect();
    if !turns.is_empty() {
        let mut cut = loaded.cases.clone();
        cut.cases = turns;
        cut.bind(&loaded.rubric).map_err(|e| {
            Failure::Usage(format!(
                "a conversation's label does not fit one of its turns: {e}"
            ))
        })?;
    }
    Ok(planned)
}

/// Answer and grade one planned unit through `backend`.
pub(crate) fn answer(
    backend: &Backend,
    loaded: &Loaded,
    planned: Planned,
) -> Result<Answered, Failure> {
    let response = backend.answer(&planned.unit.case.state, &planned.questions)?;
    grade(loaded, planned.unit, planned.questions, response)
}

/// Read a response through the policy and against the labels.
pub(crate) fn grade(
    loaded: &Loaded,
    unit: Unit,
    questions: Questions,
    response: Response,
) -> Result<Answered, Failure> {
    let verdicts = loaded.rubric.apply(&questions, &response).map_err(|e| {
        Failure::Backend(format!(
            "the answer for case {} does not fit the rubric: {e}",
            unit.name
        ))
    })?;
    let judgments = judgment::jud::grade(&loaded.rubric, &unit.case, unit.index, &response)
        .map_err(|e| {
            Failure::Backend(format!(
                "the answer for case {} cannot be graded: {e}",
                unit.name
            ))
        })?;
    Ok(Answered {
        unit,
        questions,
        response,
        verdicts,
        judgments,
    })
}

/// Answer every planned unit, in order. Under a replay a missing recording
/// is not fatal until every unit has been tried, so one run names every case
/// that has none; any other failure stops the run, naming the case, because
/// against a server the next call would fail the same way and cost the same.
pub(crate) fn answer_all(
    backend: &Backend,
    loaded: &Loaded,
    planned: Vec<Planned>,
) -> Result<Vec<Answered>, Failure> {
    let mut answered = Vec::with_capacity(planned.len());
    let mut missing: Vec<String> = Vec::new();
    let total = planned.len();
    backend.announce(total);
    for (position, one) in planned.into_iter().enumerate() {
        let name = one.unit.name.clone();
        match answer(backend, loaded, one) {
            Ok(a) => answered.push(a),
            Err(Failure::Missing(_)) => missing.push(name),
            Err(Failure::Backend(m)) => {
                return Err(Failure::Backend(format!(
                    "case {name} ({} of {total}): {m}",
                    position + 1
                )));
            }
            Err(e) => return Err(e),
        }
    }
    if missing.is_empty() {
        Ok(answered)
    } else {
        Err(Failure::Missing(format!(
            "no recording answers {} case{}: {}; record them first with `jud record`",
            missing.len(),
            if missing.len() == 1 { "" } else { "s" },
            missing.join(", ")
        )))
    }
}

/// The labelled judgments of every question, in the rubric's order: what a
/// per-question report or sweep reads. A question nobody labelled has an
/// empty list.
pub(crate) fn by_question(
    rubric: &Rubric,
    answered: &[Answered],
) -> IndexMap<String, Vec<Judgment>> {
    let mut grouped: IndexMap<String, Vec<Judgment>> = rubric
        .questions
        .keys()
        .map(|id| (id.clone(), Vec::new()))
        .collect();
    for a in answered {
        for (id, judgment) in &a.judgments {
            grouped
                .entry(id.clone())
                .or_default()
                .push(judgment.clone());
        }
    }
    grouped
}
