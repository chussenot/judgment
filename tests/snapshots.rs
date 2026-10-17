//! Snapshots (insta) of what the crate sends and reports, reviewed as diffs:
//! the request body each example rubric lowers to, which is the bytes the
//! wire carries, and the report `jud eval --json --replay` gives over the
//! recordings committed under `examples/recordings/`. A change to either is
//! a contract change (CLAUDE.md), so it shows up here as a diff to read
//! rather than a failing assertion to edit. Run `cargo insta review` to
//! accept one; CI refuses a pending snapshot.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::path::Path;
use std::process::Command;

use judgment::Request;
use judgment::jud::{Cases, Rubric};
use serde_json::Value;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// `value` with every object's keys in sorted order, arrays untouched. A
/// parsed `Value` keeps its keys sorted or as written depending on whether
/// `serde_json`'s `preserve_order` is on, which feature unification flips (the
/// `mcp` feature turns it on, so `--all-features` has it and `--features cli`
/// does not); a snapshot that followed it would pass in one build and fail in
/// the other. What the model sees in order (the questions, a Choice's options)
/// is asserted where the order is the point: `tests/jud_loop.rs` and
/// `tests/jud_mcp.rs`.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// Every example rubric with the first case of the cases document bound to
/// it, as (rubric file stem, rubric, that case). The request is the one
/// `jud record` would send for the case, minus its images (a rubric's
/// `assets` are bytes in a request, not part of what is being snapshotted).
fn lowered() -> Vec<(String, Value)> {
    let dir = Path::new(ROOT).join("examples/jud");
    let mut rubrics: Vec<(String, Rubric)> = Vec::new();
    let mut cases: Vec<Cases> = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        if path.extension().and_then(|e| e.to_str()) != Some("jud") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if let Ok(rubric) = Rubric::parse(&text) {
            rubrics.push((stem, rubric));
        } else if let Ok(parsed) = Cases::parse(&text) {
            cases.push(parsed);
        }
    }
    let mut out = Vec::new();
    for (stem, rubric) in rubrics {
        // The first case of a document bound to this rubric by name; a
        // rubric nothing is labelled for is not snapshotted.
        let Some(case) = cases
            .iter()
            .find(|c| c.rubric.as_deref() == Some(rubric.name.as_str()))
            .and_then(|c| c.cases.first())
        else {
            continue;
        };
        let questions = rubric.lower(&case.state, &case.options).unwrap();
        let body = Request::new(&case.state, judgment::client::DEFAULT_MODEL, &questions);
        out.push((stem, sorted(serde_json::to_value(&body).unwrap())));
    }
    out
}

#[test]
fn each_example_rubric_lowers_to_the_same_request_body() {
    let all = lowered();
    assert!(
        all.len() >= 6,
        "the example rubrics were found: {}",
        all.len()
    );
    for (stem, body) in all {
        insta::assert_json_snapshot!(format!("request_{stem}"), body);
    }
}

/// `jud eval RUBRIC CASES --replay DIR --json`, parsed.
fn eval_report(rubric: &str, cases: &str, recordings: &str) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_jud"))
        .current_dir(ROOT)
        .args(["eval", rubric, cases, "--replay", recordings, "--json"])
        .env_remove("JUD_REPLAY")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    sorted(serde_json::from_slice(&out.stdout).unwrap())
}

#[test]
fn each_recorded_example_gives_the_same_eval_report() {
    for (name, rubric, cases, recordings) in [
        ("triage", "triage", "triage-cases", "jud_calibration"),
        ("agent_mail", "agent-mail", "agent-mail-cases", "agent_mail"),
        ("shapes", "shapes", "shapes-cases", "shapes"),
        ("stamps", "stamps", "stamps-cases", "stamps"),
    ] {
        let report = eval_report(
            &format!("examples/jud/{rubric}.jud"),
            &format!("examples/jud/{cases}.jud"),
            &format!("examples/recordings/{recordings}"),
        );
        // A recording's wall-clock fields differ between machines; none is in
        // a report today, and a future one is redacted here, not snapshotted.
        insta::assert_json_snapshot!(
            format!("eval_{name}"),
            report,
            {
                ".**.recorded_at" => "[timestamp]",
                ".**.request_id" => "[request id]",
                ".**.elapsed_ms" => "[ms]",
            }
        );
    }
}
