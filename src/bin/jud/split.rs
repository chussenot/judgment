//! `jud split CASES`: hold out every Nth case of a Cases document, writing
//! the two halves as documents of their own, so a bar is tuned on one half
//! and graded on the other. Each case is copied as read, state and labels
//! and notes alike, so a recording that answers a case in the whole set
//! answers it in its half: nothing is recorded again.
//!
//! The halves are new files beside the input (or under `--out`), refused if
//! either exists; the input is never written.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, ValueHint};
use indexmap::IndexMap;
use judgment::jud::{Case, Cases, Expect};

use crate::backend::Failure;
use crate::fsutil::same_file;
use crate::{out, tools};

/// Split a Cases document into a tuning set and a held-out set.
///
/// Every Nth case (the Nth, the 2Nth, ...) goes to NAME-holdout.jud, the
/// others to NAME-tune.jud, beside CASES or in --out. Cases are copied as
/// read, so recordings made over CASES answer both halves. Prints each half's
/// count, fingerprint and labels per question, and warns about a label one
/// half has and the other lacks. Refuses to write over an existing file.
#[derive(Args)]
pub(crate) struct Split {
    /// The Cases document to split.
    #[arg(value_name = "CASES", value_hint = ValueHint::FilePath)]
    pub cases: String,
    /// Hold out every Nth case, 2 or more: 4 holds out a quarter.
    #[arg(long, value_name = "N", default_value_t = 4, value_parser = clap::value_parser!(u32).range(2..))]
    pub every: u32,
    /// The directory the two halves are written to; CASES's own by default.
    #[arg(long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    pub out: Option<PathBuf>,
}

pub(crate) fn run(args: &Split) -> ExitCode {
    match split(args) {
        Ok(text) => match out::result(&text) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => failure.report(),
        },
        Err(failure) => failure.report(),
    }
}

/// Cases with their position in the whole document.
type Numbered = Vec<(usize, Case)>;

/// One half: its file, its document, and what to call it in the report.
struct Half {
    role: &'static str,
    path: PathBuf,
    cases: Cases,
}

fn split(args: &Split) -> Result<String, Failure> {
    let whole = tools::read_document(&args.cases, "Cases", Cases::parse).map_err(Failure::Usage)?;
    let every = usize::try_from(args.every).unwrap_or(usize::MAX);
    let held = whole.cases.len() / every;
    if held == 0 {
        return Err(Failure::Usage(format!(
            "{} has {} case{}, fewer than --every {every}: nothing would be held out",
            args.cases,
            whole.cases.len(),
            if whole.cases.len() == 1 { "" } else { "s" }
        )));
    }
    let (holdout, tune): (Numbered, Numbered) = whole
        .cases
        .iter()
        .cloned()
        .enumerate()
        .partition(|(i, _)| (i + 1) % every == 0);
    let dir = args.out.clone().unwrap_or_else(|| {
        Path::new(&args.cases)
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf)
    });
    let stem = Path::new(&args.cases)
        .file_stem()
        .map_or_else(|| "cases".to_owned(), |s| s.to_string_lossy().into_owned());
    let halves = [
        half(&whole, "tune", &dir, &stem, tune, every, &args.cases),
        half(&whole, "holdout", &dir, &stem, holdout, every, &args.cases),
    ];
    for h in &halves {
        if same_file(&h.path, Path::new(&args.cases)) {
            return Err(Failure::Usage(format!(
                "refusing to write {} over the cases it splits",
                h.path.display()
            )));
        }
        if h.path.exists() {
            return Err(Failure::Usage(format!(
                "{} already exists; move or delete it, or pass --out",
                h.path.display()
            )));
        }
    }
    std::fs::create_dir_all(&dir)
        .map_err(|e| Failure::Usage(format!("cannot use {}: {e}", dir.display())))?;
    for h in &halves {
        write_new(h, &args.cases, every)?;
    }
    for warning in missing_labels(&halves) {
        out::note!("jud: warning: {warning}");
    }
    Ok(report(&halves))
}

fn half(
    whole: &Cases,
    role: &'static str,
    dir: &Path,
    stem: &str,
    cases: Numbered,
    every: usize,
    source: &str,
) -> Half {
    let positions: Vec<String> = cases
        .iter()
        .take(3)
        .map(|(i, _)| (i + 1).to_string())
        .collect();
    let description = match role {
        "holdout" => format!(
            "Held out from {} ({source}): every {every}th case, cases {}, ...; graded, never tuned on.",
            whole.name,
            positions.join(", ")
        ),
        _ => format!(
            "{} ({source}) without every {every}th case: the cases a bar is tuned on.",
            whole.name
        ),
    };
    Half {
        role,
        path: dir.join(format!("{stem}-{role}.{}", judgment::jud::EXTENSION)),
        cases: Cases {
            name: format!("{}-{role}", whole.name),
            description: Some(description),
            labels: whole.labels.clone(),
            annotations: whole.annotations.clone(),
            rubric: whole.rubric.clone(),
            cases: cases.into_iter().map(|(_, c)| c).collect(),
        },
    }
}

/// Write a half to a file that must not exist yet: `create_new` refuses one
/// that appeared since the check, so nothing is ever written over.
fn write_new(half: &Half, source: &str, every: usize) -> Result<(), Failure> {
    let yaml = half
        .cases
        .to_yaml()
        .map_err(|e| Failure::Backend(format!("cannot write {}: {e}", half.path.display())))?;
    let text = format!(
        "# The {} half of {source}, written by `jud split --every {every}`.\n{yaml}",
        half.role
    );
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&half.path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(|e| Failure::Usage(format!("cannot write {}: {e}", half.path.display())))
}

/// A label as one word: `true`, an option key, a level's index or text,
/// `from_turn 2`.
fn label(value: &Expect) -> String {
    match value {
        Expect::Bool(b) => b.to_string(),
        Expect::Index(i) => i.to_string(),
        Expect::Text(t) => t.clone(),
        Expect::FromTurn(turn) => match turn.from_turn {
            Some(n) => format!("from_turn {n}"),
            None => "from_turn never".to_owned(),
        },
    }
}

/// Per question, per label, how many cases carry it, in first-seen order.
fn label_counts(cases: &Cases) -> IndexMap<String, IndexMap<String, usize>> {
    let mut counts: IndexMap<String, IndexMap<String, usize>> = IndexMap::new();
    for case in &cases.cases {
        for (question, value) in &case.expect {
            *counts
                .entry(question.clone())
                .or_default()
                .entry(label(value))
                .or_default() += 1;
        }
    }
    counts
}

/// A label one half has and the other lacks: a held-out number on an
/// outcome the tuning set never saw, or the reverse, says little.
fn missing_labels(halves: &[Half; 2]) -> Vec<String> {
    let [tune, holdout] = halves.each_ref().map(|h| label_counts(&h.cases));
    let mut warnings = Vec::new();
    for (have, lack, name) in [(&tune, &holdout, "holdout"), (&holdout, &tune, "tune")] {
        for (question, labels) in have {
            for value in labels.keys() {
                if !lack.get(question).is_some_and(|l| l.contains_key(value)) {
                    warnings.push(format!(
                        "the {name} half has no case labelling {question} {}",
                        out::plain(value)
                    ));
                }
            }
        }
    }
    warnings
}

/// Each half: where it went, how many cases, its fingerprint, and its labels
/// per question.
fn report(halves: &[Half; 2]) -> String {
    let mut lines = Vec::new();
    for h in halves {
        lines.push(format!(
            "{} {}: name {}, {} case{}, cases {}",
            h.role,
            h.path.display(),
            h.cases.name,
            h.cases.cases.len(),
            if h.cases.cases.len() == 1 { "" } else { "s" },
            h.cases.fingerprint()
        ));
        for (question, labels) in label_counts(&h.cases) {
            let counts: Vec<String> = labels
                .iter()
                .map(|(value, n)| format!("{} {n}", out::plain(value)))
                .collect();
            lines.push(format!("  {question}: {}", counts.join(", ")));
        }
    }
    lines.join("\n") + "\n"
}
