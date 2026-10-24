//! `--swap` and `--tone-perturb`, on `jud eval` and `jud record`: the same
//! cases asked again with each Choice's options in the reverse order, or
//! with the better answer rewritten in another tone, and the report of how
//! far the verdicts move. `jud eval --against DIR` reads the same table
//! between the run's answers and another model's recordings.
//!
//! A swap reverses the options' order and keeps every key with its
//! description, so the one thing that changes is where each answer sits: a
//! judge whose pick follows the place rather than the answer flips. A tone
//! perturbation replaces the description of the labelled option (the
//! better answer) with a rewrite the user supplies, CALM's sentiment test
//! (Ye et al., arXiv 2410.02736). arXiv 2610.18122 found both moving the
//! verdicts of typed decision models (`docs/guides/test-a-judge-for-position-and-tone-bias.md`).
//!
//! A perturbed request is a request like any other: its own fingerprint,
//! its own recording (`CASE.swapped.jud`, `CASE.toned.jud`), answered from a
//! replay with no key. The ordinary report reads the cases as written only;
//! the perturbed answers are read here, against them.

use std::collections::{BTreeMap, HashMap};
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum, ValueHint};
use indexmap::IndexMap;
use judgment::eval::metrics::wilson_interval;
use judgment::eval::{Judgment, Z_95};
use judgment::jud::Expect;
use judgment::{Answer, Question, Questions, Response};
use serde::Serialize;
use serde_json::Value;

use crate::backend::Failure;
use crate::batch::{Answered, Loaded, Planned, Variant};
use crate::out;

/// The suffix a swapped request's recording name takes, and the one a
/// joint copy's question id takes.
pub(crate) const SWAPPED: &str = "swapped";
/// The suffix a tone-perturbed request's recording name takes.
pub(crate) const TONED: &str = "toned";

/// How the reversed order is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum SwapMode {
    /// A second request per case, the options reversed: the two orders
    /// cannot see each other, and the original order's recording is the
    /// one a plain `jud eval` reads.
    Separate,
    /// One request per case holding both orders, the reversed copy of each
    /// Choice under `ID.swapped`: the state is sent once. For a server that
    /// answers a request's questions independently, as System One does.
    Joint,
}

/// The perturbation flags `jud eval` and `jud record` share.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct PerturbArgs {
    /// Ask every case again with each Choice's options in the reverse
    /// order, and report how often the pick changes.
    ///
    /// Each key stays with its description, so only the answers' places
    /// change; keys that say nothing about place (not `first`) keep it a
    /// test of position. `separate` (the default) asks the reversed order
    /// as a second request; `joint` adds it to the same request as a
    /// second question, `ID.swapped`, which a server that answers
    /// questions independently allows. Recordings are kept per request:
    /// record with the flag you evaluate with.
    #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "separate", value_name = "HOW")]
    pub swap: Option<SwapMode>,
    /// Ask every case the file names again with its better answer
    /// rewritten, and report how often the pick changes.
    ///
    /// FILE is YAML or JSON: case id, then question id, then the rewrite
    /// (text, or an object or array as an option's description may be):
    /// `{case-7: {better: "That is SO wrong, it's 4!"}}`. The rewrite
    /// replaces the description of the option the case labels right, so
    /// the content stays and the tone changes, as in CALM's sentiment test.
    /// A case or question the file names that the cases do not have, or a
    /// question the case does not label with an option, is refused before
    /// any call.
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub tone_perturb: Option<PathBuf>,
}

impl PerturbArgs {
    /// Whether any perturbation was asked for.
    pub(crate) const fn any(&self) -> bool {
        self.swap.is_some() || self.tone_perturb.is_some()
    }
}

/// A Choice with its options in the reverse order; `None` for a question
/// whose order a swap cannot change.
fn reversed(question: &Question) -> Option<Question> {
    match question {
        Question::Choice {
            instructions,
            criteria,
        } if criteria.len() >= 2 => Some(Question::Choice {
            instructions: instructions.clone(),
            criteria: criteria
                .iter()
                .rev()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }),
        _ => None,
    }
}

/// The reversed copies of a request's swappable Choices, under `ID.swapped`.
///
/// The copy is renamed as well as reversed in both modes. A request's
/// fingerprint is canonical JSON, whose keys are sorted, so it is blind to
/// option order (decision HD2 in the beads, open): under its own id the
/// reversed request has a fingerprint of its own, records into a file of
/// its own and is never answered from the original order's recording.
/// TypeSafe documents that a question's key "is not sent to the underlying
/// model" (<https://docs.typesafe.ai/api>), so there the model sees the same
/// question with only the order changed; a server that prompts with ids
/// would not (`docs/reference/cli.md`, Position and tone).
fn joint_copies(questions: &Questions, case: &str) -> Result<Questions, Failure> {
    let mut copies = Questions::new();
    for (id, question) in questions.iter() {
        let Some(copy) = reversed(question) else {
            continue;
        };
        let name = format!("{id}.{SWAPPED}");
        if questions.get(&name).is_some() {
            return Err(Failure::Usage(format!(
                "--swap=joint asks the reversed copy of `{id}` as `{name}`, which the rubric already asks for case {case}; use --swap=separate"
            )));
        }
        copies.add(name, copy).map_err(|e| {
            Failure::Usage(format!("case {case}: the reversed copy of `{id}`: {e}"))
        })?;
    }
    Ok(copies)
}

/// Rewrites by case, then by question.
type Rewrites = IndexMap<String, IndexMap<String, Value>>;

fn read_rewrites(path: &std::path::Path) -> Result<Rewrites, Failure> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        Failure::Usage(format!(
            "cannot read --tone-perturb {}: {e}",
            path.display()
        ))
    })?;
    serde_saphyr::from_str::<Rewrites>(&text).map_err(|e| {
        Failure::Usage(format!(
            "--tone-perturb {}: not a map of case id to question id to rewrite: {e}",
            path.display()
        ))
    })
}

/// The request with the labelled option of each named Choice described by
/// its rewrite. Every name the file uses must be there: a rewrite that
/// lands nowhere would report a case as robust that was never perturbed.
fn toned(
    planned: &Planned,
    rewrites: &IndexMap<String, Value>,
    file: &str,
) -> Result<Questions, Failure> {
    let case = &planned.unit.name;
    let mut out = Questions::new();
    for (id, question) in planned.questions.iter() {
        let next = match (rewrites.get(id), question) {
            (None, _) => question.clone(),
            (
                Some(rewrite),
                Question::Choice {
                    instructions,
                    criteria,
                },
            ) => {
                let Some(Expect::Text(label)) = planned.unit.case.expect.get(id) else {
                    return Err(Failure::Usage(format!(
                        "{file}: case {case} rewrites the better answer of `{id}`, which the case does not label with an option"
                    )));
                };
                if !criteria.contains_key(label) {
                    return Err(Failure::Usage(format!(
                        "{file}: case {case} labels `{id}` `{label}`, which is not one of its options"
                    )));
                }
                let mut criteria = criteria.clone();
                criteria.insert(label.clone(), rewrite.clone());
                Question::Choice {
                    instructions: instructions.clone(),
                    criteria,
                }
            }
            (Some(_), other) => {
                return Err(Failure::Usage(format!(
                    "{file}: case {case} rewrites an answer of `{id}`, a {}; only a Choice's options are answers",
                    other.kind()
                )));
            }
        };
        let _ = out.add(id, next);
    }
    for id in rewrites.keys() {
        if planned.questions.get(id).is_none() {
            return Err(Failure::Usage(format!(
                "{file}: case {case} rewrites `{id}`, a question its request does not ask"
            )));
        }
    }
    Ok(out)
}

/// The planned requests with the perturbed ones added, each after the
/// request it perturbs: the original, then its swap, then its tone. Every
/// refusal is a usage failure before any call.
pub(crate) fn expand(
    loaded: &Loaded,
    planned: Vec<Planned>,
    args: &PerturbArgs,
) -> Result<Vec<Planned>, Failure> {
    if !args.any() {
        return Ok(planned);
    }
    let rewrites = match &args.tone_perturb {
        Some(path) => Some((read_rewrites(path)?, path.display().to_string())),
        None => None,
    };
    if let Some((rewrites, file)) = &rewrites {
        for case in rewrites.keys() {
            if !planned.iter().any(|p| &p.unit.name == case) {
                return Err(Failure::Usage(format!(
                    "{file} rewrites case {case}, which the cases do not have"
                )));
            }
        }
    }
    if args.swap.is_some()
        && !planned
            .iter()
            .any(|p| p.questions.iter().any(|(_, q)| reversed(q).is_some()))
    {
        return Err(Failure::Usage(format!(
            "--swap reverses a Choice's options, and {} asks no Choice with two or more",
            loaded.rubric.name
        )));
    }
    let mut out = Vec::with_capacity(planned.len() * 3);
    for mut p in planned {
        let name = p.unit.name.clone();
        let copies = match args.swap {
            Some(_) => joint_copies(&p.questions, &name)?,
            None => Questions::new(),
        };
        // Joint: the copies ride in the case's own request. Separate: they
        // are a request of their own, which asks nothing else.
        let swap = match args.swap {
            Some(SwapMode::Joint) => {
                p.joint = copies;
                None
            }
            Some(SwapMode::Separate) if !copies.is_empty() => Some(copies),
            _ => None,
        };
        let tone = match &rewrites {
            Some((rewrites, file)) => rewrites
                .get(&name)
                .map(|r| toned(&p, r, file))
                .transpose()?,
            None => None,
        };
        let derive = |questions: Questions, joint: Questions, variant: Variant, suffix: &str| {
            let mut unit = p.unit.clone();
            unit.name = format!("{name}.{suffix}");
            unit.variant = variant;
            unit.base.clone_from(&name);
            Planned {
                unit,
                questions,
                assets: p.assets.clone(),
                joint,
            }
        };
        let swap = swap.map(|copies| derive(Questions::new(), copies, Variant::Swapped, SWAPPED));
        let tone = tone.map(|q| derive(q, Questions::new(), Variant::Toned, TONED));
        out.push(p);
        out.extend(swap);
        out.extend(tone);
    }
    Ok(out)
}

/// Take the joint copies' answers out of a response, by the id of the
/// question each copies, so the rest is the response to the rubric's own
/// questions.
pub(crate) fn split_joint(
    mut response: Response,
    copies: &Questions,
) -> (Response, BTreeMap<String, Answer>) {
    let mut joint = BTreeMap::new();
    for id in copies.ids() {
        if let Some(answer) = response.answers.remove(id) {
            let base = id
                .strip_suffix(&format!(".{SWAPPED}"))
                .unwrap_or(id)
                .to_owned();
            joint.insert(base, answer);
        }
    }
    (response, joint)
}

/// Which perturbation a comparison measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Test {
    /// The options in the reverse order.
    Swap,
    /// The better answer in another tone.
    Tone,
    /// The same request, answered in the recordings `--against` names.
    Against,
}

impl Test {
    const fn label(self) -> &'static str {
        match self {
            Self::Swap => "swap",
            Self::Tone => "tone",
            Self::Against => "against",
        }
    }
}

/// Accuracy over the labelled pairs under one form of the request.
#[derive(Debug, Clone, Default, Serialize)]
#[allow(clippy::struct_field_names)] // the names `jud eval`'s questions use
pub(crate) struct Accuracy {
    pub labelled: usize,
    pub correct: usize,
    pub accuracy: Option<f64>,
    pub accuracy_interval95: Option<(f64, f64)>,
}

impl Accuracy {
    fn of(correct: usize, labelled: usize) -> Self {
        Self {
            labelled,
            correct,
            accuracy: ratio(correct, labelled),
            accuracy_interval95: wilson_interval(correct, labelled, Z_95),
        }
    }
}

/// One question under one perturbation: how often its pick moved, and
/// what that did to its accuracy.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Comparison {
    pub question: String,
    pub test: Test,
    /// Cases answered both as written and perturbed.
    pub pairs: usize,
    /// Pairs whose pick differs between the two.
    pub flips: usize,
    pub flip_rate: Option<f64>,
    pub flip_rate_interval95: Option<(f64, f64)>,
    /// Pairs whose original answer had confidence at or above `confident_at`.
    pub confident_pairs: usize,
    pub confident_flips: usize,
    pub confident_flip_rate: Option<f64>,
    pub confident_flip_rate_interval95: Option<(f64, f64)>,
    /// Mean of |p − p′| over the pairs, p being the probability the
    /// original answer put on its own pick and p′ what the perturbed
    /// answer put on that same option.
    pub mean_abs_dp: Option<f64>,
    /// Accuracy as written, over the labelled pairs.
    pub original: Accuracy,
    /// Accuracy perturbed, over the same pairs.
    pub perturbed: Accuracy,
    /// The cases whose pick moved, in document order.
    pub flipped: Vec<String>,
}

/// What `--swap` and `--tone-perturb` found, as the JSON report's
/// `robustness` holds it.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Robustness {
    /// The confidence bar the `confident_*` counts use, from `--confident-at`.
    pub confident_at: f64,
    /// The perturbed requests answered (a joint swap is in its original's).
    pub perturbed_requests: usize,
    /// One per question and perturbation, the swap first, in the rubric's order.
    pub comparisons: Vec<Comparison>,
}

#[allow(clippy::cast_precision_loss)] // counts, far below 2^52
fn ratio(n: usize, of: usize) -> Option<f64> {
    (of > 0).then(|| n as f64 / of as f64)
}

/// One pair, as the totals count it.
struct Pair {
    case: String,
    flipped: bool,
    confident: bool,
    abs_dp: f64,
    original: String,
    perturbed: String,
    /// Whether each answer is right, when the case labels the question.
    correct: Option<(bool, bool)>,
}

/// Two answers to one question as a pair, read as `jud eval` grades them
/// (`Judgment::of_answer`): a Choice by its option, a Score by its level, a
/// Noul as yes from 0.5. `None` when either has no pick and distribution
/// to compare: a Box, an answer of a kind this release does not know, or
/// none at all.
fn pair(case: &str, a: Option<&Answer>, b: Option<&Answer>, confident_at: f64) -> Option<Pair> {
    let comparable = |x: Option<&Answer>| {
        x.filter(|x| {
            matches!(
                x,
                Answer::Choice { .. } | Answer::Score { .. } | Answer::Noul { .. }
            )
        })
        .map(|x| Judgment::of_answer(x, None))
    };
    let (a, b) = (comparable(a)?, comparable(b)?);
    let on_pick = |j: &Judgment| j.probabilities.get(&a.predicted).copied().unwrap_or(0.0);
    Some(Pair {
        case: case.to_owned(),
        flipped: a.predicted != b.predicted,
        confident: a.confidence >= confident_at,
        abs_dp: (on_pick(&a) - on_pick(&b)).abs(),
        correct: None,
        original: a.predicted,
        perturbed: b.predicted,
    })
}

impl Robustness {
    /// Pair every perturbed answer with its original's, question by
    /// question: a swap's reversed copies, wherever they were asked, and a
    /// tone's questions where the rewrite changed what they ask.
    pub(crate) fn build(
        loaded: &Loaded,
        original: &[Answered],
        perturbed: &[Answered],
        confident_at: f64,
    ) -> Self {
        let by_name: HashMap<&str, &Answered> =
            original.iter().map(|a| (a.unit.name.as_str(), a)).collect();
        let mut pairs: IndexMap<(String, Test), Vec<Pair>> = IndexMap::new();
        for test in [Test::Swap, Test::Tone] {
            for id in loaded.rubric.questions.keys() {
                pairs.insert((id.clone(), test), Vec::new());
            }
        }
        let mut push = |id: &str, test: Test, base: &Answered, answer: Option<&Answer>| {
            // A swap or a tone rewrite changes a Choice only.
            let Some(original @ Answer::Choice { .. }) = base.response.answers.get(id) else {
                return;
            };
            let Some(mut p) = pair(&base.unit.name, Some(original), answer, confident_at) else {
                return;
            };
            if let Some(Expect::Text(label)) = base.unit.case.expect.get(id) {
                p.correct = Some((&p.original == label, &p.perturbed == label));
            }
            pairs.entry((id.to_owned(), test)).or_default().push(p);
        };
        // Joint copies, in the original's own response.
        for base in original {
            for (id, answer) in &base.joint {
                push(id, Test::Swap, base, Some(answer));
            }
        }
        for a in perturbed {
            let Some(base) = by_name.get(a.unit.base.as_str()) else {
                continue;
            };
            for (id, answer) in &a.joint {
                push(id, Test::Swap, base, Some(answer));
            }
            if a.unit.variant == Variant::Toned {
                for (id, question) in a.questions.iter() {
                    if base.questions.get(id) != Some(question) {
                        push(id, Test::Tone, base, a.response.answers.get(id));
                    }
                }
            }
        }
        let comparisons = pairs
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|((question, test), v)| summarise(question, test, &v))
            .collect();
        Self {
            confident_at,
            perturbed_requests: perturbed.len(),
            comparisons,
        }
    }
}

fn summarise(question: String, test: Test, pairs: &[Pair]) -> Comparison {
    let n = pairs.len();
    let flips = pairs.iter().filter(|p| p.flipped).count();
    let confident: Vec<&Pair> = pairs.iter().filter(|p| p.confident).collect();
    let confident_flips = confident.iter().filter(|p| p.flipped).count();
    let labelled: Vec<(bool, bool)> = pairs.iter().filter_map(|p| p.correct).collect();
    let right = |side: fn(&(bool, bool)) -> bool| labelled.iter().filter(|c| side(c)).count();
    Comparison {
        question,
        test,
        pairs: n,
        flips,
        flip_rate: ratio(flips, n),
        flip_rate_interval95: wilson_interval(flips, n, Z_95),
        confident_pairs: confident.len(),
        confident_flips,
        confident_flip_rate: ratio(confident_flips, confident.len()),
        confident_flip_rate_interval95: wilson_interval(confident_flips, confident.len(), Z_95),
        mean_abs_dp: ratio(1, n).map(|per| pairs.iter().map(|p| p.abs_dp).sum::<f64>() * per),
        original: Accuracy::of(right(|c| c.0), labelled.len()),
        perturbed: Accuracy::of(right(|c| c.1), labelled.len()),
        flipped: pairs
            .iter()
            .filter(|p| p.flipped)
            .map(|p| p.case.clone())
            .collect(),
    }
}

/// `0.12 [0.06, 0.24] (6/50)`, or `-` with nothing to count.
fn rate(value: Option<f64>, interval: Option<(f64, f64)>, n: usize, of: usize) -> String {
    match (value, interval) {
        (Some(v), Some((low, high))) => format!("{v:.2} [{low:.2}, {high:.2}] ({n}/{of})"),
        _ => "-".to_owned(),
    }
}

/// What `jud eval --against DIR` found: the run's answers and the ones
/// recorded in DIR, request by request, as the JSON report's `agreement`
/// holds them. A report and never a gate: two models that agree can both
/// be wrong, so agreement says where to look before labelling, not which
/// model is right (`docs/concepts/rubrics-cases-recordings.md`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Agreement {
    /// The directory the other answers were replayed from.
    pub against: String,
    /// The distinct models its recordings name, in the order first seen.
    pub models: Vec<String>,
    /// The confidence bar the `confident_*` counts use, from `--confident-at`.
    pub confident_at: f64,
    /// The requests answered on both sides.
    pub requests: usize,
    /// One per question, in the rubric's order: `test` is `against`,
    /// `original` the run's answers and `perturbed` DIR's.
    pub comparisons: Vec<Comparison>,
}

impl Agreement {
    /// Pair each of the run's answers with DIR's answer to the same
    /// request, question by question. `ours` and `theirs` come from the
    /// same plan, so they are in the same order.
    pub(crate) fn build(
        loaded: &Loaded,
        ours: &[Answered],
        theirs: &[Answered],
        against: &Path,
        confident_at: f64,
    ) -> Self {
        let correct = |a: &Answered, id: &str| {
            a.judgments
                .iter()
                .find(|(q, _)| q == id)
                .and_then(|(_, j)| j.correct)
        };
        let mut comparisons = Vec::new();
        for id in loaded.rubric.questions.keys() {
            let pairs: Vec<Pair> = ours
                .iter()
                .zip(theirs)
                .filter_map(|(a, b)| {
                    debug_assert_eq!(a.unit.name, b.unit.name, "one plan, one order");
                    let mut p = pair(
                        &a.unit.name,
                        a.response.answers.get(id),
                        b.response.answers.get(id),
                        confident_at,
                    )?;
                    p.correct = correct(a, id).zip(correct(b, id));
                    Some(p)
                })
                .collect();
            if !pairs.is_empty() {
                comparisons.push(summarise(id.clone(), Test::Against, &pairs));
            }
        }
        let mut models: Vec<String> = Vec::new();
        for b in theirs {
            if !models.contains(&b.response.model) {
                models.push(b.response.model.clone());
            }
        }
        Self {
            against: against.display().to_string(),
            models,
            confident_at,
            requests: theirs.len(),
            comparisons,
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// The comparisons as a Markdown table, then the cases that moved, each
/// line opening with `moved`: what both reports print, ready to paste
/// into a pull request or a page. `columns` names the counts, the change
/// and the two accuracies; `test` adds the test's column.
fn table(
    f: &mut Formatter<'_>,
    comparisons: &[Comparison],
    confident_at: f64,
    columns: [&str; 4],
    test: bool,
    moved: impl Fn(&Comparison) -> String,
) -> fmt::Result {
    let [change, confident, original, perturbed] = columns;
    let (test_head, test_rule) = if test { (" test |", "---|") } else { ("", "") };
    writeln!(
        f,
        "| question |{test_head} pairs | {change} | {confident} ≥ {confident_at:.2} | mean \\|Δp\\| | {original} | {perturbed} |"
    )?;
    writeln!(f, "|---|{test_rule}---:|---|---|---:|---|---|")?;
    for c in comparisons {
        let test_cell = if test {
            format!(" {} |", c.test.label())
        } else {
            String::new()
        };
        writeln!(
            f,
            "| {} |{test_cell} {} | {} | {} | {} | {} | {} |",
            out::plain(&c.question),
            c.pairs,
            rate(c.flip_rate, c.flip_rate_interval95, c.flips, c.pairs),
            rate(
                c.confident_flip_rate,
                c.confident_flip_rate_interval95,
                c.confident_flips,
                c.confident_pairs
            ),
            c.mean_abs_dp
                .map_or_else(|| "-".to_owned(), |v| format!("{v:.3}")),
            rate(
                c.original.accuracy,
                c.original.accuracy_interval95,
                c.original.correct,
                c.original.labelled
            ),
            rate(
                c.perturbed.accuracy,
                c.perturbed.accuracy_interval95,
                c.perturbed.correct,
                c.perturbed.labelled
            ),
        )?;
    }
    for c in comparisons.iter().filter(|c| !c.flipped.is_empty()) {
        writeln!(f)?;
        let cases: Vec<String> = c.flipped.iter().map(|n| out::plain(n)).collect();
        writeln!(f, "{}: {}", moved(c), cases.join(", "))?;
    }
    Ok(())
}

/// The swap and tone comparisons: how far the verdicts move when only the
/// request's form does.
impl Display for Robustness {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "robustness ({} perturbed request{}; rates with 95% Wilson intervals)",
            self.perturbed_requests,
            plural(self.perturbed_requests)
        )?;
        writeln!(f)?;
        table(
            f,
            &self.comparisons,
            self.confident_at,
            [
                "flip rate",
                "flip rate, confidence",
                "accuracy, original",
                "accuracy, perturbed",
            ],
            true,
            |c| {
                format!(
                    "flipped under {} on {}",
                    c.test.label(),
                    out::plain(&c.question)
                )
            },
        )
    }
}

/// The run's answers beside DIR's, with the reminder that agreeing is not
/// being right.
impl Display for Agreement {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let models: Vec<String> = self.models.iter().map(|m| out::plain(m)).collect();
        writeln!(
            f,
            "agreement with {} ({}; {} request{}; rates with 95% Wilson intervals; agreement is not accuracy)",
            out::plain(&self.against),
            models.join(", "),
            self.requests,
            plural(self.requests)
        )?;
        writeln!(f)?;
        table(
            f,
            &self.comparisons,
            self.confident_at,
            [
                "differs",
                "differs, confidence",
                "accuracy",
                "accuracy, against",
            ],
            false,
            |c| format!("differs on {}", out::plain(&c.question)),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use judgment::question::Question;
    use serde_json::json;

    fn choice_q(keys: &[&str]) -> Question {
        Question::Choice {
            instructions: json!("which?"),
            criteria: keys
                .iter()
                .map(|k| ((*k).to_owned(), json!(format!("text {k}"))))
                .collect(),
        }
    }

    #[test]
    fn a_swap_reverses_a_choice_and_keeps_each_key_with_its_text() {
        let Some(Question::Choice { criteria, .. }) = reversed(&choice_q(&["a", "b", "c"])) else {
            panic!("a Choice of three reverses");
        };
        let order: Vec<(&str, &Value)> = criteria.iter().map(|(k, v)| (k.as_str(), v)).collect();
        assert_eq!(
            order,
            [
                ("c", &json!("text c")),
                ("b", &json!("text b")),
                ("a", &json!("text a"))
            ]
        );
        assert!(
            reversed(&choice_q(&["only"])).is_none(),
            "one option has no order"
        );
        let noul = Question::Noul {
            instructions: json!("?"),
            criteria: None,
        };
        assert!(reversed(&noul).is_none(), "a Noul has no options");
    }

    #[test]
    fn the_joint_answers_leave_the_response_under_their_question() {
        let mut copies = Questions::new();
        copies.add("better.swapped", choice_q(&["b", "a"])).unwrap();
        let answer = |pick: &str| Answer::Choice {
            choice: pick.to_owned(),
            probabilities: BTreeMap::new(),
            confidence: judgment::Confidence::new(0.5).unwrap(),
        };
        let mut response: Response =
            serde_json::from_value(json!({"model": "m", "answers": {}})).unwrap();
        response.answers.insert("better".into(), answer("a"));
        response
            .answers
            .insert("better.swapped".into(), answer("b"));
        let (rest, joint) = split_joint(response, &copies);
        assert_eq!(rest.answers.keys().collect::<Vec<_>>(), ["better"]);
        assert!(
            matches!(joint.get("better"), Some(Answer::Choice { choice, .. }) if choice == "b")
        );
    }

    #[test]
    fn a_pair_reads_a_noul_and_a_score_as_eval_grades_them_and_skips_a_box() {
        let noul = |p: f64| Answer::Noul {
            noul: judgment::Probability::new(p).unwrap(),
        };
        let p = pair("c", Some(&noul(0.93)), Some(&noul(0.2)), 0.5).unwrap();
        assert_eq!((p.original.as_str(), p.perturbed.as_str()), ("yes", "no"));
        assert!(p.flipped && p.confident);
        assert!((p.abs_dp - 0.73).abs() < 1e-12, "on yes: 0.93 to 0.2");
        let at_the_bar = pair("c", Some(&noul(0.93)), Some(&noul(0.2)), 0.93).unwrap();
        assert!(at_the_bar.confident, "confident from the bar, not above it");

        let score = |dist: [f64; 3]| -> Answer {
            serde_json::from_value(json!({"type": "score", "score": 1.0,
                "legend": {"0": "calm", "1": "annoyed", "2": "angry"},
                "probabilities": {"0": dist[0], "1": dist[1], "2": dist[2]},
                "confidence": 0.4}))
            .unwrap()
        };
        let p = pair(
            "c",
            Some(&score([0.3, 0.6, 0.1])),
            Some(&score([0.1, 0.7, 0.2])),
            0.5,
        )
        .unwrap();
        assert!(!p.flipped, "level 1 both times");
        assert!(!p.confident, "0.4 is below the bar");
        assert!(
            (p.abs_dp - 0.1).abs() < 1e-12,
            "on level 1: 0.6 to 0.7, a distance"
        );

        let boxed = Answer::Box {
            edges: judgment::BoxEdges::new(0.1, 0.5, 0.1, 0.5),
            confidence: None,
        };
        assert!(
            pair("c", Some(&boxed), Some(&boxed), 0.5).is_none(),
            "a Box has no pick"
        );
        assert!(
            pair("c", Some(&noul(0.9)), None, 0.5).is_none(),
            "nothing to compare"
        );
    }

    #[test]
    fn the_totals_count_flips_confident_flips_and_both_accuracies() {
        let pair =
            |flipped: bool, confident: bool, label: &str, original: &str, perturbed: &str| Pair {
                case: format!("{original}{perturbed}"),
                flipped,
                confident,
                abs_dp: if flipped { 0.4 } else { 0.0 },
                correct: Some((original == label, perturbed == label)),
                original: original.to_owned(),
                perturbed: perturbed.to_owned(),
            };
        let pairs = [
            pair(false, true, "a", "a", "a"),
            pair(true, true, "a", "a", "b"),
            pair(true, false, "b", "a", "b"),
            pair(false, false, "b", "b", "b"),
        ];
        let c = summarise("better".into(), Test::Swap, &pairs);
        assert_eq!(
            (c.pairs, c.flips, c.confident_pairs, c.confident_flips),
            (4, 2, 2, 1)
        );
        assert_eq!(c.flip_rate, Some(0.5));
        assert_eq!(c.confident_flip_rate, Some(0.5));
        assert!((c.mean_abs_dp.unwrap() - 0.2).abs() < 1e-12);
        assert_eq!(
            (c.original.correct, c.perturbed.correct, c.original.labelled),
            (3, 3, 4)
        );
        assert_eq!(c.flipped, ["ab", "ab"]);
        let (low, high) = c.flip_rate_interval95.unwrap();
        assert!(low < 0.5 && high > 0.5, "Wilson brackets the rate");
    }
}
