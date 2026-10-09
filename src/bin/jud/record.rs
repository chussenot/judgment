//! `jud record RUBRIC CASES --out DIR`: answer every case once and keep the
//! answers as `.jud` recordings (decision 0021). The only subcommand that
//! spends model calls; it never edits the rubric or the cases.
//!
//! Every refusal that can be known from the documents alone comes before the
//! first call: a case without an id (a recording is a file named after its
//! case), two requests that would share a file name, a request the rubric
//! does not lower or that asks nothing, a label a conversation's turn cannot
//! carry, a recording that would be written over the rubric or the cases, a
//! stale recording in a file this run would not replace, a missing key, an
//! output directory that cannot be made.
//! After that the cases are asked one at a time, in order, and a recording is
//! written the moment its answer is verified, so whatever happens later
//! (a failed call, a closed terminal) the answers already paid for are on
//! disk.

use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Args, ValueHint};
use judgment::eval::{Recording, canonical, now_rfc3339, request_hash};
use judgment::jud::{self, recording_to_yaml};
use judgment::{Error, Questions, Replay, SystemOne};

use crate::backend::{Backend, Failure};
use crate::batch::{self, Unit};
use crate::fsutil::same_file;
use crate::out::note;
use crate::recordings;

/// Answer every case once and write the recordings.
///
/// Each case's request is lowered from its state and its own options and
/// sent to the configured backend; the verified response is written to
/// DIR/CASE.jud with the request fingerprint, the rubric, the server and the
/// time. A conversation labelled with `from_turn` is recorded turn by turn as
/// CASE-turn-N.jud. A request already recorded in DIR is kept and not asked
/// again, so an interrupted run resumes.
///
/// Record always asks the configured backend and never reads `JUD_REPLAY`, so
/// a run spends calls. --dry-run says how many, to whom and for how long, and
/// asks nothing.
#[derive(Args)]
pub(crate) struct Record {
    /// The Rubric document the cases are for.
    #[arg(value_name = "RUBRIC", value_hint = ValueHint::FilePath)]
    pub rubric: String,
    /// The Cases document to answer.
    #[arg(value_name = "CASES", value_hint = ValueHint::FilePath)]
    pub cases: String,
    /// The directory the recordings are written to, created if needed.
    #[arg(long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    pub out: PathBuf,
    /// Ask every case again, replacing the recordings already in DIR.
    #[arg(long)]
    pub refresh: bool,
    /// Ask nothing and write nothing: print what a run would do. The requests,
    /// how many DIR already answers, which are to be asked or replaced, the
    /// backend and model it would ask, whether a key is set, and how long the
    /// run would take at the median time of the recordings in DIR.
    #[arg(long)]
    pub dry_run: bool,
}

pub(crate) fn run(args: &Record) -> ExitCode {
    if args.dry_run {
        return match plan_only(args) {
            Ok(text) => match crate::out::result(&text) {
                Ok(()) => ExitCode::SUCCESS,
                Err(failure) => failure.report(),
            },
            Err(failure) => failure.report(),
        };
    }
    match record(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => failure.report(),
    }
}

/// One request to ask: the case (or turn), the name its recording takes and
/// the questions it lowers to.
struct Planned {
    name: String,
    unit: Unit,
    questions: Questions,
}

/// What the recordings already known hold for a request.
enum Held {
    /// A recording answers it, verified against the request.
    Answers,
    /// A recording matches the request and does not verify: it was edited by
    /// hand, or written under another reading of the format.
    Stale,
    /// Nothing answers it.
    Nothing,
}

/// How many requests were recorded and how many were already there.
#[derive(Default, Clone, Copy)]
struct Tally {
    recorded: usize,
    kept: usize,
}

/// One run: where it writes, whom it asks, what is already recorded, and how
/// far it has got.
struct Session<'a> {
    out: &'a Path,
    rubric: &'a str,
    backend: Backend,
    /// The recordings a request can be matched to: those found in `out` at
    /// the start (none under `--refresh`) and each one written since.
    known: Replay,
    total: usize,
    tally: Tally,
}

/// `--dry-run`: the run `record` would make, as text for the person about
/// to pay for it. Everything is read as a run reads it (the documents bound,
/// every request lowered, the directory's recordings matched by
/// fingerprint), and nothing is asked, created or written; no key is
/// needed. The time is the median `elapsed_ms` of the recordings already in
/// DIR, so it is the backend's own pace, and unknown before the first.
fn plan_only(args: &Record) -> Result<String, Failure> {
    let loaded = batch::load(&args.rubric, &args.cases)?;
    let shared = batch::plan(&loaded)?;
    let units: Vec<Unit> = shared.iter().map(|p| p.unit.clone()).collect();
    let names = recording_names(&units, &args.cases)?;
    let planned: Vec<Planned> = names
        .into_iter()
        .zip(shared)
        .map(|(name, p)| Planned {
            name,
            unit: p.unit,
            questions: p.questions,
        })
        .collect();
    refuse_inputs(args, &planned)?;
    let resolved = crate::config::resolve().map_err(|e| Failure::Usage(e.to_string()))?;
    // The run's `create_dir_all` fails on a path that is a file: so does
    // the plan, with the run's words.
    if args.out.exists() && !args.out.is_dir() {
        return Err(Failure::Usage(format!(
            "cannot use {} for the recordings: it is not a directory",
            args.out.display()
        )));
    }
    let present = args.out.is_dir();
    let known = if args.refresh || !present {
        Replay::default()
    } else {
        Replay::open(&args.out).map_err(|e| {
            Failure::Usage(format!(
                "cannot read the recordings already in {}: {e}",
                args.out.display()
            ))
        })?
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(|e| Failure::Backend(format!("cannot start the runtime: {e}")))?;
    let (mut kept, mut to_ask, mut stale) = (0_usize, Vec::new(), Vec::new());
    let mut stale_steps: Vec<&Planned> = Vec::new();
    // Two cases that lower to one request are asked once, and the second is
    // kept from the first's recording (`Session::record_one`): count it so.
    let mut asked: Vec<String> = Vec::new();
    for step in &planned {
        let fingerprint = canonical::request_fingerprint(&step.unit.case.state, &step.questions);
        if asked.contains(&fingerprint) {
            kept += 1;
            continue;
        }
        let held = runtime.block_on(known.answer(
            &step.unit.case.state,
            resolved.model(),
            &step.questions,
        ));
        match held {
            Ok(_) => kept += 1,
            Err(Error::NoRecording(_)) => {
                to_ask.push(step.name.as_str());
                asked.push(fingerprint);
            }
            Err(_) => {
                stale.push(step.name.as_str());
                stale_steps.push(step);
                asked.push(fingerprint);
            }
        }
    }
    if present {
        refuse_stale_files(&args.out, &stale_steps)?;
    }
    let times: Vec<u64> = if present {
        recordings::scan(&args.out)
            .iter()
            .map(|f| f.recording.elapsed_ms)
            .collect()
    } else {
        Vec::new()
    };
    Ok(describe_plan(&PlanSummary {
        out: &args.out,
        refresh: args.refresh,
        total: planned.len(),
        kept,
        to_ask: &to_ask,
        stale: &stale,
        base_url: resolved.base_url(),
        model: resolved.model(),
        model_from: &resolved.model_from(),
        key: resolved.api_key_source(),
        times: &times,
    }))
}

/// What `--dry-run` found, to be put in words.
struct PlanSummary<'a> {
    out: &'a Path,
    refresh: bool,
    total: usize,
    kept: usize,
    to_ask: &'a [&'a str],
    stale: &'a [&'a str],
    base_url: &'a str,
    model: &'a str,
    model_from: &'a str,
    key: &'a str,
    times: &'a [u64],
}

/// The plan in a few lines: the counts first, then the names, the backend,
/// the key and the time.
fn describe_plan(p: &PlanSummary<'_>) -> String {
    let asked = p.to_ask.len() + p.stale.len();
    let held = if p.refresh {
        "--refresh: none kept".to_owned()
    } else {
        format!("{} already recorded in {}", p.kept, p.out.display())
    };
    let mut lines = vec![format!(
        "{} request{}: {held}, {asked} to ask ({} replacing a stale recording)",
        p.total,
        if p.total == 1 { "" } else { "s" },
        p.stale.len()
    )];
    if !p.to_ask.is_empty() {
        lines.push(format!("to ask: {}", p.to_ask.join(", ")));
    }
    if !p.stale.is_empty() {
        lines.push(format!("stale, to replace: {}", p.stale.join(", ")));
    }
    lines.push(format!(
        "backend {}, model {} (from {})",
        crate::out::plain(p.base_url),
        crate::out::plain(p.model),
        p.model_from
    ));
    lines.push(match p.key {
        "missing" => "API key: missing; the run would stop before the first call (a local server that ignores it takes any word)".to_owned(),
        source => format!("API key: set ({source})"),
    });
    lines.push(match median(p.times) {
        Some(ms) if asked > 0 => format!(
            "time: about {} a request (median of {} recording{} in {}), about {} for {asked}",
            duration(ms),
            p.times.len(),
            if p.times.len() == 1 { "" } else { "s" },
            p.out.display(),
            duration(ms.saturating_mul(u64::try_from(asked).unwrap_or(u64::MAX)))
        ),
        None if asked > 0 => "time: unknown until the first request answers (no recording in the directory to read it from)".to_owned(),
        _ => "time: nothing to ask".to_owned(),
    });
    lines.join("\n") + "\n"
}

/// The refusal behind [`Session::refuse_stale_elsewhere`], for the requests
/// whose recording is stale: shared with `--dry-run`, so the plan refuses
/// what the run would refuse.
fn refuse_stale_files(out: &Path, held_stale: &[&Planned]) -> Result<(), Failure> {
    let mut found: Option<Vec<recordings::Found>> = None;
    for step in held_stale {
        let state = &step.unit.case.state;
        let fingerprint = canonical::request_fingerprint(state, &step.questions);
        let hash = request_hash(state, &step.questions);
        let own = recording_path(out, &step.name);
        let files = found.get_or_insert_with(|| recordings::scan(out));
        for file in files.iter().filter(|f| {
            f.path != own
                && (f.recording.fingerprint.as_deref() == Some(fingerprint.as_str())
                    || f.recording.request_hash.as_deref() == Some(hash.as_str()))
        }) {
            // The file's own reason: a valid twin of a stale recording in
            // the case's own file is no problem (the run reports it).
            if let Err(reason) = file.recording.response.verify(&step.questions) {
                return Err(Failure::Usage(format!(
                    "{} records this request and no longer fits the questions ({reason}); delete or move it, then record again",
                    file.path.display()
                )));
            }
        }
    }
    Ok(())
}

/// The middle value, the lower one of two; `None` for none.
fn median(times: &[u64]) -> Option<u64> {
    let mut sorted = times.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len().checked_sub(1)? / 2).copied()
}

/// Milliseconds for a person: `850 ms`, `9.1 s`, `14 min`, `2 h 5 min`.
/// Each unit is chosen after rounding, so 59.96 s reads `1 min`, never
/// `60.0 s`, and an hour less a second `1 h 0 min`, never `60 min`.
fn duration(ms: u64) -> String {
    if ms < 1_000 {
        return format!("{ms} ms");
    }
    let tenths = (ms + 50) / 100;
    if tenths < 600 {
        return format!("{}.{} s", tenths / 10, tenths % 10);
    }
    let minutes = (ms + 30_000) / 60_000;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    format!("{} h {} min", minutes / 60, minutes % 60)
}

fn record(args: &Record) -> Result<(), Failure> {
    let loaded = batch::load(&args.rubric, &args.cases)?;
    // Every request is lowered, and every label of a conversation's turn
    // checked, before the first call (`batch::plan`): a request that does not
    // lower must be found before case 1 is paid for, not after case 4.
    let shared = batch::plan(&loaded)?;
    let units: Vec<Unit> = shared.iter().map(|p| p.unit.clone()).collect();
    let names = recording_names(&units, &args.cases)?;
    let planned: Vec<Planned> = names
        .into_iter()
        .zip(shared)
        .map(|(name, p)| Planned {
            name,
            unit: p.unit,
            questions: p.questions,
        })
        .collect();
    // Before `Backend::open`, so that no key is needed to find that a case
    // would be recorded over one of the documents it was given.
    refuse_inputs(args, &planned)?;
    let backend = Backend::open(None)?;
    backend.announce(planned.len());
    let known = prepare_directory(&args.out, args.refresh)?;
    let mut session = Session {
        out: &args.out,
        rubric: &loaded.rubric.name,
        backend,
        known,
        total: planned.len(),
        tally: Tally::default(),
    };
    session.refuse_stale_elsewhere(&planned)?;
    for (position, step) in planned.iter().enumerate() {
        session.record_one(position, step)?;
    }
    note!(
        "recorded {}, kept {} in {}",
        session.tally.recorded,
        session.tally.kept,
        args.out.display()
    );
    // Whatever the run wrote or kept, two files left in the directory for one
    // request (a case renamed and asked again with `--refresh`, say) are
    // worth a line: a replay reads one of them.
    recordings::warn_duplicates(&args.out);
    Ok(())
}

impl Session<'_> {
    /// Keep the recording that already answers this request, or ask and
    /// write one. Cases are asked one at a time, in order, and the first
    /// failure stops the run: the next call would fail the same way and cost
    /// the same, and what was written stays.
    fn record_one(&mut self, position: usize, planned: &Planned) -> Result<(), Failure> {
        let name = planned.name.as_str();
        // Two cases that lower to the identical request share one
        // fingerprint. The first one's recording, added to `known` once
        // written, answers the second, which is kept: asking again would pay
        // for the same answer twice.
        let held = self.held(planned);
        if matches!(held, Held::Answers) {
            note!("kept {name}");
            self.tally.kept += 1;
            return Ok(());
        }
        let elapsed_ms = self
            .ask_and_write(planned)
            .map_err(|failure| self.stopped(position, name, &failure))?;
        if matches!(held, Held::Stale) {
            note!("replaced {name} (stale)");
        } else {
            note!(
                "recorded {name} ({}/{}, {elapsed_ms} ms)",
                position + 1,
                self.total
            );
        }
        Ok(())
    }

    /// What `known` holds for this request. A [`Replay`] answers a request
    /// only from a recording whose fingerprint matches it and whose response
    /// verifies against it, so `Ok` means "already recorded". Only
    /// [`Error::NoRecording`] means nothing is there; any other error is a
    /// recording that matches the request and cannot be used, which is asked
    /// again and replaced.
    ///
    /// Where the stale recording sits decides what happens to it:
    /// [`Session::refuse_stale_elsewhere`] has already refused a run in which
    /// it sits in a file this run would not write, so a `Stale` here is the
    /// recording in the case's own file, which is replaced.
    fn held(&self, planned: &Planned) -> Held {
        let answered = self.backend.block_on(self.known.answer(
            &planned.unit.case.state,
            self.backend.model(),
            &planned.questions,
        ));
        match answered {
            Ok(_) => Held::Answers,
            Err(Error::NoRecording(_)) => Held::Nothing,
            Err(_) => Held::Stale,
        }
    }

    /// Refuse, before any call, a request whose stale recording sits in a file
    /// that is not the one this run would write. The file is not ours to
    /// remove, and writing the new recording beside it would leave two files
    /// for one request, of which a replay reads the one that comes last by
    /// name, which may well be the stale one. The stale recording is found
    /// by the fingerprint (or the hash a harness keyed it by) the request
    /// has; a stale recording in `NAME.jud` itself is the file written
    /// anyway. Nothing is held under `--refresh`, so nothing is refused.
    fn refuse_stale_elsewhere(&self, planned: &[Planned]) -> Result<(), Failure> {
        let stale: Vec<&Planned> = planned
            .iter()
            .filter(|step| matches!(self.held(step), Held::Stale))
            .collect();
        refuse_stale_files(self.out, &stale)
    }

    /// Ask the backend, record the answer and write it; the call's time in
    /// milliseconds.
    fn ask_and_write(&mut self, planned: &Planned) -> Result<u64, Failure> {
        let state = &planned.unit.case.state;
        let started = Instant::now();
        let response = self.backend.answer(state, &planned.questions)?;
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

        let mut recording = Recording::new(planned.name.clone(), response, elapsed_ms);
        recording.fingerprint = Some(canonical::request_fingerprint(state, &planned.questions));
        recording.rubric = Some(self.rubric.to_owned());
        recording.server = self.backend.server();
        recording.recorded_at = Some(now_rfc3339());
        let text = render(&recording, &planned.questions)?;
        write_atomically(self.out, &planned.name, &text).map_err(|e| {
            Failure::Backend(format!(
                "cannot write {}: {e}",
                recording_path(self.out, &planned.name).display()
            ))
        })?;
        self.known.add(recording);
        self.tally.recorded += 1;
        Ok(elapsed_ms)
    }

    /// The failure that ends the run: which case it stopped at, and how much
    /// of the run was done and is on disk.
    fn stopped(&self, position: usize, name: &str, failure: &Failure) -> Failure {
        Failure::Backend(format!(
            "stopped at case {name} ({}/{}), {} recorded and {} kept before it, in {}: {}",
            position + 1,
            self.total,
            self.tally.recorded,
            self.tally.kept,
            self.out.display(),
            failure.message()
        ))
    }
}

/// Refuse, before any call, a recording that would take the place of the
/// rubric or the cases: nothing is ever written over an input (decision
/// 0021). `--refresh` does not read the directory, so a case named like an
/// input in `--out` would replace it without a word; [`same_file`] also sees
/// `./`, a symlinked directory and a hard link as the input.
fn refuse_inputs(args: &Record, planned: &[Planned]) -> Result<(), Failure> {
    for step in planned {
        let target = recording_path(&args.out, &step.name);
        for (kind, input) in [("rubric", &args.rubric), ("cases", &args.cases)] {
            if same_file(&target, Path::new(input)) {
                return Err(Failure::Usage(format!(
                    "refusing to write {} over the {kind} {input}; give the case another id, or record into another directory",
                    target.display()
                )));
            }
        }
    }
    Ok(())
}

/// The name each unit's recording takes, or a refusal, before any call: a
/// recording is a file named after its case, so a case without an id has
/// none (`#3` is a position, not a name), and two requests under one name
/// would overwrite each other.
fn recording_names(units: &[Unit], cases_path: &str) -> Result<Vec<String>, Failure> {
    let mut unnamed: Vec<usize> = Vec::new();
    for unit in units.iter().filter(|u| u.recording_name().is_none()) {
        if !unnamed.contains(&unit.index) {
            unnamed.push(unit.index);
        }
    }
    if !unnamed.is_empty() {
        let list: Vec<String> = unnamed.iter().map(|i| format!("#{i}")).collect();
        return Err(Failure::Usage(format!(
            "{cases_path}: {} cannot be recorded, a recording is a file named after its case: {} (by position, counting from 0); give the case an id",
            if unnamed.len() == 1 {
                "a case without an id"
            } else {
                "cases without an id"
            },
            list.join(", ")
        )));
    }
    let mut seen: HashSet<&str> = HashSet::new();
    let mut names = Vec::with_capacity(units.len());
    for unit in units {
        let name = unit.name.as_str();
        if !seen.insert(name) {
            return Err(Failure::Usage(format!(
                "{cases_path}: two requests would be recorded as {name}.{}; a case named like a turn of a conversation (`<id>-turn-<n>`) collides with that turn, rename one of them",
                jud::EXTENSION
            )));
        }
        names.push(name.to_owned());
    }
    Ok(names)
}

/// Make `dir` and read what it already holds: the recordings a request can
/// be matched to. With `--refresh` nothing already there counts, so the run
/// starts from an empty set and asks every request, and only what this run
/// writes can answer a later request of the same run.
///
/// A directory with a file the reader refuses (a `.jud` that is not a
/// recording, one under another apiVersion) is a usage failure before any
/// call: whether its request is recorded is unknown, and guessing would
/// either spend calls the person did not expect or keep a stale answer.
fn prepare_directory(dir: &Path, refresh: bool) -> Result<Replay, Failure> {
    std::fs::create_dir_all(dir).map_err(|e| {
        Failure::Usage(format!(
            "cannot use {} for the recordings: {e}",
            dir.display()
        ))
    })?;
    if refresh {
        return Ok(Replay::default());
    }
    Replay::open(dir).map_err(|e| {
        Failure::Usage(format!(
            "cannot read the recordings already in {}: {e}; fix or move the file, or pass --refresh to ask every case again and replace any file named after a case",
            dir.display()
        ))
    })
}

/// The recording as the file holds it: verified once more against the
/// questions it answers, so nothing that does not fit its request is ever
/// on disk, then the format's YAML under the one-line comment the example
/// recordings carry.
fn render(recording: &Recording, questions: &Questions) -> Result<String, Failure> {
    recording.response.verify(questions).map_err(|e| {
        Failure::Backend(format!(
            "the answer for case {} does not fit what was asked: {e}",
            recording.case
        ))
    })?;
    let yaml = recording_to_yaml(recording).map_err(|e| {
        Failure::Backend(format!(
            "cannot write the recording of case {} as a document: {e}",
            recording.case
        ))
    })?;
    Ok(format!(
        "# Recorded answer for case `{}`; found by fingerprint.\n{yaml}",
        recording.case
    ))
}

fn recording_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.{}", jud::EXTENSION))
}

/// Write `DIR/NAME.jud` so that no reader ever sees half of it: the text goes
/// to a temporary file in the same directory (so the rename stays on one
/// file system), is flushed to disk, and only then takes the recording's
/// name. The temporary name ends in `.tmp`, never `.jud` or `.json`, so a
/// [`Replay`] opened over the directory meanwhile skips it, and carries the
/// process id so two runs over one directory do not interleave their bytes.
/// A failed write removes its temporary file.
fn write_atomically(dir: &Path, name: &str, text: &str) -> std::io::Result<()> {
    let temporary = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let written = File::create(&temporary).and_then(|mut file| {
        file.write_all(text.as_bytes())?;
        file.sync_all()
    });
    let renamed = written.and_then(|()| std::fs::rename(&temporary, recording_path(dir, name)));
    if renamed.is_err() {
        // Best effort: the error being returned is the one that matters.
        let _ = std::fs::remove_file(&temporary);
    }
    renamed
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use judgment::Response;
    use serde_json::json;

    use super::*;

    /// One Noul question, and a response that answers it as asked.
    fn asked() -> (Questions, Response) {
        let mut questions = Questions::new();
        questions
            .noul("urgent", "Is `message` urgent?", None)
            .unwrap();
        let response: Response = serde_json::from_value(json!({
            "model": "jev-1.13.0",
            "answers": { "urgent": { "type": "noul", "noul": 0.9 } }
        }))
        .unwrap();
        (questions, response)
    }

    #[test]
    fn a_duration_reads_in_the_unit_a_person_would_say() {
        assert_eq!(duration(850), "850 ms");
        assert_eq!(duration(9_049), "9.0 s");
        assert_eq!(duration(9_950), "10.0 s");
        assert_eq!(duration(59_949), "59.9 s");
        assert_eq!(duration(14 * 60_000), "14 min");
        assert_eq!(duration(2 * 3_600_000 + 5 * 60_000), "2 h 5 min");
        // Rounding never shows a unit's own ceiling.
        assert_eq!(duration(59_950), "1 min");
        assert_eq!(duration(3_599_000), "1 h 0 min");
        assert_eq!(duration(7_199_000), "2 h 0 min");
        assert_eq!(duration(3_570_000), "1 h 0 min");
        assert_eq!(duration(3_569_999), "59 min");
    }

    #[test]
    fn the_median_is_the_lower_middle_and_none_of_nothing() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[7]), Some(7));
        assert_eq!(median(&[9, 1, 5]), Some(5));
        assert_eq!(median(&[4, 1, 3, 2]), Some(2));
    }

    #[test]
    fn a_recording_is_rendered_under_the_comment_the_examples_carry() {
        let (questions, response) = asked();
        let recording = Recording::new("urgent-case", response, 12);
        // `Failure` has no `Debug`, so its message stands in for it.
        let text = render(&recording, &questions)
            .map_err(|f| f.message().to_owned())
            .unwrap();
        assert!(
            text.starts_with("# Recorded answer for case `urgent-case`; found by fingerprint.\n"),
            "{text}"
        );
        assert!(text.contains("kind: Recording"), "{text}");
    }

    #[test]
    fn a_response_that_does_not_fit_its_questions_is_never_rendered() {
        // A Choice where a Noul was asked: the backend verifies before this
        // point, and the file's writer holds it to the same rule again, so no
        // path to the disk skips it.
        let (questions, _) = asked();
        let wrong: Response = serde_json::from_value(json!({
            "model": "jev-1.13.0",
            "answers": { "urgent": { "type": "choice", "choice": "yes",
                                     "probabilities": { "yes": 1.0 }, "confidence": 1.0 } }
        }))
        .unwrap();
        let failure = render(&Recording::new("urgent-case", wrong, 1), &questions).unwrap_err();
        assert!(matches!(failure, Failure::Backend(_)));
        assert!(
            failure.message().contains("urgent-case") && failure.message().contains("does not fit"),
            "{}",
            failure.message()
        );
    }

    #[test]
    fn a_write_that_fails_leaves_no_temporary_file_behind() {
        // A directory where the recording should go: the rename onto it
        // fails, after the text was written to the temporary file.
        let dir = std::env::temp_dir().join(format!("jud-record-unit-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("taken.jud")).unwrap();
        assert!(write_atomically(&dir, "taken", "text").is_err());
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            left,
            ["taken.jud"],
            "only the directory that was in the way"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_write_that_succeeds_leaves_only_the_recording() {
        let dir = std::env::temp_dir().join(format!("jud-record-unit-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_atomically(&dir, "fresh", "text").unwrap();
        write_atomically(&dir, "fresh", "again").unwrap();
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["fresh.jud"]);
        assert_eq!(
            std::fs::read_to_string(dir.join("fresh.jud")).unwrap(),
            "again"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
