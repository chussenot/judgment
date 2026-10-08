//! `jud tune RUBRIC CASES --replay DIR`: propose the bars of the rubric's
//! gates from recorded answers (decision 0021). Never calls a backend and
//! never rewrites its input.
//!
//! The run is the loop of `examples/jud_calibration.rs` over files: answer
//! every case from the recordings, grade the answers against the labels,
//! sweep each gate, and print what the sweep says. The tables and every note
//! go to stderr; stdout is the data, the proposed `policy` and `tuning` blocks
//! a person pastes under `spec:`. A proposal is a person's to read first, so
//! nothing is written unless `--out` names a file, and never over the input
//! or among the recordings.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, ValueHint};
use indexmap::IndexMap;
use judgment::eval::tuning::{
    GateRow, LevelRow, ThresholdRow, best_level, best_threshold, default_bars, default_thresholds,
    gate_table, level_sweep, lowest_bar, threshold_sweep,
};
use judgment::eval::{ECE_BINS, Judgment, QuestionMetrics, canonical};
use judgment::jud::{Gate, LevelRef, Rubric, Tuning};
use judgment::{Question, eval};
use serde::Serialize;
use serde_json::Value;

use crate::backend::{self, Backend, Failure};
use crate::batch::{self, Answered};
use crate::fsutil;
use crate::out::{self, note};
use crate::recordings;

/// An accuracy interval wider than this says too few cases were read.
const THIN_INTERVAL: f64 = 0.2;

/// How many case names a refusal lists before it says "and N more".
const NAMES_SHOWN: usize = 5;

/// Propose each gate's bar from recorded answers.
///
/// Reads the recordings in DIR (it never calls a backend), grades them
/// against the cases, sweeps each gate and prints a proposal: a Noul's
/// threshold by best F1, a Choice's or Score's confidence bar as the lowest
/// that keeps --target-accuracy over at least --min-covered cases, a Score's
/// `level_at_least` by best F1. The tables go to stderr. Stdout carries the
/// proposed policy and tuning blocks as YAML, starting at column 0: paste
/// them under `spec:`, indented two spaces. --out writes the whole rubric
/// with the proposal applied instead.
#[derive(Args)]
pub(crate) struct Tune {
    /// The Rubric document whose gates are tuned.
    #[arg(value_name = "RUBRIC", value_hint = ValueHint::FilePath)]
    pub rubric: String,
    /// The Cases document the gates are tuned on.
    #[arg(value_name = "CASES", value_hint = ValueHint::FilePath)]
    pub cases: String,
    /// The recordings to tune from; `jud record` writes them.
    #[arg(long, env = "JUD_REPLAY", value_name = "DIR", required = true, value_hint = ValueHint::DirPath)]
    pub replay: PathBuf,
    /// The accuracy a Choice's or Score's confidence bar must keep, 0 to 1.
    #[arg(long, value_name = "ACCURACY", default_value_t = 0.95)]
    pub target_accuracy: f64,
    /// The fewest cases a confidence bar must still cover.
    #[arg(long, value_name = "N", default_value_t = 3)]
    pub min_covered: usize,
    /// Write the whole rubric with the proposal applied to this file, instead
    /// of printing the blocks. The rubric is serialised again: its comments
    /// and layout are lost and `version` is written as text. Never the
    /// rubric, the cases or a file in DIR.
    #[arg(long, value_name = "PATH", value_hint = ValueHint::FilePath)]
    pub out: Option<PathBuf>,
}

pub(crate) fn run(args: &Tune) -> ExitCode {
    match tune(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => failure.report(),
    }
}

/// The whole command. Everything that can be refused is refused before the
/// recordings are read; nothing is printed on stdout or written to a file
/// until every gate has been swept.
fn tune(args: &Tune) -> Result<(), Failure> {
    check_settings(args)?;
    check_out(args)?;
    // Two files recording one request leave a replay to answer from either,
    // so the sweep might read a stale answer: say so before it does.
    recordings::warn_duplicates(&args.replay);
    let loaded = batch::load(&args.rubric, &args.cases)?;
    if loaded.rubric.policy.gates.is_empty() {
        return Err(Failure::Usage(format!(
            "the rubric has no gates to tune; add one per question first, under `policy` in {}",
            args.rubric
        )));
    }
    let planned = batch::plan(&loaded)?;
    let backend = Backend::open(Some(args.replay.as_path()))?;
    let answered = batch::answer_all(&backend, &loaded, planned)?;
    let model = single_model(&answered)?;

    let settings = Settings {
        target_accuracy: args.target_accuracy,
        min_covered: args.min_covered,
    };
    note!(
        "jud: tuning {} on {} answers to {} cases ({}) from the recordings under {}",
        loaded.rubric.name,
        answered.len(),
        loaded.cases.cases.len(),
        loaded.cases.fingerprint(),
        args.replay.display()
    );
    let proposals = propose(&loaded.rubric, &answered, &settings);
    if proposals.is_empty() {
        let file = args.out.as_ref().map_or_else(String::new, |path| {
            format!(" and no file was written to {}", path.display())
        });
        note!("jud: nothing was proposed, so there is no policy or tuning block{file}");
        return Ok(());
    }

    let tuning = Tuning {
        cases: Some(loaded.cases.fingerprint()),
        model,
        server: recorded_server(&args.replay, &answered),
        tuned_at: Some(eval::now_rfc3339()),
        extra: IndexMap::from([("labelled".to_owned(), labelled(&proposals))]),
    };
    let tuned = apply(&loaded.rubric, &proposals, tuning)?;
    let yaml = tuned
        .to_yaml()
        .map_err(|e| Failure::Usage(format!("the tuned rubric cannot be written: {e}")))?;
    match &args.out {
        Some(path) => {
            std::fs::write(path, &yaml)
                .map_err(|e| Failure::Usage(format!("cannot write {}: {e}", path.display())))?;
            note!("jud: wrote the tuned rubric to {}", path.display());
            Ok(())
        }
        None => out::result(&blocks(&yaml)?),
    }
}

/// The numbers a run was given, refused before any file is read.
fn check_settings(args: &Tune) -> Result<(), Failure> {
    if !(0.0..=1.0).contains(&args.target_accuracy) {
        return Err(Failure::Usage(format!(
            "--target-accuracy must be from 0 to 1, got {}",
            args.target_accuracy
        )));
    }
    if args.min_covered == 0 {
        return Err(Failure::Usage(
            "--min-covered must be at least 1, got 0".to_owned(),
        ));
    }
    Ok(())
}

/// `--out` is never an input. Tuning rewrites a document through
/// `Rubric::to_yaml`, which drops the comments the author wrote, so it
/// is only ever done into a new place; the cases are as precious (the labels
/// are the work). Spelled differently is not another file: a path that
/// resolves to the input, through `.`, a symlink or a hard link, is the
/// input. Nor is it among the recordings: a replay reads every `.jud` and
/// `.json` in its directory as one, so a rubric put there makes the whole
/// directory unreadable, and one put over a recording loses an answer that
/// was paid for.
fn check_out(args: &Tune) -> Result<(), Failure> {
    let Some(out) = &args.out else {
        return Ok(());
    };
    for (what, input) in [("rubric", &args.rubric), ("cases", &args.cases)] {
        if fsutil::same_file(out, Path::new(input)) {
            return Err(Failure::Usage(format!(
                "refusing to overwrite the input: --out {} is the {what} {input}",
                out.display()
            )));
        }
    }
    if among_recordings(out, &args.replay) {
        return Err(Failure::Usage(format!(
            "refusing to write into the recordings directory: --out {} is in {}, and a replay reads every .jud and .json there as a recording",
            out.display(),
            args.replay.display()
        )));
    }
    Ok(())
}

/// Whether a file at `out` would be in the recordings directory `dir`, or
/// is one of its recordings under another name (a symlink, a hard link), or
/// is a link whose write would create one there.
fn among_recordings(out: &Path, dir: &Path) -> bool {
    let in_dir = |path: &Path| fsutil::same_file(&fsutil::parent_dir(path), dir);
    in_dir(out)
        || in_dir(&landing(out))
        || recordings::scan(dir)
            .iter()
            .any(|found| fsutil::same_file(out, &found.path))
}

/// Where a write to `path` ends up: `path`, or where the symlinks it is lead,
/// including a link to a file that is not there yet, which a write creates.
/// Followed as many times as the system does before it gives up.
fn landing(path: &Path) -> PathBuf {
    const MAX_LINKS: usize = 40;
    let mut here = path.to_path_buf();
    for _ in 0..MAX_LINKS {
        let Ok(target) = std::fs::read_link(&here) else {
            break;
        };
        // A relative target is relative to the directory the link is in.
        here = here.parent().unwrap_or_else(|| Path::new("")).join(target);
    }
    here
}

/// What a bar must keep to be proposed.
struct Settings {
    target_accuracy: f64,
    min_covered: usize,
}

/// A gate the sweep changed, and how many labelled cases it was read from.
struct Proposal {
    gate: Gate,
    labelled: usize,
}

/// A gate with the sweep's proposal in it, and the bar among its fields that
/// the sweep proposed (`threshold` or `confidence`, not a level), which a
/// strict gate reads the other way at.
struct Proposed {
    gate: Gate,
    bar: Option<f64>,
}

/// Sweep every question of the rubric, in the rubric's order, and collect
/// what it proposes. A question that cannot be tuned says why on stderr.
fn propose(
    rubric: &Rubric,
    answered: &[Answered],
    settings: &Settings,
) -> IndexMap<String, Proposal> {
    let by_question = batch::by_question(rubric, answered);
    let mut proposals = IndexMap::new();
    for (id, declared) in &rubric.questions {
        let Some(gate) = rubric.policy.gates.get(id) else {
            note!("jud: {id}: skipped, it has no gate; add one under `policy` to tune it");
            continue;
        };
        let judgments = by_question.get(id).map_or(&[][..], Vec::as_slice);
        if judgments.is_empty() {
            note!("jud: {id}: skipped, no case labels it");
            continue;
        }
        if let Some(proposal) = tune_question(id, &declared.question, gate, judgments, settings) {
            proposals.insert(id.clone(), proposal);
        }
    }
    proposals
}

/// One question's sweep: the table, the proposal when there is one, and the
/// warning when the cases are too few to trust it. The gate comes back as it
/// was written but for the fields the sweep read off the cases.
fn tune_question(
    id: &str,
    question: &Question,
    gate: &Gate,
    judgments: &[Judgment],
    settings: &Settings,
) -> Option<Proposal> {
    let metrics = QuestionMetrics::summarise(judgments, ECE_BINS);
    let labelled = metrics.labelled;
    let reading = reading(&metrics);
    note!(
        "jud: {id} ({}): {labelled} labelled cases, {reading}",
        question.kind()
    );
    let proposed = match question {
        Question::Noul { .. } => tune_threshold(id, gate, judgments, labelled),
        Question::Choice { .. } => tune_confidence(id, gate, judgments, labelled, settings, None),
        Question::Score { criteria, .. } => tune_confidence(
            id,
            gate,
            judgments,
            labelled,
            settings,
            Some(criteria.as_slice()),
        ),
    };
    if let Some(proposed) = &proposed
        && gate.strict
    {
        note_strict(id, question, judgments, proposed.bar);
    }
    if metrics
        .accuracy_interval95
        .is_some_and(|(low, high)| high - low > THIN_INTERVAL)
    {
        note!(
            "jud: warning: {id}: {labelled} labelled cases, {reading}; a bar read off so few cases is a guess with a number on it"
        );
    }
    proposed.map(|proposed| Proposal {
        gate: proposed.gate,
        labelled,
    })
}

/// A Noul's `threshold`: the best F1 over the sweep, the lowest on a tie.
fn tune_threshold(
    id: &str,
    gate: &Gate,
    judgments: &[Judgment],
    labelled: usize,
) -> Option<Proposed> {
    let rows = threshold_sweep(judgments, &default_thresholds());
    let best = best_threshold(&rows);
    print_threshold_table(&rows, best);
    let Some(threshold) = best else {
        note!(
            "jud: {id}: no threshold has an F1 over {labelled} labelled cases (it needs a case labelled yes and an answer that reaches yes); the gate stays as written"
        );
        return None;
    };
    let note = format!("best F1 on {labelled} labelled cases; ties go to the lower threshold");
    note!(
        "jud: {id}: propose threshold {threshold:.2} (now {}): {note}",
        shown_bar(gate.threshold)
    );
    Some(Proposed {
        gate: Gate {
            threshold: Some(threshold),
            note: Some(note),
            ..gate.clone()
        },
        bar: Some(threshold),
    })
}

/// A Choice's or Score's `confidence`: the lowest bar that keeps the target
/// accuracy over enough cases, and a Score's `level_at_least` by best F1
/// when the gate has one. A gate with `bands` gets its table and nothing
/// else (decision 0021 leaves bands to a later record). `levels` is the
/// Score's own, `None` for a Choice.
fn tune_confidence(
    id: &str,
    gate: &Gate,
    judgments: &[Judgment],
    labelled: usize,
    settings: &Settings,
    levels: Option<&[Value]>,
) -> Option<Proposed> {
    let rows = gate_table(judgments, &default_bars());
    let digits = accuracy_digits(&rows, settings.target_accuracy);
    if !gate.bands.is_empty() {
        print_bar_table(&rows, None, digits);
        note!(
            "jud: {id}: the gate has bands, which jud tune prints the table for and does not propose; the gate stays as written"
        );
        return None;
    }
    let bar = lowest_bar(&rows, settings.target_accuracy, settings.min_covered);
    print_bar_table(&rows, bar, digits);
    let mut proposed = gate.clone();
    let mut notes = Vec::new();
    let mut proposed_bar = None;
    let mut changed = false;
    match bar.and_then(|bar| rows.iter().find(|row| same(row.bar, bar))) {
        Some(row) => {
            let note = format!(
                "lowest bar at {} accuracy; covers {} of {} labelled cases",
                percent(settings.target_accuracy),
                row.covered,
                row.n
            );
            note!(
                "jud: {id}: propose confidence {:.2} (now {}): {note}",
                row.bar,
                shown_bar(gate.confidence)
            );
            if same(row.bar, 0.0) {
                // A bar the table starts at is no bar: say so, since a gate
                // that defers nothing is what the number now means.
                note!(
                    "jud: {id}: the proposed bar is 0, so the gate would defer nothing: the labelled answers are already at least {} right with no bar",
                    percent(settings.target_accuracy)
                );
            }
            if levels.is_some() {
                // The table is the library's: a Score's answer is its most
                // probable level there, where the gate acts on the level
                // nearest the weighted score. They agree on a one-peaked
                // answer and part on a spread one.
                note!(
                    "jud: {id}: the table reads the most probable level as the answer; the policy reads the level nearest the weighted score, which can differ"
                );
            }
            proposed.confidence = Some(row.bar);
            proposed_bar = Some(row.bar);
            notes.push(note);
            changed = true;
        }
        None => note!(
            "jud: {id}: no bar reaches {} accuracy over at least {} cases; the confidence stays as written",
            percent(settings.target_accuracy),
            settings.min_covered
        ),
    }
    if let (Some(levels), Some(written)) = (levels, &gate.level_at_least) {
        let rows = level_sweep(judgments, levels.len());
        let best = best_level(&rows);
        print_level_table(&rows, best);
        if let Some(level) = best {
            let note = format!(
                "level_at_least by best F1 on {labelled} labelled cases; ties go to the lower level"
            );
            note!(
                "jud: {id}: propose level_at_least {} (now {}): level {level}; {note}",
                level_text(levels.get(level), level),
                level_ref_text(written)
            );
            proposed.level_at_least = Some(level_like(written, level, levels));
            notes.push(note);
            changed = true;
        } else {
            note!(
                "jud: {id}: no level has an F1 over {labelled} labelled cases; the level_at_least stays as written"
            );
        }
    }
    if changed {
        proposed.note = Some(notes.join("; "));
        Some(Proposed {
            gate: proposed,
            bar: proposed_bar,
        })
    } else {
        None
    }
}

/// What a strict gate does at the bar that was proposed. A strict gate acts
/// above its bar, the sweep counts an answer at it as acting, so an answer
/// exactly there is read the other way; say how many there are when there
/// are any, and that it can happen when there are none to count (or no bar
/// was proposed, only a level).
fn note_strict(id: &str, question: &Question, judgments: &[Judgment], bar: Option<f64>) {
    let at_bar = bar.map_or(0, |bar| at_the_bar(question, judgments, bar));
    match bar {
        Some(bar) if at_bar > 0 => note!("jud: {id}: {}", strict_at_bar(question, at_bar, bar)),
        _ => note!(
            "jud: {id}: the gate is strict (acts above the bar) and the sweep counts an answer at a bar as acting, so an answer exactly at the proposed bar is read the other way"
        ),
    }
}

/// How many labelled answers sit exactly at `bar`: the probability of yes
/// for a Noul (what its threshold is read against), the confidence for the
/// others, over the answers the table counts.
fn at_the_bar(question: &Question, judgments: &[Judgment], bar: f64) -> usize {
    judgments
        .iter()
        .filter_map(|j| match question {
            Question::Noul { .. } => j
                .expected
                .as_ref()
                .and_then(|_| j.probabilities.get("yes").copied()),
            _ => j.correct.map(|_| j.confidence),
        })
        .filter(|value| same(*value, bar))
        .count()
}

/// The sentence for `n` answers at `bar` under a strict gate: a Choice's or
/// Score's are deferred, a Noul's are a no, which is a verdict and not a
/// deferral.
fn strict_at_bar(question: &Question, n: usize, bar: f64) -> String {
    let (subject, verb) = if n == 1 {
        ("answer", "sits")
    } else {
        ("answers", "sit")
    };
    let effect = match (question, n) {
        (Question::Noul { .. }, 1) => "reads it as no",
        (Question::Noul { .. }, _) => "reads them as no",
        (_, 1) => "defers it",
        (_, _) => "defers them",
    };
    format!("{n} {subject} {verb} exactly at the proposed bar {bar:.2} and a strict gate {effect}")
}

/// The level `level` named the way the gate named its own: an index stays an
/// index, a text becomes the level's text. The text is only used when it
/// reads back as that level (two levels with one text would not), else the
/// index as a string, which the format also accepts.
fn level_like(written: &LevelRef, level: usize, levels: &[Value]) -> LevelRef {
    match written {
        LevelRef::Index(_) => LevelRef::Index(level),
        LevelRef::Text(_) => {
            let named = LevelRef::Text(level_text(levels.get(level), level));
            if named.resolve(levels) == Some(level) {
                named
            } else {
                LevelRef::Text(level.to_string())
            }
        }
    }
}

/// A level's text as the format reads it: a string as it is, anything else
/// as compact JSON; the index when the level is not there at all.
fn level_text(level: Option<&Value>, index: usize) -> String {
    match level {
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
        None => index.to_string(),
    }
}

/// A level as the gate wrote it: its index, or its text.
fn level_ref_text(level: &LevelRef) -> String {
    match level {
        LevelRef::Index(index) => index.to_string(),
        LevelRef::Text(text) => text.clone(),
    }
}

/// What the model got right over the labelled cases, with the interval that
/// says how far to trust it.
fn reading(metrics: &QuestionMetrics) -> String {
    match (metrics.accuracy, metrics.accuracy_interval95) {
        (Some(accuracy), Some((low, high))) => {
            format!("accuracy {accuracy:.2} (95% interval {low:.2}-{high:.2})")
        }
        _ => "no accuracy".to_owned(),
    }
}

/// `0.95` as `95%`, `0.975` as `97.5%`.
fn percent(fraction: f64) -> String {
    let text = format!("{:.2}", fraction * 100.0);
    format!("{}%", text.trim_end_matches('0').trim_end_matches('.'))
}

/// Two grid values are one value; the bars and thresholds are multiples of 0.05.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// A bar as the gate has it: two decimals like the proposal beside it, more
/// when it was written with more, `unset` when the gate has no such field.
fn shown_bar(bar: Option<f64>) -> String {
    bar.map_or_else(
        || "unset".to_owned(),
        |bar| {
            let two = format!("{bar:.2}");
            if two.parse::<f64>().is_ok_and(|shown| same(shown, bar)) {
                two
            } else {
                bar.to_string()
            }
        },
    )
}

/// The decimals the accuracy column of a bar table is printed to: two, and
/// more when a row under the target would otherwise print as the target (18
/// of 19 is 0.947, which two decimals show as the 0.95 asked for, beside a
/// proposal that skips it). One count for the whole column, so it stays
/// aligned, and the comparison is the one `lowest_bar` makes.
fn accuracy_digits(rows: &[GateRow], target: f64) -> usize {
    (2..6)
        .find(|digits| {
            rows.iter().filter_map(|row| row.accuracy).all(|accuracy| {
                let shown: f64 = format!("{accuracy:.digits$}").parse().unwrap_or(accuracy);
                (shown >= target) == (accuracy >= target)
            })
        })
        .unwrap_or(6)
}

fn cell(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), |v| format!("{v:.2}"))
}

fn mark(chosen: bool) -> &'static str {
    if chosen { "  <- proposed" } else { "" }
}

fn print_threshold_table(rows: &[ThresholdRow], best: Option<f64>) {
    note!("  threshold  accuracy  precision  recall  f1");
    for row in rows {
        note!(
            "  {:<9.2}  {:<8.2}  {:<9}  {:<6}  {}{}",
            row.threshold,
            row.accuracy,
            cell(row.precision),
            cell(row.recall),
            cell(row.f1),
            mark(best.is_some_and(|b| same(b, row.threshold)))
        );
    }
}

fn print_bar_table(rows: &[GateRow], bar: Option<f64>, digits: usize) {
    note!("  bar   covered  coverage  correct  accuracy");
    for row in rows {
        let accuracy = row
            .accuracy
            .map_or_else(|| "-".to_owned(), |a| format!("{a:.digits$}"));
        note!(
            "  {:<4.2}  {:<7}  {:<8.2}  {:<7}  {}{}",
            row.bar,
            row.covered,
            row.coverage,
            row.correct,
            accuracy,
            mark(bar.is_some_and(|b| same(b, row.bar)))
        );
    }
}

fn print_level_table(rows: &[LevelRow], best: Option<usize>) {
    note!("  level  accuracy  precision  recall  f1");
    for row in rows {
        note!(
            "  {:<5}  {:<8.2}  {:<9}  {:<6}  {}{}",
            row.level,
            row.accuracy,
            cell(row.precision),
            cell(row.recall),
            cell(row.f1),
            mark(best == Some(row.level))
        );
    }
}

/// The one model that answered, `None` when nothing was answered. A bar is
/// tuned per model version, so recordings of two are refused; the refusal
/// says which cases each answered, since the odd one out is what to record
/// again. A model's name is the recording's or the server's to choose, so it
/// is shown as text (`out::plain`), never as the controls it may hold.
fn single_model(answered: &[Answered]) -> Result<Option<String>, Failure> {
    let mut by_model: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for a in answered {
        by_model
            .entry(a.response.model.as_str())
            .or_default()
            .push(a.unit.name.as_str());
    }
    if by_model.len() <= 1 {
        return Ok(by_model.keys().next().map(|m| (*m).to_owned()));
    }
    let models: Vec<String> = by_model.keys().map(|m| out::plain(m)).collect();
    let answers: Vec<String> = by_model
        .iter()
        .map(|(model, cases)| {
            let model = out::plain(model);
            let shown = cases
                .iter()
                .take(NAMES_SHOWN)
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            match cases.len().checked_sub(NAMES_SHOWN) {
                Some(more) if more > 0 => format!("{model} answered {shown} and {more} more"),
                _ => format!("{model} answered {shown}"),
            }
        })
        .collect();
    Err(Failure::Usage(format!(
        "the recordings come from more than one model ({}); a bar is tuned per model, record again with one ({})",
        models.join(", "),
        answers.join("; ")
    )))
}

/// The one server that answered the requests, found in the recordings
/// themselves: `Backend::server` is `None` under a replay, and a replay does
/// not say which recording answered. Omitted when none of them says, or when
/// they disagree, because a server written into the provenance has to be the
/// one that gave these answers. Each is written without the credentials,
/// query and fragment a URL can carry (`backend::public_url`), also when the
/// recording was written by a run that kept them: the tuning block is
/// meant to be committed.
fn recorded_server(dir: &Path, answered: &[Answered]) -> Option<String> {
    let found = recordings::scan(dir);
    let mut servers = BTreeSet::new();
    for a in answered {
        let fingerprint = canonical::request_fingerprint(&a.unit.case.state, &a.questions);
        let mut matching: Vec<_> = found
            .iter()
            .map(|f| &f.recording)
            .filter(|r| r.fingerprint.as_deref() == Some(fingerprint.as_str()))
            .collect();
        if matching.is_empty() {
            // The order the replay itself looks in: the fingerprint, then the hash.
            let hash = eval::request_hash(&a.unit.case.state, &a.questions);
            matching = found
                .iter()
                .map(|f| &f.recording)
                .filter(|r| r.request_hash.as_deref() == Some(hash.as_str()))
                .collect();
        }
        servers.extend(
            matching
                .iter()
                .filter_map(|r| r.server.as_deref().map(backend::public_url)),
        );
    }
    let mut servers = servers.into_iter();
    match (servers.next(), servers.next()) {
        (Some(server), None) => Some(server),
        _ => None,
    }
}

/// The questions the sweep tuned and how many labelled cases each was read
/// from: the `labelled` the tuning block keeps, since a gate left as written
/// was not tuned on these cases.
fn labelled(proposals: &IndexMap<String, Proposal>) -> Value {
    Value::Object(
        proposals
            .iter()
            .map(|(id, p)| (id.clone(), Value::from(p.labelled)))
            .collect(),
    )
}

/// The rubric with the proposals and the tuning applied, each gate checked
/// against its question as a parsed one is.
fn apply(
    rubric: &Rubric,
    proposals: &IndexMap<String, Proposal>,
    tuning: Tuning,
) -> Result<Rubric, Failure> {
    let mut tuned = rubric.clone();
    for (id, proposal) in proposals {
        tuned
            .gate(id.clone(), proposal.gate.clone())
            .map_err(|e| Failure::Usage(format!("the proposed gate for {id} is refused: {e}")))?;
    }
    tuned.policy.tuning = Some(tuning);
    Ok(tuned)
}

/// What a person pastes under `spec:`: the policy and the tuning of the
/// tuned rubric and nothing else. The rubric is written and read back through
/// the library, as `--out` would write it, so what is printed is what the
/// format reads; the blocks are then written from the typed values, not from
/// a generic map, so the gates keep the rubric's order.
#[derive(Serialize)]
struct Blocks<'a> {
    policy: &'a IndexMap<String, Gate>,
    tuning: &'a Tuning,
}

/// The two blocks of `yaml`, as the text that goes to stdout.
fn blocks(yaml: &str) -> Result<String, Failure> {
    let reread = Rubric::parse(yaml)
        .map_err(|e| Failure::Usage(format!("the tuned rubric does not read back: {e}")))?;
    let Some(tuning) = reread.policy.tuning.as_ref() else {
        return Err(Failure::Usage(
            "the tuned rubric lost its tuning block".to_owned(),
        ));
    };
    serde_saphyr::to_string(&Blocks {
        policy: &reread.policy.gates,
        tuning,
    })
    .map_err(|e| Failure::Usage(format!("cannot write the proposal: {e}")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_text_level_reads_back_as_the_level_or_falls_back_to_its_index() {
        let calm = [json!("calm"), json!("annoyed"), json!("angry")];
        assert_eq!(
            level_like(&LevelRef::Text("angry".into()), 1, &calm),
            LevelRef::Text("annoyed".into())
        );
        assert_eq!(
            level_like(&LevelRef::Index(2), 1, &calm),
            LevelRef::Index(1)
        );
        // Levels whose texts are numbers: the text "3" reads as the index 3,
        // which is another level, so the index is what names this one.
        let numbers = [json!(1), json!(2), json!(3)];
        assert_eq!(
            level_like(&LevelRef::Text("3".into()), 1, &numbers),
            LevelRef::Text("1".into()),
            "the level whose text is 2 is the index 1"
        );
        // The same text on two levels names only the first.
        let twins = [json!("low"), json!("high"), json!("high")];
        assert_eq!(
            level_like(&LevelRef::Text("high".into()), 2, &twins),
            LevelRef::Text("2".into())
        );
    }

    #[test]
    fn the_strict_sentence_counts_and_says_what_happens_to_them() {
        let noul = Question::Noul {
            instructions: Value::Null,
            criteria: None,
        };
        let choice = Question::Choice {
            instructions: Value::Null,
            criteria: IndexMap::new(),
        };
        assert_eq!(
            strict_at_bar(&choice, 2, 0.45),
            "2 answers sit exactly at the proposed bar 0.45 and a strict gate defers them"
        );
        assert_eq!(
            strict_at_bar(&choice, 1, 0.45),
            "1 answer sits exactly at the proposed bar 0.45 and a strict gate defers it"
        );
        assert_eq!(
            strict_at_bar(&noul, 1, 0.55),
            "1 answer sits exactly at the proposed bar 0.55 and a strict gate reads it as no"
        );
        assert_eq!(
            strict_at_bar(&noul, 3, 0.55),
            "3 answers sit exactly at the proposed bar 0.55 and a strict gate reads them as no"
        );
    }

    #[test]
    fn a_bar_is_shown_with_the_digits_it_has() {
        assert_eq!(shown_bar(Some(0.3)), "0.30");
        assert_eq!(shown_bar(Some(0.0)), "0.00");
        assert_eq!(shown_bar(Some(1.0)), "1.00");
        assert_eq!(shown_bar(Some(0.333)), "0.333");
        assert_eq!(shown_bar(None), "unset");
    }
}
