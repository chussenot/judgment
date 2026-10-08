//! `jud tune RUBRIC CASES --replay DIR`: propose the bars of the rubric's
//! gates from recorded answers (decision 0021). Never calls a backend and
//! never rewrites its input.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, ValueHint};

use crate::backend::Failure;

/// Propose each gate's bar from recorded answers.
///
/// Reads the recordings in DIR (it never calls a backend), grades them
/// against the cases, sweeps each gate and prints a proposal: a Noul's
/// threshold by best F1, a Choice's or Score's confidence bar as the lowest
/// that keeps --target-accuracy over at least --min-covered cases, a Score's
/// level_at_least by best F1. The tables go to stderr; stdout carries the
/// proposed policy and tuning blocks as YAML. --out writes the whole rubric
/// with the proposal applied to a new file; the input is never rewritten.
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
    /// Write the rubric with the proposal applied to this new file.
    #[arg(long, value_name = "PATH", value_hint = ValueHint::FilePath)]
    pub out: Option<PathBuf>,
}

pub(crate) fn run(_args: &Tune) -> ExitCode {
    Failure::Usage("jud tune is not implemented yet".to_owned()).report()
}
