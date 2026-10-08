//! `jud record RUBRIC CASES --out DIR`: answer every case once and keep the
//! answers as `.jud` recordings (decision 0021). The only subcommand that
//! spends model calls.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, ValueHint};

use crate::backend::Failure;

/// Answer every case once and write the recordings.
///
/// Each case's request is lowered from its state and its own options and
/// sent to the configured backend; the verified response is written to
/// DIR/CASE.jud with the request fingerprint, the rubric, the server and the
/// time. A conversation labelled with `from_turn` is recorded turn by turn as
/// CASE-turn-N.jud. A request already recorded in DIR is kept and not asked
/// again, so an interrupted run resumes.
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
}

pub(crate) fn run(_args: &Record) -> ExitCode {
    Failure::Usage("jud record is not implemented yet".to_owned()).report()
}
