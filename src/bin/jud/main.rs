//! `jud`: the `.jud` format on the command line. `jud RUBRIC` evaluates the
//! JSON state on stdin against a Rubric document and prints the verdicts, so
//! a decision written as a file composes with `jq`, `yq`, `cat` and `curl`;
//! `check` and `lower` read documents the way the crate does; `config` shows
//! which System One backend a run would talk to; `completion` prints a shell
//! completion script. A thin adapter: parsing, lowering, the call and the
//! policy are the library's, and the command tree is clap's, so the help,
//! the usage errors and the completions come from one definition.
//!
//! Exit status: 0 on verdicts; 1 when the backend call failed; 2 when the
//! invocation, a file, the state or the configuration is wrong, which is
//! fixable before any call is made; 3 when `jud eval` ran and a
//! `--min-accuracy` gate was not met.

use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand, ValueHint};

mod backend;
mod batch;
mod config;
mod eval;
mod record;
mod run;
mod tools;
mod tune;

type Fallible<T> = Result<T, Box<dyn std::error::Error>>;

/// The invocation, a file, the state or the configuration is wrong. Also
/// clap's status for a usage error, so one number covers both.
const EXIT_USAGE: u8 = 2;
/// The backend was asked and the call failed.
const EXIT_BACKEND: u8 = 1;
/// `jud eval` ran, and a `--min-accuracy` gate was not met.
const EXIT_UNMET: u8 = 3;

const EXAMPLES: &str = "\
Examples:
  cat state.json | jud RUBRIC.jud
  jq '.customer' event.json | jud rubric.jud
  yq -o=json '.spec' resource.yaml | jud rubric.jud

Exit status: 0 verdicts printed; 1 the backend call failed; 2 the
invocation, a file, the state or the configuration is wrong; 3 `jud eval`
ran and a --min-accuracy gate was not met.";

/// Evaluate JSON input against a .jud Rubric and print the verdicts.
///
/// The rubric file holds the questions and the policy (kind: Rubric); stdin
/// holds the JSON state the questions are asked about. The configured System
/// One backend answers (TypeSafe by default), the policy reads the answers,
/// and one verdict per question is printed as JSON on stdout.
#[derive(Parser)]
#[command(
    name = "jud",
    version,
    after_help = EXAMPLES,
    arg_required_else_help = true,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
struct Cli {
    /// The Rubric document to evaluate; the state comes on stdin.
    #[arg(value_name = "RUBRIC", value_hint = ValueHint::FilePath)]
    rubric: Option<String>,
    /// Answer from the recordings in this directory instead of a server.
    ///
    /// The crate's Replay backend: a recording whose request fingerprint
    /// matches this state and rubric answers, verified against the
    /// questions as a server's response would be; no key, no network.
    /// A state nobody recorded is an error, never a guess.
    #[arg(long, env = "JUD_REPLAY", value_name = "DIR", value_hint = ValueHint::DirPath)]
    replay: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// The backend a run would use: base URL, model, timeout, whether an API
    /// key is set.
    ///
    /// Resolved from TYPESAFE_API_KEY and TYPESAFE_BASE_URL, then
    /// ~/.config/jud/config.yaml, then the defaults.
    Config,
    /// Read documents as the crate reads them.
    ///
    /// Cases are bound to their rubric among the files, recordings are
    /// verified against the request they answer; status 2 when any document
    /// is refused.
    Check {
        /// The .jud documents to read.
        #[arg(value_name = "FILE", required = true, value_hint = ValueHint::FilePath)]
        files: Vec<String>,
    },
    /// Print the request a rubric lowers to, for a state or for every case.
    Lower(tools::Lower),
    Record(record::Record),
    Eval(eval::Eval),
    Tune(tune::Tune),
    /// Print a shell completion script for jud's commands and flags.
    ///
    /// Generated from the same command tree clap parses, so it cannot drift
    /// from the binary. Writes to stdout; docs/start/install.md says where each shell
    /// wants it.
    Completion {
        /// The shell whose syntax to emit.
        shell: clap_complete::Shell,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let replay = cli.replay;
    match (cli.command, cli.rubric) {
        (Some(Command::Config), _) => report(config::show(), EXIT_USAGE),
        (Some(Command::Check { files }), _) => report(Ok(tools::check(&files)), EXIT_USAGE),
        (Some(Command::Lower(args)), _) => report(tools::lower(&args), EXIT_USAGE),
        (Some(Command::Record(args)), _) => record::run(&args),
        (Some(Command::Eval(args)), _) => eval::run(&args),
        (Some(Command::Tune(args)), _) => tune::run(&args),
        (Some(Command::Completion { shell }), _) => report(completion(shell), EXIT_USAGE),
        (None, Some(path)) => run::run(&path, replay.as_deref()),
        // `arg_required_else_help` has already printed the help and exited.
        (None, None) => ExitCode::from(EXIT_USAGE),
    }
}

/// `jud completion <shell>`: the script, generated from [`Cli`] into a
/// buffer first so nothing reaches stdout until the whole script exists. A
/// reader that closes early (`jud completion bash | head -1`) is not an
/// error: the script was complete, it is the pipe that ended.
fn completion(shell: clap_complete::Shell) -> Fallible<bool> {
    let mut command = Cli::command();
    let name = command.get_name().to_owned();
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut command, name, &mut script);
    match std::io::stdout().write_all(&script) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(true),
        Err(e) => Err(e.into()),
    }
}

/// `Ok(true)` succeeds, `Ok(false)` is a refusal already printed, an error
/// goes to stderr with `code`.
fn report(result: Fallible<bool>, code: u8) -> ExitCode {
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(code),
        Err(e) => {
            eprintln!("jud: {e}");
            ExitCode::from(code)
        }
    }
}
