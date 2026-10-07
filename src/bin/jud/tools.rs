//! `jud check` and `jud lower`: read `.jud` documents the way the crate
//! reads them, and show what a rubric lowers to. For a person or an agent
//! writing a document by hand: `check` refuses what the reader refuses, with
//! the field named, binds cases to their rubric and verifies a recording
//! against the request it answers; `lower` prints the request one state
//! produces, so a `when` or a `part_when` can be seen rather than guessed.

use clap::ValueHint;
use judgment::Questions;
use judgment::eval::Recording;
use judgment::eval::canonical;
use judgment::jud::{self, Case, Cases, Document, Rubric, Supplied};
use serde_json::Value;

use crate::Fallible;

/// One document read from a file, with the version it declared.
struct Loaded {
    path: String,
    declared: String,
    document: Document,
}

fn declared_version(text: &str) -> String {
    serde_saphyr::from_str::<Value>(text)
        .ok()
        .and_then(|v| v.get("apiVersion").cloned())
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "?".to_owned())
}

/// What to add to a reader's refusal for someone who handed `jud` a file
/// that is not a `.jud` document at all: the reader names the missing field,
/// the hint says where the envelope is described.
pub(crate) fn hint(e: &jud::Error) -> &'static str {
    match e {
        jud::Error::Missing {
            field: "apiVersion" | "kind",
        }
        | jud::Error::Version { .. } => {
            " (not a jud/v1.3 document; docs/reference/jud-format.md has the envelope)"
        }
        _ => "",
    }
}

/// Read and parse one document for a subcommand, every failure naming the
/// path and what was expected of it: `cannot read Rubric PATH: ...` when the
/// file cannot be read, `PATH is not a valid Rubric: ...` when the reader
/// refuses it, with [`hint`] appended.
pub(crate) fn read_document<T>(
    path: &str,
    what: &'static str,
    parse: fn(&str) -> jud::Result<T>,
) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {} {path}: {e}", what.to_ascii_lowercase()))?;
    parse(&text).map_err(|e| format!("{path} is not a valid {what}: {e}{}", hint(&e)))
}

fn load(paths: &[String]) -> (Vec<Loaded>, usize) {
    let mut loaded = Vec::new();
    let mut errors = 0;
    for path in paths {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                println!("error     {path}: {e}");
                errors += 1;
                continue;
            }
        };
        match jud::parse(&text) {
            Ok(document) => loaded.push(Loaded {
                path: path.clone(),
                declared: declared_version(&text),
                document,
            }),
            Err(e) => {
                println!("error     {path}: {e}{}", hint(&e));
                errors += 1;
            }
        }
    }
    (loaded, errors)
}

/// True when every document was read; a refusal is printed as it happens.
pub(crate) fn check(paths: &[String]) -> bool {
    let (loaded, mut errors) = load(paths);
    let rubrics: Vec<(&str, &Rubric)> = loaded
        .iter()
        .filter_map(|l| match &l.document {
            Document::Rubric(r) => Some((l.path.as_str(), r)),
            _ => None,
        })
        .collect();
    let mut bound: Vec<(&Cases, &Rubric)> = Vec::new();
    for l in &loaded {
        match &l.document {
            Document::Rubric(rubric) => print_rubric(l, rubric),
            Document::Cases(cases) => {
                let rubric = find_rubric(cases.rubric.as_deref(), &rubrics);
                print_cases(l, cases, rubric.map(|(p, _)| p));
                if let Some((_, rubric)) = rubric {
                    match cases.bind(rubric) {
                        Ok(()) => bound.push((cases, rubric)),
                        Err(e) => {
                            println!("error     {}: {e}", l.path);
                            errors += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    for l in &loaded {
        if let Document::Recording(recording) = &l.document {
            errors += usize::from(!print_recording(l, recording, &bound));
        }
    }
    let documents = loaded.len() + errors;
    println!("{documents} documents, {errors} refused");
    errors == 0
}

fn find_rubric<'a>(
    named: Option<&str>,
    rubrics: &[(&'a str, &'a Rubric)],
) -> Option<(&'a str, &'a Rubric)> {
    match named {
        Some(name) => rubrics
            .iter()
            .copied()
            .find(|(_, r)| r.name == name || r.fingerprint() == name),
        None if rubrics.len() == 1 => rubrics.first().copied(),
        None => None,
    }
}

fn print_rubric(l: &Loaded, rubric: &Rubric) {
    let gates = rubric.policy.gates.len();
    let tuned = if rubric.policy.tuning.is_some() {
        "tuned"
    } else {
        "untuned"
    };
    println!(
        "rubric    {}: name {}, {}, {} questions, {gates} gates ({tuned})",
        l.path,
        rubric.name,
        l.declared,
        rubric.questions.len()
    );
    println!("          questions {}", rubric.fingerprint());
    println!("          policy    {}", rubric.policy_fingerprint());
    if let Some(cases) = rubric
        .policy
        .tuning
        .as_ref()
        .and_then(|t| t.cases.as_deref())
    {
        println!("          tuned on  {cases}");
    }
}

fn print_cases(l: &Loaded, cases: &Cases, rubric_path: Option<&str>) {
    let bound_to = match (rubric_path, cases.rubric.as_deref()) {
        (Some(path), _) => format!("bound to {path}"),
        (None, Some(name)) => format!("rubric `{name}` not among the files, labels unchecked"),
        (None, None) => "no rubric given, labels unchecked".to_owned(),
    };
    println!(
        "cases     {}: name {}, {}, {} cases, {bound_to}",
        l.path,
        cases.name,
        l.declared,
        cases.cases.len()
    );
    println!("          cases     {}", cases.fingerprint());
}

/// The state and the request a recording's `case` names among the bound
/// cases: a case by its name, or one turn of a conversation case as
/// `<case>-turn-<n>` (the names `examples/jud_calibration.rs` records under).
fn find_request(bound: &[(&Cases, &Rubric)], name: &str) -> Option<Fallible<(Value, Questions)>> {
    for (cases, rubric) in bound {
        for (i, case) in cases.cases.iter().enumerate() {
            let case_name = case.name(i);
            if case_name == name {
                return Some(request_of(case, rubric));
            }
            for turn in case.per_turn() {
                if format!("{case_name}-turn-{}", turn.index) == name {
                    return Some(request_of(&turn.case, rubric));
                }
            }
        }
    }
    None
}

fn request_of(case: &Case, rubric: &Rubric) -> Fallible<(Value, Questions)> {
    Ok((case.state.clone(), case.request(rubric)?))
}

/// Print one recording; false when it fails against the request it answers.
fn print_recording(l: &Loaded, recording: &Recording, bound: &[(&Cases, &Rubric)]) -> bool {
    let mut line = format!(
        "recording {}: case {}, {}, model {}",
        l.path, recording.case, l.declared, recording.response.model
    );
    let Some(found) = find_request(bound, &recording.case) else {
        println!("{line}, no bound case of that name, answers unchecked");
        return true;
    };
    let (state, questions) = match found {
        Ok(found) => found,
        Err(e) => {
            println!("{line}");
            println!("error     {}: the case's request: {e}", l.path);
            return false;
        }
    };
    if let Err(e) = recording.response.verify(&questions) {
        println!("{line}");
        println!("error     {}: against the request: {e}", l.path);
        return false;
    }
    let expected = canonical::request_fingerprint(&state, &questions);
    let verdict = match recording.fingerprint.as_deref() {
        Some(f) if f == expected => "fingerprint matches".to_owned(),
        Some(_) => format!("fingerprint differs (expected {expected})"),
        None => format!("no fingerprint (would be {expected})"),
    };
    line.push_str(", verified, ");
    line.push_str(&verdict);
    println!("{line}");
    true
}

/// `jud lower RUBRIC`: the state is a literal, a file, or every case of a
/// Cases document; with none, the empty object, which shows what a rubric
/// asks before any `when` holds.
#[derive(clap::Args)]
pub(crate) struct Lower {
    /// The Rubric document to lower.
    #[arg(value_name = "RUBRIC", value_hint = ValueHint::FilePath)]
    rubric: String,
    /// The JSON state, inline.
    #[arg(long, value_name = "JSON", conflicts_with_all = ["state_file", "cases"])]
    state: Option<String>,
    /// The JSON state, read from a file.
    #[arg(long, value_name = "PATH", conflicts_with = "cases", value_hint = ValueHint::FilePath)]
    state_file: Option<String>,
    /// Supplied options for `options_from: request` questions, as a JSON
    /// object of question key to option key to text.
    #[arg(long, value_name = "JSON", conflicts_with = "cases")]
    options: Option<String>,
    /// A Cases document: lower every case it holds against the rubric.
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    cases: Option<String>,
}

pub(crate) fn lower(args: &Lower) -> Fallible<bool> {
    let rubric = read_document(&args.rubric, "Rubric", Rubric::parse)?;
    let state: Option<Value> = match (&args.state, &args.state_file) {
        (Some(json), _) => {
            Some(serde_json::from_str(json).map_err(|e| format!("--state is not JSON: {e}"))?)
        }
        (None, Some(path)) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read state file {path}: {e}"))?;
            Some(serde_json::from_str(&text).map_err(|e| format!("{path} is not JSON: {e}"))?)
        }
        (None, None) => None,
    };
    let options: Supplied = match &args.options {
        Some(json) => serde_json::from_str(json).map_err(|e| {
            format!("--options is not a JSON object of question key to option key to text: {e}")
        })?,
        None => Supplied::default(),
    };
    let cases: Option<Cases> = match &args.cases {
        Some(path) => Some(read_document(path, "Cases", Cases::parse)?),
        None => None,
    };
    match (cases, state) {
        (Some(cases), _) => {
            for (i, case) in cases.cases.iter().enumerate() {
                let questions = case.request(&rubric)?;
                println!("# {}", case.name(i));
                println!("{}", serde_json::to_string_pretty(&questions)?);
            }
        }
        (None, Some(state)) => {
            let questions = rubric.lower(&state, &options)?;
            println!("{}", serde_json::to_string_pretty(&questions)?);
        }
        (None, None) => {
            let questions = rubric.lower(&Value::Object(serde_json::Map::new()), &options)?;
            println!("{}", serde_json::to_string_pretty(&questions)?);
        }
    }
    Ok(true)
}
