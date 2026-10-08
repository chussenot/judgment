//! `jud eval RUBRIC CASES [--replay DIR]`: answer every case, grade each
//! answer against its labels and report what the model got right and what
//! the policy does with it (decision 0021). Read-only; the one subcommand
//! that can exit with status 3.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, ValueHint};

use crate::backend::Failure;

/// Grade a model's answers against the labelled cases.
///
/// Answers every case from the configured backend, or from the recordings in
/// DIR with --replay (no key, no network), and grades each against its
/// expected answer: per question the accuracy with its 95% interval, the Brier
/// score, the calibration error, how often the rubric's gate acts rather than
/// defers, and every miss. With --min-accuracy the command exits with status
/// 3 when a question falls short, which makes it a CI check.
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
    /// every question, `desk=0.95` for one. Repeatable.
    #[arg(long, value_name = "[QUESTION=]ACCURACY", value_parser = parse_min_accuracy)]
    pub min_accuracy: Vec<MinAccuracy>,
}

/// One `--min-accuracy` gate: the bar, and the question it applies to
/// (every question when `None`).
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

pub(crate) fn run(_args: &Eval) -> ExitCode {
    Failure::Usage("jud eval is not implemented yet".to_owned()).report()
}
