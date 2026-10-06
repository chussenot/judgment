//! `jud RUBRIC`: the state on stdin, the rubric from the file, the verdicts
//! on stdout. Every step is the library's: `Rubric::parse`, `Rubric::lower`
//! with no supplied options, the client's `answer`, `Rubric::apply`.

use std::io::Read;
use std::process::ExitCode;

use judgment::SystemOne;
use judgment::jud::{Error as JudError, Rubric, Supplied};
use serde_json::Value;

use crate::{EXIT_BACKEND, EXIT_USAGE, config};

pub(crate) fn run(path: &str, rest: &[String]) -> ExitCode {
    if let Some(extra) = rest.first() {
        eprintln!(
            "jud: unexpected argument `{extra}`; the state comes from stdin, not an argument"
        );
        eprint!("{}", crate::USAGE);
        return ExitCode::from(EXIT_USAGE);
    }
    match evaluate(path) {
        Ok(verdicts) => {
            println!("{verdicts}");
            ExitCode::SUCCESS
        }
        Err(Failure::Usage(message)) => {
            eprintln!("jud: {message}");
            ExitCode::from(EXIT_USAGE)
        }
        Err(Failure::Backend(message)) => {
            eprintln!("jud: {message}");
            ExitCode::from(EXIT_BACKEND)
        }
    }
}

enum Failure {
    /// Fixable before any call: the file, the state, the configuration.
    Usage(String),
    /// The call was made and failed, or its answer did not fit.
    Backend(String),
}

fn read_rubric(path: &str) -> Result<Rubric, Failure> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Failure::Usage(format!("cannot read rubric {path}: {e}")))?;
    Rubric::parse(&text).map_err(|e| {
        let hint = match &e {
            JudError::Kind { .. } => " (jud evaluates a Rubric)".to_owned(),
            JudError::Missing { .. } | JudError::Version { .. } => {
                " (not a jud/v1.3 document; docs/jud.md has the envelope)".to_owned()
            }
            _ => String::new(),
        };
        Failure::Usage(format!("{path} is not a valid Rubric: {e}{hint}"))
    })
}

fn read_state() -> Result<Value, Failure> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| Failure::Usage(format!("cannot read stdin: {e}")))?;
    if text.trim().is_empty() {
        return Err(Failure::Usage(
            "stdin is empty: pipe the JSON state in, as `cat state.json | jud rubric.jud`"
                .to_owned(),
        ));
    }
    serde_json::from_str(&text).map_err(|e| {
        let hint = if e.is_syntax() && text.trim_start().starts_with(['{', '[', '"'])
            && e.to_string().contains("trailing")
        {
            "; stdin holds more than one JSON value, so pass one (`jq -c .`, or `jq -s .` to wrap them)"
        } else {
            "; stdin must be one JSON value (`yq -o=json` turns YAML into one)"
        };
        Failure::Usage(format!("stdin is not JSON: {e}{hint}"))
    })
}

fn evaluate(path: &str) -> Result<String, Failure> {
    let rubric = read_rubric(path)?;
    let state = read_state()?;
    let resolved = config::resolve().map_err(|e| Failure::Usage(e.to_string()))?;
    let client = resolved
        .client()
        .map_err(|e| Failure::Usage(e.to_string()))?;
    let questions = rubric.lower(&state, &Supplied::default()).map_err(|e| {
        Failure::Usage(format!(
            "the rubric does not lower for this state: {e} (a Choice with `options_from: request` needs options jud cannot supply yet)"
        ))
    })?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Failure::Backend(format!("cannot start the runtime: {e}")))?;
    let response = runtime
        .block_on(client.answer(&state, resolved.model(), &questions))
        .map_err(|e| {
            Failure::Backend(format!(
                "the backend at {} failed: {e}",
                resolved.base_url()
            ))
        })?;
    let verdicts = rubric
        .apply(&questions, &response)
        .map_err(|e| Failure::Backend(format!("the answers do not fit the rubric: {e}")))?;
    serde_json::to_string_pretty(&verdicts)
        .map_err(|e| Failure::Backend(format!("cannot serialise the verdicts: {e}")))
}
