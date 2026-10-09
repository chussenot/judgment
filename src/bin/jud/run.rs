//! `jud RUBRIC`: the state on stdin, the rubric from the file, the verdicts
//! on stdout. Every step is the library's: `Rubric::parse`, `Rubric::lower`
//! with the options `--options` or `--options-file` supply, the client's
//! `answer` (or `Replay`'s, over a directory of recordings), `Rubric::apply`.

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use judgment::jud::{Rubric, Supplied};
use serde_json::Value;

use crate::backend::{Backend, Failure};
use crate::tools;

/// Where the supplied options come from: `--options` inline, `--options-file`,
/// or neither (clap refuses both).
#[derive(Clone, Copy)]
pub(crate) struct OptionsArg<'a> {
    pub(crate) inline: Option<&'a str>,
    pub(crate) file: Option<&'a str>,
}

impl OptionsArg<'_> {
    fn read(self) -> Result<Supplied, Failure> {
        let parsed = match (self.inline, self.file) {
            (Some(json), _) => tools::parse_supplied(json, "--options"),
            (None, Some(path)) => std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read options file {path}: {e}"))
                .and_then(|text| tools::parse_supplied(&text, path)),
            (None, None) => Ok(Supplied::default()),
        };
        parsed.map_err(Failure::Usage)
    }
}

pub(crate) fn run(path: &str, replay: Option<&Path>, options: OptionsArg<'_>) -> ExitCode {
    match evaluate(path, replay, options) {
        Ok(verdicts) => match crate::out::result(&format!("{verdicts}\n")) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => failure.report(),
        },
        Err(failure) => failure.report(),
    }
}

fn read_rubric(path: &str) -> Result<Rubric, Failure> {
    tools::read_document(path, "Rubric", Rubric::parse).map_err(|message| {
        // `jud evaluate` is a mistyped subcommand that reads as a rubric path.
        Failure::Usage(format!("{message}{}", crate::not_a_subcommand_either(path)))
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

fn evaluate(path: &str, replay: Option<&Path>, options: OptionsArg<'_>) -> Result<String, Failure> {
    let rubric = read_rubric(path)?;
    // Before stdin and the backend: a malformed options argument is wrong
    // whatever the state, and needs no key to say so.
    let supplied = options.read()?;
    let state = read_state()?;
    let backend = Backend::open(replay)?;
    let questions = rubric.lower(&state, &supplied).map_err(|e| {
        // The questions that take options and were given none: the likely
        // cause whatever the builder's own words, when some were supplied
        // for one question and not another as much as when none were.
        let wants: Vec<&str> = rubric
            .questions
            .iter()
            .filter(|(id, q)| q.options_from.is_some() && !supplied.contains_key(id.as_str()))
            .map(|(id, _)| id.as_str())
            .collect();
        let hint = if wants.is_empty() {
            String::new()
        } else {
            format!(
                " ({} take{} options per request: pass them with --options or --options-file)",
                wants.join(", "),
                if wants.len() == 1 { "s" } else { "" }
            )
        };
        Failure::Usage(format!(
            "the rubric does not lower for this state: {e}{hint}"
        ))
    })?;
    let response = backend.answer(&state, &questions)?;
    let verdicts = rubric
        .apply(&questions, &response)
        .map_err(|e| Failure::Backend(format!("the answers do not fit the rubric: {e}")))?;
    serde_json::to_string_pretty(&verdicts)
        .map_err(|e| Failure::Backend(format!("cannot serialise the verdicts: {e}")))
}
