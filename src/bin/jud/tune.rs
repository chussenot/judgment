//! `jud tune RUBRIC CASES --replay DIR`: propose the bars of the rubric's
//! gates from recorded answers (decision 0021). Never calls a backend and
//! never rewrites its input.
//!
//! The run is the loop of `examples/jud_calibration.rs` over files: answer
//! every case from the recordings, grade the answers against the labels,
//! sweep each gate, and print what the sweep says. The tables and every note
//! go to stderr; stdout is the data, the proposed `policy` and `tuning` blocks
//! a person pastes under `spec:`. A proposal is a person's to read first, so
//! nothing is written unless `--out` names a file, and never over the input.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, ValueHint};
use indexmap::IndexMap;
use judgment::eval::tuning::{
    GateRow, LevelRow, ThresholdRow, best_level, best_threshold, default_bars, default_thresholds,
    gate_table, level_sweep, lowest_bar, threshold_sweep,
};
use judgment::eval::{ECE_BINS, Judgment, QuestionMetrics, Recording, canonical};
use judgment::jud::{Gate, LevelRef, Rubric, Tuning};
use judgment::{Question, eval};
use serde::Serialize;
use serde_json::Value;

use crate::backend::{Backend, Failure};
use crate::batch::{self, Answered};

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
/// `level_at_least` by best F1. The tables go to stderr; stdout carries the
/// proposed policy and tuning blocks as YAML. --out writes the whole rubric
/// with the proposal applied to a file instead; the input is never rewritten.
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
    /// of printing the blocks; never the input.
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
    eprintln!(
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
        eprintln!("jud: nothing was proposed, so there is no policy or tuning block{file}");
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
            eprintln!("jud: wrote the tuned rubric to {}", path.display());
        }
        None => print_blocks(&yaml)?,
    }
    Ok(())
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
/// resolves to the input, through `.` or a link, is the input.
fn check_out(args: &Tune) -> Result<(), Failure> {
    let Some(out) = &args.out else {
        return Ok(());
    };
    for (what, input) in [("rubric", &args.rubric), ("cases", &args.cases)] {
        if same_file(out, Path::new(input)) {
            return Err(Failure::Usage(format!(
                "refusing to overwrite the input: --out {} is the {what} {input}",
                out.display()
            )));
        }
    }
    Ok(())
}

/// Whether two paths name one file: the same spelling, or the same file
/// once both exist and are resolved.
fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (a.canonicalize(), b.canonicalize()),
            (Ok(a), Ok(b)) if a == b
        )
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

/// Sweep every question of the rubric, in the rubric's order, and collect
/// what it proposes. A question that cannot be tuned says why on stderr.
fn propose(
    rubric: &Rubric,
    answered: &[Answered],
    settings: &Settings,
) -> IndexMap<String, Proposal> {
    let mut proposals = IndexMap::new();
    for (id, declared) in &rubric.questions {
        let Some(gate) = rubric.policy.gates.get(id) else {
            eprintln!("jud: {id}: skipped, it has no gate; add one under `policy` to tune it");
            continue;
        };
        let judgments: Vec<Judgment> = answered
            .iter()
            .flat_map(|a| &a.judgments)
            .filter(|(question, _)| question == id)
            .map(|(_, judgment)| judgment.clone())
            .collect();
        if judgments.is_empty() {
            eprintln!("jud: {id}: skipped, no case labels it");
            continue;
        }
        if let Some(proposal) = tune_question(id, &declared.question, gate, &judgments, settings) {
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
    eprintln!(
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
    if proposed.is_some() && gate.strict {
        eprintln!(
            "jud: {id}: the gate is strict (acts above the bar) and the sweep counts an answer at a bar as acting, so an answer exactly at the proposed bar is read the other way"
        );
    }
    if metrics
        .accuracy_interval95
        .is_some_and(|(low, high)| high - low > THIN_INTERVAL)
    {
        eprintln!(
            "jud: warning: {id}: {labelled} labelled cases, {reading}; a bar read off so few cases is a guess with a number on it"
        );
    }
    proposed.map(|gate| Proposal { gate, labelled })
}

/// A Noul's `threshold`: the best F1 over the sweep, the lowest on a tie.
fn tune_threshold(id: &str, gate: &Gate, judgments: &[Judgment], labelled: usize) -> Option<Gate> {
    let rows = threshold_sweep(judgments, &default_thresholds());
    let best = best_threshold(&rows);
    print_threshold_table(&rows, best);
    let Some(threshold) = best else {
        eprintln!(
            "jud: {id}: no threshold has an F1 over {labelled} labelled cases (it needs a case labelled yes and an answer that reaches yes); the gate stays as written"
        );
        return None;
    };
    let note = format!("best F1 on {labelled} labelled cases; ties go to the lower threshold");
    eprintln!("jud: {id}: propose threshold {threshold:.2} ({note})");
    Some(Gate {
        threshold: Some(threshold),
        note: Some(note),
        ..gate.clone()
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
) -> Option<Gate> {
    let rows = gate_table(judgments, &default_bars());
    if !gate.bands.is_empty() {
        print_bar_table(&rows, None);
        eprintln!(
            "jud: {id}: the gate has bands, which jud tune prints the table for and does not propose; the gate stays as written"
        );
        return None;
    }
    let bar = lowest_bar(&rows, settings.target_accuracy, settings.min_covered);
    print_bar_table(&rows, bar);
    let mut proposed = gate.clone();
    let mut notes = Vec::new();
    let mut changed = false;
    match bar.and_then(|bar| rows.iter().find(|row| same(row.bar, bar))) {
        Some(row) => {
            let note = format!(
                "lowest bar at {} accuracy; covers {} of {} labelled cases",
                percent(settings.target_accuracy),
                row.covered,
                row.n
            );
            eprintln!("jud: {id}: propose confidence {:.2} ({note})", row.bar);
            if same(row.bar, 0.0) {
                // A bar the table starts at is no bar: say so, since a gate
                // that defers nothing is what the number now means.
                eprintln!(
                    "jud: {id}: the proposed bar is 0, so the gate would defer nothing: the labelled answers are already at least {} right with no bar",
                    percent(settings.target_accuracy)
                );
            }
            proposed.confidence = Some(row.bar);
            notes.push(note);
            changed = true;
        }
        None => eprintln!(
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
            eprintln!(
                "jud: {id}: propose level_at_least {} (level {level}; {note})",
                level_text(levels.get(level), level)
            );
            proposed.level_at_least = Some(level_like(written, level, levels));
            notes.push(note);
            changed = true;
        } else {
            eprintln!(
                "jud: {id}: no level has an F1 over {labelled} labelled cases; the level_at_least stays as written"
            );
        }
    }
    if changed {
        proposed.note = Some(notes.join("; "));
        Some(proposed)
    } else {
        None
    }
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

fn cell(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), |v| format!("{v:.2}"))
}

fn mark(chosen: bool) -> &'static str {
    if chosen { "  <- proposed" } else { "" }
}

fn print_threshold_table(rows: &[ThresholdRow], best: Option<f64>) {
    eprintln!("  threshold  accuracy  precision  recall  f1");
    for row in rows {
        eprintln!(
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

fn print_bar_table(rows: &[GateRow], bar: Option<f64>) {
    eprintln!("  bar   covered  coverage  correct  accuracy");
    for row in rows {
        eprintln!(
            "  {:<4.2}  {:<7}  {:<8.2}  {:<7}  {}{}",
            row.bar,
            row.covered,
            row.coverage,
            row.correct,
            cell(row.accuracy),
            mark(bar.is_some_and(|b| same(b, row.bar)))
        );
    }
}

fn print_level_table(rows: &[LevelRow], best: Option<usize>) {
    eprintln!("  level  accuracy  precision  recall  f1");
    for row in rows {
        eprintln!(
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
/// again.
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
    let models: Vec<&str> = by_model.keys().copied().collect();
    let answers: Vec<String> = by_model
        .iter()
        .map(|(model, cases)| {
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
/// one that gave these answers.
fn recorded_server(dir: &Path, answered: &[Answered]) -> Option<String> {
    let recordings = read_recordings(dir);
    let mut servers = BTreeSet::new();
    for a in answered {
        let fingerprint = canonical::request_fingerprint(&a.unit.case.state, &a.questions);
        let mut matching: Vec<&Recording> = recordings
            .iter()
            .filter(|r| r.fingerprint.as_deref() == Some(fingerprint.as_str()))
            .collect();
        if matching.is_empty() {
            // The order the replay itself looks in: the fingerprint, then the hash.
            let hash = eval::request_hash(&a.unit.case.state, &a.questions);
            matching = recordings
                .iter()
                .filter(|r| r.request_hash.as_deref() == Some(hash.as_str()))
                .collect();
        }
        servers.extend(matching.iter().filter_map(|r| r.server.clone()));
    }
    let mut servers = servers.into_iter();
    match (servers.next(), servers.next()) {
        (Some(server), None) => Some(server),
        _ => None,
    }
}

/// The recordings under `dir`, read the way `Replay::open` reads them: the
/// regular `.json` and `.jud` files, never a link or a directory. A file the
/// replay already read cannot fail here; one that does is left out.
fn read_recordings(dir: &Path) -> Vec<Recording> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.symlink_metadata()
                .is_ok_and(|meta| meta.file_type().is_file())
        })
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => serde_json::from_str::<Recording>(&text).ok(),
                Some(judgment::jud::EXTENSION) => judgment::jud::parse_recording(&text).ok(),
                _ => None,
            }
        })
        .collect()
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

fn print_blocks(yaml: &str) -> Result<(), Failure> {
    let reread = Rubric::parse(yaml)
        .map_err(|e| Failure::Usage(format!("the tuned rubric does not read back: {e}")))?;
    let Some(tuning) = reread.policy.tuning.as_ref() else {
        return Err(Failure::Usage(
            "the tuned rubric lost its tuning block".to_owned(),
        ));
    };
    let blocks = serde_saphyr::to_string(&Blocks {
        policy: &reread.policy.gates,
        tuning,
    })
    .map_err(|e| Failure::Usage(format!("cannot write the proposal: {e}")))?;
    // A reader that closes early is not an error: the proposal was complete,
    // it is the pipe that ended (the same rule as `jud completion`).
    match std::io::stdout().write_all(blocks.as_bytes()) {
        Err(e) if e.kind() != ErrorKind::BrokenPipe => {
            Err(Failure::Usage(format!("cannot write to stdout: {e}")))
        }
        _ => Ok(()),
    }
}
