//! `jud`: the `.jud` format on the command line. `jud RUBRIC` evaluates the
//! JSON state on stdin against a Rubric document and prints the verdicts, so
//! a decision written as a file composes with `jq`, `yq`, `cat` and `curl`;
//! `check` and `lower` read documents the way the crate does; `record`,
//! `eval` and `tune` run a rubric over labelled cases, grade the answers and
//! propose the policy's bars (decision 0021); `config` shows which System One
//! backend a run would talk to; `completion` prints a shell completion
//! script. A thin adapter: parsing, lowering, the call and the policy are the
//! library's, and the command tree is clap's, so the help, the usage errors
//! and the completions come from one definition.
//!
//! Exit status, for every command: 0 on success; 1 when a backend call
//! failed, or `--replay` has no recording for a request; 2 when the
//! invocation, a file, the state or the configuration is wrong, which is
//! fixable before any call is made; 3 only for `jud eval`, when a
//! `--min-accuracy` bar was not met.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::error::{ContextKind, ContextValue, ErrorKind as ClapErrorKind};
use clap::{CommandFactory, Parser, Subcommand, ValueHint};

use crate::backend::Failure;

mod backend;
mod batch;
mod config;
mod eval;
mod fsutil;
mod out;
mod record;
mod recordings;
mod run;
mod tools;
mod tune;

type Fallible<T> = Result<T, Box<dyn std::error::Error>>;

/// The invocation, a file, the state or the configuration is wrong. Also
/// clap's status for a usage error, so one number covers both.
const EXIT_USAGE: u8 = 2;
/// A backend call failed, or a replay has no recording for a request.
const EXIT_BACKEND: u8 = 1;
/// `jud eval` ran, and a `--min-accuracy` gate was not met.
const EXIT_UNMET: u8 = 3;

const EXAMPLES: &str = "\
Examples:
  cat state.json | jud RUBRIC.jud
  jq '.customer' event.json | jud rubric.jud
  yq -o=json '.spec' resource.yaml | jud rubric.jud

  # The loop over labelled cases: ask once, then grade and tune offline.
  jud record rubric.jud cases.jud --out recordings/
  jud eval rubric.jud cases.jud --replay recordings/
  jud tune rubric.jud cases.jud --replay recordings/

Exit status: 0 success; 1 a backend call failed, or a recording is missing
under --replay; 2 wrong before any call: the invocation, a file, the state or
the configuration; 3 `jud eval` only: a --min-accuracy gate was not met.";

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
    /// A state nobody recorded is an error, never a guess. `jud eval` and
    /// `jud tune` take their own --replay after the subcommand; `jud record`
    /// never replays, it writes recordings with --out.
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
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => return usage_error(&e),
    };
    let replay = cli.replay;
    match (cli.command, cli.rubric) {
        (Some(Command::Config), _) => report(config::show(), EXIT_USAGE),
        (Some(Command::Check { files }), _) => report(Ok(tools::check(&files)), EXIT_USAGE),
        (Some(Command::Lower(args)), _) => report(tools::lower(&args), EXIT_USAGE),
        (Some(Command::Record(args)), _) => record::run(&args),
        (Some(Command::Eval(args)), _) => eval::run(&args),
        (Some(Command::Tune(args)), _) => tune::run(&args),
        (Some(Command::Completion { shell }), _) => completion(shell),
        // A subcommand's name in the RUBRIC slot got there because the flag
        // came first: with no `--replay` before it, clap runs the subcommand.
        // A file that really has that name is still read as the rubric.
        (None, Some(word))
            if subcommand_names().contains(&word) && !Path::new(&word).exists() =>
        {
            Failure::Usage(format!("{word} is a subcommand, not a rubric. {REPLAY_AFTER}"))
                .report()
        }
        (None, Some(path)) => run::run(&path, replay.as_deref()),
        // `arg_required_else_help` prints the help only for an empty command
        // line; a `--replay` or a `JUD_REPLAY` alone is an argument, so this
        // arm is reached with nothing to run and says so.
        (None, None) => Failure::Usage(format!(
            "a rubric is required: `jud RUBRIC < state.json`, or a subcommand ({}); see `jud --help`",
            subcommand_names().join(", ")
        ))
        .report(),
    }
}

/// clap's zsh script declares the optional RUBRIC before the subcommand, so
/// in `jud eval <TAB>` zsh takes `eval` for the rubric and never reaches the
/// subcommand's arguments. One slot instead, offering a subcommand or a
/// file, and the dispatch reads the subcommand from that first slot.
/// ponytail: text surgery on `clap_complete`'s output; if a `clap_complete`
/// upgrade changes these lines the replacements no-op and
/// `tests/jud_cli.rs` fails on the old layout.
fn zsh_subcommand_first(script: &str) -> String {
    script
        .lines()
        .filter(|l| !l.starts_with("'::rubric -- "))
        .map(|l| match l {
            "\":: :_jud_commands\" \\" => "\":: :{_jud_commands; _files}\" \\".to_owned(),
            _ => l
                .replace("words=($line[2] ", "words=($line[1] ")
                .replace("jud-command-$line[2]:", "jud-command-$line[1]:")
                .replace("case $line[2] in", "case $line[1] in"),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// Where a subcommand's options go, for the one mistake clap's own message
/// does not explain: `jud --replay DIR eval RUBRIC CASES`.
const REPLAY_AFTER: &str =
    "--replay belongs after the subcommand: jud eval RUBRIC CASES --replay DIR";

/// The subcommands, read from the command tree so a list in a message cannot
/// drift from the binary.
fn subcommand_names() -> Vec<String> {
    Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_owned())
        .collect()
}

/// A mistyped subcommand was read as the RUBRIC, and the file `jud` would
/// then have opened is not there: say what else the word could have been.
/// Only for a bare name (no path separator, no extension), since a path or a
/// `.jud` file that is missing is a missing file and nothing else.
pub(crate) fn not_a_subcommand_either(path: &str) -> String {
    let bare = !path.contains(['/', '\\']) && Path::new(path).extension().is_none();
    let missing = matches!(std::fs::metadata(path), Err(e) if e.kind() == ErrorKind::NotFound);
    if bare && missing {
        format!(
            " (not a subcommand either: {})",
            subcommand_names().join(", ")
        )
    } else {
        String::new()
    }
}

/// clap's own report (help and version go to stdout with status 0, a usage
/// error to stderr with status 2), then one line for the two conflicts a
/// newcomer meets: `jud` takes either a RUBRIC or a subcommand, and clap
/// words the clash by what it was given, which names neither.
fn usage_error(e: &clap::Error) -> ExitCode {
    let _ = e.print();
    if let Some(hint) = conflict_hint(e) {
        out::note!("jud: {hint}");
    }
    ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(EXIT_USAGE))
}

fn conflict_hint(e: &clap::Error) -> Option<String> {
    if e.kind() != ClapErrorKind::ArgumentConflict
        || e.get(ContextKind::InvalidSubcommand).is_none()
    {
        return None;
    }
    let prior: Vec<&str> = match e.get(ContextKind::PriorArg) {
        Some(ContextValue::String(one)) => vec![one.as_str()],
        Some(ContextValue::Strings(many)) => many.iter().map(String::as_str).collect(),
        _ => Vec::new(),
    };
    if prior.iter().any(|arg| arg.starts_with("--replay")) {
        Some(REPLAY_AFTER.to_owned())
    } else {
        Some(format!(
            "`jud RUBRIC` takes one file, and the first word is not a subcommand ({})",
            subcommand_names().join(", ")
        ))
    }
}

/// `jud completion <shell>`: the script, generated from [`Cli`] into a
/// buffer first so nothing reaches stdout until the whole script exists. A
/// reader that closes early (`jud completion bash | head -1`) is not an
/// error: the script was complete, it is the pipe that ended (`out::result`).
fn completion(shell: clap_complete::Shell) -> ExitCode {
    let mut command = Cli::command();
    let name = command.get_name().to_owned();
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut command, name, &mut script);
    let mut script = String::from_utf8_lossy(&script).into_owned();
    if shell == clap_complete::Shell::Zsh {
        script = zsh_subcommand_first(&script);
    }
    match out::result(&script) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => failure.report(),
    }
}

/// `Ok(true)` succeeds, `Ok(false)` is a refusal already printed, an error
/// goes to stderr with `code`.
fn report(result: Fallible<bool>, code: u8) -> ExitCode {
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(code),
        Err(e) => {
            out::note!("jud: {e}");
            ExitCode::from(code)
        }
    }
}
