//! `jud`: the `.jud` format on the command line. `jud RUBRIC` evaluates the
//! JSON state on stdin against a Rubric document and prints the verdicts, so
//! a decision written as a file composes with `jq`, `yq`, `cat` and `curl`;
//! `check` and `lower` read documents the way the crate does; `config` shows
//! which System One backend a run would talk to. A thin adapter: parsing,
//! lowering, the call and the policy are the library's.
//!
//! Exit status: 0 on verdicts; 1 when the backend call failed; 2 when the
//! invocation, a file, the state or the configuration is wrong, which is
//! fixable before any call is made.

use std::process::ExitCode;

mod config;
mod run;
mod tools;

type Fallible<T> = Result<T, Box<dyn std::error::Error>>;

const USAGE: &str = "\
jud: evaluate JSON input against a .jud Rubric and print the verdicts

  cat state.json | jud RUBRIC.jud
  jq '.customer' event.json | jud rubric.jud
  yq -o=json '.spec' resource.yaml | jud rubric.jud

      The rubric file holds the questions and the policy (kind: Rubric);
      stdin holds the JSON state the questions are asked about. The
      configured System One backend answers (TypeSafe by default), the
      policy reads the answers, and one verdict per question is printed
      as JSON on stdout.

  jud config
      The backend a run would use: base URL, model, timeout, and whether an
      API key is set. Resolved from TYPESAFE_API_KEY and TYPESAFE_BASE_URL,
      then ~/.config/jud/config.yaml, then the defaults.

  jud check FILE...
      Read documents as the crate reads them: cases bound to their rubric,
      recordings verified against the request they answer; exits 2 when
      any document is refused.

  jud lower RUBRIC [--state JSON | --state-file PATH] [--options JSON]
  jud lower RUBRIC --cases FILE
      Print the request a rubric lowers to for a state, or for every case.

  jud --version

Exit status: 0 verdicts printed; 1 the backend call failed; 2 the
invocation, a file, the state or the configuration is wrong.
";

/// The invocation, a file, the state or the configuration is wrong.
const EXIT_USAGE: u8 = 2;
/// The backend was asked and the call failed.
const EXIT_BACKEND: u8 = 1;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => {
            eprint!("{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
        Some("help" | "-h" | "--help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            println!("jud {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("config") => report(config::show(), EXIT_USAGE),
        Some("check") => report(tools::check(&args[1..]), EXIT_USAGE),
        Some("lower") => report(tools::lower(&args[1..]), EXIT_USAGE),
        Some(path) => run::run(path, &args[1..]),
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
