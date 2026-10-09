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
use judgment::Question;
use judgment::jud::{Case, Cases, Expect, Rubric};

use crate::backend::Failure;
use crate::fsutil::same_file;
use crate::{out, tools};

/// Split a Cases document into a tuning set and a held-out set.
///
/// Every Nth case (the Nth, the 2Nth, ...) goes to NAME-holdout.jud, the
/// others to NAME-tune.jud, beside CASES or in --out. Cases are copied as
/// read, so recordings made over CASES answer both halves. Prints each half's
/// count, fingerprint and labels per question, and warns about a label one
/// half has and the other lacks. With --rubric, the labels are read as the
/// rubric reads them (a Score's level by its text whether written as text or
/// index), checked against it, and listed in its order. Refuses to write over
/// an existing file.
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
    /// The Rubric the cases are for: read the labels as it reads them, so a
    /// Score level written as `0` and as `calm` is one level.
    #[arg(long, value_name = "RUBRIC", value_hint = ValueHint::FilePath)]
    pub rubric: Option<String>,
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
    let rubric = match &args.rubric {
        Some(path) => {
            let rubric =
                tools::read_document(path, "Rubric", Rubric::parse).map_err(Failure::Usage)?;
            whole
                .bind(&rubric)
                .map_err(|e| Failure::Usage(format!("{} does not fit {path}: {e}", args.cases)))?;
            Some(rubric)
        }
        None => None,
    };
    let reading = Reading(rubric.as_ref());
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
    // Both halves or neither: a holdout that cannot be written takes the
    // tune half already written with it, so a retry is not refused by it.
    write_new(&halves[0], &args.cases, every)?;
    if let Err(failure) = write_new(&halves[1], &args.cases, every) {
        let _ = std::fs::remove_file(&halves[0].path);
        return Err(failure);
    }
    for warning in missing_labels(&halves, reading) {
        out::note!("jud: warning: {warning}");
    }
    if rubric.is_none()
        && whole
            .cases
            .iter()
            .any(|c| c.expect.values().any(|v| matches!(v, Expect::Index(_))))
    {
        out::note!(
            "jud: note: a label names a level by its index; pass --rubric to read it as the level's text"
        );
    }
    Ok(report(&halves, reading))
}

/// `2nd`, `3rd`, `4th`, `11th`, `21st`: how a person says "every Nth".
fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
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
    let mut positions: Vec<String> = cases
        .iter()
        .take(3)
        .map(|(i, _)| (i + 1).to_string())
        .collect();
    if cases.len() > 3 {
        positions.push("...".to_owned());
    }
    let nth = ordinal(every);
    let description = match role {
        "holdout" => format!(
            "Held out from {} ({source}): every {nth} case, case{} {}; graded, never tuned on.",
            whole.name,
            if cases.len() == 1 { "" } else { "s" },
            positions.join(", ")
        ),
        _ => format!(
            "{} ({source}) without every {nth} case: the cases a bar is tuned on.",
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

/// How labels are read: by their own words, or, given the rubric, as the
/// outcomes it grades.
#[derive(Clone, Copy)]
struct Reading<'a>(Option<&'a Rubric>);

impl<'a> Reading<'a> {
    /// The outcomes one label stands for, as words. A conversation's
    /// `from_turn` stands for the turns it labels: `true` from turn N on,
    /// `false` before it (and on every turn for "never"), which is what an
    /// eval grades. A Score level is its text when the rubric is known,
    /// whether the label wrote the text or the index.
    fn outcomes(self, question: &str, value: &Expect) -> Vec<String> {
        match value {
            Expect::Bool(b) => vec![b.to_string()],
            Expect::FromTurn(turn) => match turn.from_turn {
                Some(0) => vec!["true".to_owned()],
                Some(_) => vec!["false".to_owned(), "true".to_owned()],
                None => vec!["false".to_owned()],
            },
            Expect::Index(i) => vec![
                self.level_text(question, usize::try_from(*i).ok())
                    .unwrap_or_else(|| i.to_string()),
            ],
            Expect::Text(t) => vec![t.clone()],
        }
    }

    fn levels(self, question: &str) -> Option<&'a [serde_json::Value]> {
        match self.0?.questions.get(question).map(|q| &q.question) {
            Some(Question::Score { criteria, .. }) => Some(criteria),
            _ => None,
        }
    }

    fn level_text(self, question: &str, index: Option<usize>) -> Option<String> {
        let level = self.levels(question)?.get(index?)?;
        Some(
            level
                .as_str()
                .map_or_else(|| level.to_string(), str::to_owned),
        )
    }

    /// The rubric's order for a question's outcomes: a Choice's keys, a
    /// Score's levels, `true` before `false`; anything else after them.
    fn order(self, question: &str) -> Vec<String> {
        match self
            .0
            .and_then(|r| r.questions.get(question))
            .map(|q| &q.question)
        {
            Some(Question::Choice { criteria, .. }) => criteria.keys().cloned().collect(),
            Some(Question::Score { criteria, .. }) => (0..criteria.len())
                .filter_map(|i| self.level_text(question, Some(i)))
                .collect(),
            Some(Question::Noul { .. }) => vec!["true".to_owned(), "false".to_owned()],
            None => Vec::new(),
        }
    }
}

/// Per question, per outcome, how many cases carry it: in the rubric's order
/// when it is known, else in first-seen order.
fn label_counts(cases: &Cases, reading: Reading<'_>) -> IndexMap<String, IndexMap<String, usize>> {
    let mut counts: IndexMap<String, IndexMap<String, usize>> = IndexMap::new();
    for case in &cases.cases {
        for (question, value) in &case.expect {
            let entry = counts.entry(question.clone()).or_insert_with(|| {
                reading
                    .order(question)
                    .into_iter()
                    .map(|outcome| (outcome, 0))
                    .collect()
            });
            for outcome in reading.outcomes(question, value) {
                *entry.entry(outcome).or_default() += 1;
            }
        }
    }
    for labels in counts.values_mut() {
        labels.retain(|_, n| *n > 0);
    }
    counts
}

/// A label one half has and the other lacks: a held-out number on an
/// outcome the tuning set never saw, or the reverse, says little.
fn missing_labels(halves: &[Half; 2], reading: Reading<'_>) -> Vec<String> {
    let [tune, holdout] = halves.each_ref().map(|h| label_counts(&h.cases, reading));
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
fn report(halves: &[Half; 2], reading: Reading<'_>) -> String {
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
        for (question, labels) in label_counts(&h.cases, reading) {
            let counts: Vec<String> = labels
                .iter()
                .map(|(value, n)| format!("{} {n}", out::plain(value)))
                .collect();
            lines.push(format!("  {question}: {}", counts.join(", ")));
        }
    }
    lines.join("\n") + "\n"
}
