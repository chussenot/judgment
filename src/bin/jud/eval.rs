//! `jud eval RUBRIC CASES [--replay DIR]`: answer every case, grade each
//! answer against its labels and report what the model got right and what
//! the policy does with it (decision 0021). Read-only; the one subcommand
//! that can exit with status 3.
//!
//! One [`Report`] is built from the answered units and rendered twice, as
//! text (`Display`) and as JSON (`Serialize`), so the two cannot say
//! different things. Everything it holds is the library's: the metrics are
//! `eval::QuestionMetrics`, the judgments `jud::grade`'s, the gate's verdicts
//! `Rubric::apply`'s; this file only joins them and decides the exit status.

use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, ValueHint};
use judgment::eval::{ECE_BINS, Judgment, QuestionMetrics};
use judgment::jud::{Rubric, Verdict};
use serde::Serialize;

use crate::EXIT_UNMET;
use crate::backend::{Backend, Failure};
use crate::batch::{self, Answered, Loaded};
use crate::out;

/// Grade a model's answers against the labelled cases.
///
/// Answers every case from the configured backend, or from the recordings in
/// DIR with --replay (no key, no network), and grades each against its
/// expected answer: per question the accuracy with its 95% interval, the Brier
/// score, the calibration error, how often the rubric's gate acts rather than
/// defers and how often it is right when it acts, and every miss by the
/// model. With --min-accuracy the command exits with status 3 when a question
/// falls short, which makes it a CI check. Against a server eval keeps
/// nothing: use `jud record` to keep the answers.
#[derive(Args)]
pub(crate) struct Eval {
    /// The Rubric document the cases are for.
    #[arg(value_name = "RUBRIC", value_hint = ValueHint::FilePath)]
    pub rubric: String,
    /// The Cases document to grade.
    #[arg(value_name = "CASES", value_hint = ValueHint::FilePath)]
    pub cases: String,
    /// Answer from the recordings in this directory instead of a server.
    #[arg(long, env = "JUD_REPLAY", value_name = "DIR", value_hint = ValueHint::DirPath)]
    pub replay: Option<PathBuf>,
    /// Print the report as one JSON object on stdout instead of text.
    #[arg(long)]
    pub json: bool,
    /// Fail with status 3 when accuracy is below this, 0 to 1: `0.9` for
    /// every labelled question, `desk=0.95` for one. Repeatable. It is the
    /// model's accuracy per question, not the policy's. A question with no
    /// labelled case does not meet its bar.
    #[arg(long, value_name = "[QUESTION=]ACCURACY", value_parser = parse_min_accuracy)]
    pub min_accuracy: Vec<MinAccuracy>,
}

/// One `--min-accuracy` gate: the bar, and the question it applies to
/// (every question that has labels when `None`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MinAccuracy {
    pub question: Option<String>,
    pub accuracy: f64,
}

fn parse_min_accuracy(text: &str) -> Result<MinAccuracy, String> {
    let (question, value) = match text.split_once('=') {
        Some((q, v)) if !q.is_empty() => (Some(q.to_owned()), v),
        Some(_) => return Err("the question before `=` is empty".to_owned()),
        None => (None, text),
    };
    let accuracy: f64 = value
        .parse()
        .map_err(|_| format!("`{value}` is not a number from 0 to 1"))?;
    if !(0.0..=1.0).contains(&accuracy) {
        return Err(format!("{accuracy} is not from 0 to 1"));
    }
    Ok(MinAccuracy { question, accuracy })
}

/// How far below its bar an accuracy may sit and still meet it. Accuracy is
/// a ratio of counts and the bar is typed by a person; this absorbs the last
/// bit of float representation (`9/10` against `0.9`) and nothing else, so
/// 6/7 never rounds into a pass at `0.86`.
const TOLERANCE: f64 = 1e-9;

/// Everything `jud eval` says, as data. The JSON form is this struct as
/// serialised, one object on stdout; its key names are `snake_case` and stable
/// (the command's output is part of its contract, `docs/reference/stability.md`):
///
/// ```text
/// {
///   "rubric":  { "name", "fingerprint", "policy_fingerprint" },
///   "cases":   { "name", "fingerprint", "count" },
///   "requests": N,                       // units asked: a conversation
///                                        // labelled with `from_turn` is one per turn
///   "models":  ["jev-1.13.0", ...],      // distinct `model`s the responses named
///   "questions": [                       // the rubric's order, every question
///     {
///       "id", "labelled", "correct",
///       "accuracy", "accuracy_interval95",   // [low, high], Wilson
///       "brier", "ece",
///       "confidence_when_right", "confidence_when_wrong",
///                                        // all null while "labelled" is 0;
///                                        // all of these read the MODEL's answer
///                                        // (a Noul at 0.5, a Choice by its pick,
///                                        // a Score by its most probable level)
///       "gate": { "acted", "deferred", "accuracy_when_acted" } | null,
///                                        // the POLICY's: what the gate did with
///                                        // the answers, see `GateCoverage`
///       "misses": [ { "case", "expected", "predicted", "confidence" } ]
///                                        // the model's misses, as `correct` counts
///     }
///   ],
///   "min_accuracy": [                    // one per --min-accuracy check, [] without any
///     { "question": "desk" | null, "bar", "labelled", "accuracy", "met" }
///   ]
/// }
/// ```
///
/// Numbers are numbers, unrounded; a value that cannot be computed is
/// `null`, never a string. A `case` is the case's id (`#3` for one without),
/// or `ID-turn-N` for a turn of a conversation. `--min-accuracy` holds the
/// question's `accuracy`, the model's, and never `accuracy_when_acted`.
#[derive(Debug, Serialize)]
struct Report {
    /// The rubric the cases were graded against.
    rubric: RubricInfo,
    /// The cases that were asked.
    cases: CasesInfo,
    /// How many requests were answered.
    requests: usize,
    /// The distinct models the responses named, in the order first seen.
    models: Vec<String>,
    /// One entry per question of the rubric, in its order.
    questions: Vec<QuestionReport>,
    /// The `--min-accuracy` checks, in the order given.
    min_accuracy: Vec<BarCheck>,
}

/// The rubric, named by what it asks and what reads its answers.
#[derive(Debug, Serialize)]
struct RubricInfo {
    name: String,
    /// Of the questions: what the model is sent.
    fingerprint: String,
    /// Of the policy: what reads the answers, so a moved bar is as visible
    /// as a changed question.
    policy_fingerprint: String,
}

/// The cases, named by their fingerprint so an edited set is another one.
#[derive(Debug, Serialize)]
struct CasesInfo {
    name: String,
    fingerprint: String,
    /// Cases in the document, not requests: a conversation may be several.
    count: usize,
}

/// One question's metrics, gate and misses.
#[derive(Debug, Serialize)]
struct QuestionReport {
    id: String,
    /// Judgments with a label.
    labelled: usize,
    correct: usize,
    accuracy: Option<f64>,
    accuracy_interval95: Option<(f64, f64)>,
    brier: Option<f64>,
    ece: Option<f64>,
    confidence_when_right: Option<f64>,
    confidence_when_wrong: Option<f64>,
    /// What the rubric's gate for this question does with the answers;
    /// `None` when the policy has no gate for it.
    gate: Option<GateCoverage>,
    misses: Vec<Miss>,
}

/// What the policy's gate does with the answers: acts on them, or defers them.
///
/// A Noul's gate never defers (it says yes or no at its threshold), so its
/// `acted` is every request that asked it. `accuracy_when_acted` is the
/// accuracy of the policy's own verdicts against the labels, over the acted
/// requests that carry one: a Noul's yes or no at the gate's threshold (and
/// `strict`), a Choice's option, a Score's level nearest to the weighted score.
/// That is not the question's accuracy, which reads the model's answer (a Noul
/// at 0.5, a Choice by its pick, a Score by its most probable level), so
/// moving a bar moves this number and leaves that one where it is. A deferred
/// answer is not acted on and is in neither count.
#[derive(Debug, Serialize)]
struct GateCoverage {
    acted: usize,
    deferred: usize,
    accuracy_when_acted: Option<f64>,
}

/// One labelled answer the model got wrong: the question's own accuracy
/// counts it, whatever the policy then did with it.
#[derive(Debug, Serialize)]
struct Miss {
    case: String,
    expected: String,
    predicted: String,
    /// The model's confidence in what it predicted.
    confidence: f64,
}

/// One `--min-accuracy` check: a bar and the accuracy of one question.
#[derive(Debug, Serialize)]
struct BarCheck {
    /// The question, or `None` when a bare bar found no question with labels.
    question: Option<String>,
    bar: f64,
    labelled: usize,
    accuracy: Option<f64>,
    /// False when the accuracy is below the bar, or there is none to show.
    met: bool,
}

impl BarCheck {
    fn new(question: Option<&QuestionReport>, bar: f64) -> Self {
        let accuracy = question.and_then(|q| q.accuracy);
        Self {
            question: question.map(|q| q.id.clone()),
            bar,
            labelled: question.map_or(0, |q| q.labelled),
            accuracy,
            // A question nobody labelled cannot be shown to meet a bar, so
            // an unlabelled one fails it, even at 0.
            met: accuracy.is_some_and(|a| a >= bar - TOLERANCE),
        }
    }

    /// `desk 0.86 < 0.95`: the comparison, with enough digits that the
    /// accuracy shown really is below (or at) the bar it is set against.
    fn describe(&self) -> String {
        let relation = if self.met { ">=" } else { "<" };
        match (&self.question, self.accuracy) {
            (Some(q), Some(a)) => {
                format!("{q} {} {relation} {}", shown_against(a, self.bar), self.bar)
            }
            (Some(q), None) => format!("{q} not labelled {relation} {}", self.bar),
            (None, _) => format!("(no question is labelled) {relation} {}", self.bar),
        }
    }
}

/// `accuracy` with the fewest decimals, two at least, at which the shown
/// number compares with `bar` as the real one does. 6/7 against `0.86`
/// prints `0.857 < 0.86`, never the baffling `0.86 < 0.86`.
fn shown_against(accuracy: f64, bar: f64) -> String {
    let meets = |a: f64| a >= bar - TOLERANCE;
    for digits in 2..=6 {
        let text = format!("{accuracy:.digits$}");
        if text
            .parse::<f64>()
            .is_ok_and(|shown| meets(shown) == meets(accuracy))
        {
            return text;
        }
    }
    accuracy.to_string()
}

#[allow(clippy::cast_precision_loss)] // counts, far below 2^52
fn ratio(correct: usize, labelled: usize) -> Option<f64> {
    (labelled > 0).then(|| correct as f64 / labelled as f64)
}

impl Report {
    /// Join the answered units with the rubric: metrics per question over
    /// every labelled judgment, the gate's coverage, the misses, and each
    /// `--min-accuracy` check. Pure: nothing is asked, nothing is printed.
    fn build(loaded: &Loaded, answered: &[Answered], bars: &[MinAccuracy]) -> Self {
        let questions: Vec<QuestionReport> = batch::by_question(&loaded.rubric, answered)
            .iter()
            .map(|(id, judgments)| question_report(&loaded.rubric, id, judgments, answered))
            .collect();
        let min_accuracy = bars
            .iter()
            .flat_map(|bar| checks(&questions, bar))
            .collect();
        let mut models: Vec<String> = Vec::new();
        for a in answered {
            if !models.contains(&a.response.model) {
                models.push(a.response.model.clone());
            }
        }
        Self {
            rubric: RubricInfo {
                name: loaded.rubric.name.clone(),
                fingerprint: loaded.rubric.fingerprint(),
                policy_fingerprint: loaded.rubric.policy_fingerprint(),
            },
            cases: CasesInfo {
                name: loaded.cases.name.clone(),
                fingerprint: loaded.cases.fingerprint(),
                count: loaded.cases.cases.len(),
            },
            requests: answered.len(),
            models,
            questions,
            min_accuracy,
        }
    }

    /// The checks that are not met.
    fn unmet(&self) -> Vec<&BarCheck> {
        self.min_accuracy.iter().filter(|c| !c.met).collect()
    }

    /// The line `jud eval` writes on stderr when a bar is not met, without
    /// the `jud: ` prefix; `None` when every check is met.
    fn unmet_line(&self) -> Option<String> {
        let unmet = self.unmet();
        if unmet.is_empty() {
            return None;
        }
        // Two bars on one question are two entries but one question.
        let questions: BTreeSet<Option<&str>> =
            unmet.iter().map(|c| c.question.as_deref()).collect();
        let entries: Vec<String> = unmet.iter().map(|c| c.describe()).collect();
        Some(format!(
            "{} question(s) below --min-accuracy: {}",
            questions.len(),
            entries.join(", ")
        ))
    }
}

/// One question's report: the model's metrics over its labelled `judgments`,
/// what the policy's gate did, and where the model was wrong.
fn question_report(
    rubric: &Rubric,
    id: &str,
    judgments: &[Judgment],
    answered: &[Answered],
) -> QuestionReport {
    let metrics = QuestionMetrics::summarise(judgments, ECE_BINS);
    QuestionReport {
        id: id.to_owned(),
        labelled: metrics.labelled,
        correct: metrics.correct,
        accuracy: metrics.accuracy,
        accuracy_interval95: metrics.accuracy_interval95,
        brier: metrics.brier,
        ece: metrics.ece,
        confidence_when_right: metrics.confidence_when_right,
        confidence_when_wrong: metrics.confidence_when_wrong,
        gate: rubric
            .policy
            .gates
            .contains_key(id)
            .then(|| coverage(id, answered)),
        misses: misses(id, answered),
    }
}

/// The labelled answers the model got wrong, in case order. These are the
/// model's misses (what `correct` counts), not the policy's: a Noul at 0.52
/// is a miss against a `false` label here even where the gate's threshold of
/// 0.55 answers `no` and is right.
fn misses(id: &str, answered: &[Answered]) -> Vec<Miss> {
    answered
        .iter()
        .flat_map(|a| {
            a.judgments
                .iter()
                .filter(|(question, judgment)| question == id && judgment.correct == Some(false))
                .filter_map(|(_, judgment)| {
                    Some(Miss {
                        case: a.unit.name.clone(),
                        expected: judgment.expected.clone()?,
                        predicted: judgment.predicted.clone(),
                        confidence: judgment.confidence,
                    })
                })
        })
        .collect()
}

/// Whether the policy's `verdict` is the answer `judgment`'s label asks for;
/// `None` for an answer with no label. A verdict is compared in the label's
/// own words: `yes` or `no` for a Noul, the option for a Choice, the level's
/// index for a Score. The verdict has already applied the gate's bar and
/// `strict`, and a Score's is the level nearest the weighted score where the
/// model's own reading is its most probable one, so this is what the policy
/// does and `Judgment::correct` is not.
fn policy_is_right(verdict: &Verdict, judgment: &Judgment) -> Option<bool> {
    let expected = judgment.expected.as_deref()?;
    let decided = match verdict {
        Verdict::Yes { .. } => "yes".to_owned(),
        Verdict::No { .. } => "no".to_owned(),
        Verdict::Option { key, .. } => key.clone(),
        Verdict::Level { index, .. } => index.to_string(),
        // A verdict this build does not know acts on the model's reading, so
        // a new kind of verdict cannot quietly shrink the labelled count.
        _ => return judgment.correct,
    };
    Some(decided == expected)
}

/// What the gate for `id` did over the units that asked the question: a
/// verdict and a judgment of one unit meet by question id, so the accuracy
/// among the acted answers is over the same answers the gate let through.
/// A unit that did not ask the question (its `when` was absent) has no
/// verdict and counts for neither side; a deferred one is not acted on.
fn coverage(id: &str, answered: &[Answered]) -> GateCoverage {
    let (mut acted, mut deferred, mut labelled, mut right) = (0, 0, 0, 0);
    for a in answered {
        let Some(verdict) = a.verdicts.get(id) else {
            continue;
        };
        if matches!(verdict, Verdict::Deferred(_)) {
            deferred += 1;
            continue;
        }
        acted += 1;
        let graded = a
            .judgments
            .iter()
            .find(|(question, _)| question == id)
            .and_then(|(_, judgment)| policy_is_right(verdict, judgment));
        if let Some(is_right) = graded {
            labelled += 1;
            right += usize::from(is_right);
        }
    }
    GateCoverage {
        acted,
        deferred,
        accuracy_when_acted: ratio(right, labelled),
    }
}

/// The checks one `--min-accuracy` flag makes: one for the question it
/// names, or one for each question that has labels. A bare bar over a
/// rubric where nothing is labelled is one check that fails: with no labels
/// there is nothing to hold it to, and a CI gate that passes because nothing
/// was measured is the failure it exists to catch.
fn checks(questions: &[QuestionReport], bar: &MinAccuracy) -> Vec<BarCheck> {
    if let Some(id) = &bar.question {
        return vec![BarCheck::new(
            questions.iter().find(|q| &q.id == id),
            bar.accuracy,
        )];
    }
    let labelled: Vec<BarCheck> = questions
        .iter()
        .filter(|q| q.labelled > 0)
        .map(|q| BarCheck::new(Some(q), bar.accuracy))
        .collect();
    if labelled.is_empty() {
        vec![BarCheck::new(None, bar.accuracy)]
    } else {
        labelled
    }
}

/// `-` for a number that could not be computed, else `value` to `digits`.
fn fixed(value: Option<f64>, digits: usize) -> String {
    value.map_or_else(|| "-".to_owned(), |v| format!("{v:.digits$}"))
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "rubric {}: questions {}, policy {}",
            self.rubric.name, self.rubric.fingerprint, self.rubric.policy_fingerprint
        )?;
        writeln!(
            f,
            "cases {}: {}, {} case{}",
            self.cases.name,
            self.cases.fingerprint,
            self.cases.count,
            plural(self.cases.count)
        )?;
        write!(f, "{} request{}", self.requests, plural(self.requests))?;
        // A model's name comes from the server or a recording: whatever it
        // holds is shown as text, never as a terminal command. The JSON
        // form carries it verbatim, as a string.
        let models: Vec<String> = self.models.iter().map(|m| out::plain(m)).collect();
        match models.as_slice() {
            [] => {}
            [one] => write!(f, ", model {one}")?,
            many => write!(f, ", models {}", many.join(", "))?,
        }
        writeln!(f)?;

        for q in &self.questions {
            writeln!(f)?;
            q.fmt_block(f)?;
        }

        writeln!(f)?;
        self.fmt_misses(f)?;

        if !self.min_accuracy.is_empty() {
            writeln!(f)?;
            writeln!(f, "--min-accuracy")?;
            for check in &self.min_accuracy {
                let status = if check.met { "met" } else { "NOT MET" };
                writeln!(f, "  {status:<8} {}", check.describe())?;
            }
        }
        Ok(())
    }
}

impl Report {
    /// Every miss of the model, one line each, the question first so the
    /// lines line up. Titled for what it is: the model's answers that the
    /// labels say were wrong, which is not what the gate section counts.
    fn fmt_misses(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let total: usize = self.questions.iter().map(|q| q.misses.len()).sum();
        if total == 0 {
            return writeln!(f, "model misses: none");
        }
        writeln!(f, "model misses ({total})")?;
        let width = self.questions.iter().map(|q| q.id.len()).max().unwrap_or(0);
        for q in &self.questions {
            for m in &q.misses {
                writeln!(
                    f,
                    "  {:<width$}  {}: expected {}, predicted {}, confidence {:.2}",
                    q.id, m.case, m.expected, m.predicted, m.confidence
                )?;
            }
        }
        Ok(())
    }
}

impl QuestionReport {
    /// The question's block: its id, the metrics, the gate.
    fn fmt_block(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.id)?;
        if self.labelled == 0 {
            writeln!(f, "  not labelled")?;
        } else {
            let interval = self
                .accuracy_interval95
                .map_or_else(String::new, |(low, high)| {
                    format!(" (95% interval {low:.2} to {high:.2})")
                });
            writeln!(
                f,
                "  labelled {}, correct {}, accuracy {}{interval}",
                self.labelled,
                self.correct,
                fixed(self.accuracy, 2)
            )?;
            writeln!(
                f,
                "  brier {}, calibration error {}, confidence when right {}, when wrong {}",
                fixed(self.brier, 3),
                fixed(self.ece, 3),
                fixed(self.confidence_when_right, 2),
                fixed(self.confidence_when_wrong, 2)
            )?;
        }
        match &self.gate {
            None => writeln!(f, "  gate: none"),
            Some(g) => writeln!(
                f,
                "  gate: acts on {} of {}, defers {}, accuracy when acted {}",
                g.acted,
                g.acted + g.deferred,
                g.deferred,
                fixed(g.accuracy_when_acted, 2)
            ),
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// A `--min-accuracy` that names a question the rubric does not ask is a
/// typo, and a typo that silently gates nothing passes CI for the wrong
/// reason: refuse it before any call, with the questions to choose from.
fn check_bars(rubric: &Rubric, bars: &[MinAccuracy], rubric_path: &str) -> Result<(), Failure> {
    for bar in bars {
        let Some(id) = &bar.question else { continue };
        if !rubric.questions.contains_key(id) {
            let known: Vec<&str> = rubric.questions.keys().map(String::as_str).collect();
            return Err(Failure::Usage(format!(
                "--min-accuracy names question `{id}`, which {rubric_path} does not ask; its questions are: {}",
                known.join(", ")
            )));
        }
    }
    Ok(())
}

/// Load, validate, answer and report; every failure is before the output.
fn evaluate(args: &Eval) -> Result<Report, Failure> {
    let loaded = batch::load(&args.rubric, &args.cases)?;
    check_bars(&loaded.rubric, &args.min_accuracy, &args.rubric)?;
    let backend = Backend::open(args.replay.as_deref())?;
    let planned = batch::plan(&loaded)?;
    let answered = batch::answer_all(&backend, &loaded, planned)?;
    Ok(Report::build(&loaded, &answered, &args.min_accuracy))
}

fn render(report: &Report, json: bool) -> Result<String, Failure> {
    if json {
        let mut text = serde_json::to_string_pretty(report)
            .map_err(|e| Failure::Backend(format!("cannot serialise the report: {e}")))?;
        text.push('\n');
        Ok(text)
    } else {
        Ok(report.to_string())
    }
}

pub(crate) fn run(args: &Eval) -> ExitCode {
    let report = match evaluate(args) {
        Ok(report) => report,
        Err(failure) => return failure.report(),
    };
    // The report is complete on stdout before the status says whether a bar
    // was met, and a reader that has gone does not change the status
    // (`out::result`).
    if let Err(failure) = render(&report, args.json).and_then(|text| out::result(&text)) {
        return failure.report();
    }
    match report.unmet_line() {
        Some(line) => {
            out::note!("jud: {line}");
            ExitCode::from(EXIT_UNMET)
        }
        None => ExitCode::SUCCESS,
    }
}
