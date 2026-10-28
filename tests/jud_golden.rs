//! Recordings made by old releases keep decoding and replaying (the golden
//! corpus under `tests/fixtures/golden/<tag>/`, copied with `git show` from
//! that tag and never re-recorded), a report from recordings equals the
//! report from the live run they were made from, and `jud record` resumes
//! from whatever a killed run left, asking exactly the missing.
//! No key and no network.

#![cfg(feature = "cli")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

mod support;

use std::path::Path;

use judgment::answer::Answer;
use judgment::backend::Replay;
use judgment::eval::{Recording, write_recording};
use serde_json::Value;
use support::triage_answers;
use support::{RECORDINGS, ROOT, TRIAGE, TRIAGE_CASES, code, jud, scratch, server, stderr, stdout};

fn golden(path: &str) -> String {
    format!("{ROOT}/tests/fixtures/golden/{path}")
}

#[test]
fn the_json_recordings_of_v0_6_0_decode_and_rewrite_byte_for_byte() {
    let out = scratch("golden");
    for (dir, expected) in [
        ("composite_scoring", 3),
        ("confidence_routing", 7),
        ("fan_out", 4),
    ] {
        let dir = golden(&format!("v0.6.0/{dir}"));
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            let recording: Recording = serde_json::from_str(&text).unwrap();
            assert!(recording.response.extra.is_empty(), "{}", path.display());
            assert!(
                !recording
                    .response
                    .answers
                    .values()
                    .any(|a| matches!(a, Answer::Unknown(_))),
                "{}",
                path.display()
            );
            let written = write_recording(&out, &recording).unwrap();
            assert_eq!(
                std::fs::read_to_string(written).unwrap(),
                text,
                "{}",
                path.display()
            );
            seen += 1;
        }
        assert_eq!(seen, expected, "{dir}");
        assert_eq!(Replay::open(Path::new(&dir)).unwrap().len(), expected);
    }
}

fn eval_json(recordings: &str) -> Value {
    let out = jud(
        &[
            "eval",
            TRIAGE,
            TRIAGE_CASES,
            "--replay",
            recordings,
            "--json",
        ],
        "",
        &[],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    serde_json::from_str(&stdout(&out)).unwrap()
}

#[test]
fn the_jud_recordings_of_v0_14_0_replay_to_the_report_the_examples_give() {
    let old = eval_json(&golden("v0.14.0/jud_calibration"));
    assert_eq!(old, eval_json(RECORDINGS));
}

fn env(uri: &str) -> [(&'static str, String); 2] {
    [
        ("TYPESAFE_API_KEY", "test-key".into()),
        ("TYPESAFE_BASE_URL", uri.into()),
    ]
}

fn run(args: &[&str], uri: &str) -> std::process::Output {
    let vars = env(uri);
    let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
    jud(args, "", &vars)
}

#[tokio::test]
async fn a_report_from_recordings_equals_the_report_from_the_live_run() {
    let live_server = server(triage_answers(), 7).await;
    let live = run(
        &["eval", TRIAGE, TRIAGE_CASES, "--json"],
        &live_server.uri(),
    );
    assert_eq!(code(&live), 0, "{}", stderr(&live));

    let origin = server(triage_answers(), 7).await;
    let dir = scratch("fidelity");
    let recorded = run(
        &[
            "record",
            TRIAGE,
            TRIAGE_CASES,
            "--out",
            dir.to_str().unwrap(),
        ],
        &origin.uri(),
    );
    assert_eq!(code(&recorded), 0, "{}", stderr(&recorded));

    let replayed = jud(
        &[
            "eval",
            TRIAGE,
            TRIAGE_CASES,
            "--replay",
            dir.to_str().unwrap(),
            "--json",
        ],
        "",
        &[],
    );
    assert_eq!(code(&replayed), 0, "{}", stderr(&replayed));
    assert_eq!(stdout(&live), stdout(&replayed));
}

#[tokio::test]
async fn a_killed_run_leaves_files_the_next_run_resumes_from_asking_exactly_the_missing() {
    let first = server(triage_answers(), 7).await;
    let dir = scratch("resume");
    let out = dir.to_str().unwrap();
    assert_eq!(
        code(&run(
            &["record", TRIAGE, TRIAGE_CASES, "--out", out],
            &first.uri()
        )),
        0
    );

    // A kill after k calls (two recordings gone) and one mid-write (a temporary
    // file with half a document, which a rename never turned into a recording).
    std::fs::remove_file(dir.join("thanks.jud")).unwrap();
    std::fs::remove_file(dir.join("invoice-vat.jud")).unwrap();
    std::fs::write(
        dir.join(".thanks.jud.1.0.tmp"),
        "apiVersion: jud/v1\nkind: Reco",
    )
    .unwrap();

    let second = server(triage_answers(), 2).await;
    let resumed = run(
        &["record", TRIAGE, TRIAGE_CASES, "--out", out],
        &second.uri(),
    );
    assert_eq!(code(&resumed), 0, "{}", stderr(&resumed));
    assert!(
        stderr(&resumed).contains("recorded 2, kept 5 in"),
        "{}",
        stderr(&resumed)
    );

    // And now it is whole: a third run asks nothing.
    let third = server(triage_answers(), 0).await;
    let again = run(
        &["record", TRIAGE, TRIAGE_CASES, "--out", out],
        &third.uri(),
    );
    assert_eq!(code(&again), 0, "{}", stderr(&again));
    assert!(
        stderr(&again).contains("recorded 0, kept 7 in"),
        "{}",
        stderr(&again)
    );
}
