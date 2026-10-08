//! `jud RUBRIC`: the state on stdin, the rubric from the file, the verdicts
//! on stdout. Every step is the library's: `Rubric::parse`, `Rubric::lower`
//! with no supplied options, the client's `answer` (or `Replay`'s, over a
//! directory of recordings), `Rubric::apply`.

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use judgment::jud::{Rubric, Supplied};
use serde_json::Value;

use crate::backend::{Backend, Failure};
use crate::tools;

pub(crate) fn run(path: &str, replay: Option<&Path>) -> ExitCode {
    match evaluate(path, replay) {
        Ok(verdicts) => match crate::out::result(&format!("{verdicts}\n")) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => failure.report(),
        },
        Err(failure) => failure.report(),
    }
}

fn read_rubric(path: &str) -> Result<Rubric, Failure> {
    tools::read_document(path, "Rubric", Rubric::parse).map_err(Failure::Usage)
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

fn evaluate(path: &str, replay: Option<&Path>) -> Result<String, Failure> {
    let rubric = read_rubric(path)?;
    let state = read_state()?;
    let backend = Backend::open(replay)?;
    let questions = rubric.lower(&state, &Supplied::default()).map_err(|e| {
        Failure::Usage(format!(
            "the rubric does not lower for this state: {e} (a Choice with `options_from: request` needs options jud cannot supply yet)"
        ))
    })?;
    let response = backend.answer(&state, &questions)?;
    let verdicts = rubric
        .apply(&questions, &response)
        .map_err(|e| Failure::Backend(format!("the answers do not fit the rubric: {e}")))?;
    serde_json::to_string_pretty(&verdicts)
        .map_err(|e| Failure::Backend(format!("cannot serialise the verdicts: {e}")))
}
