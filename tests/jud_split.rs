//! `jud split` (decision 0022), run as a subprocess: the halves it writes,
//! what it refuses, and that recordings made over the whole set answer both
//! halves.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

mod support;

use std::path::Path;

use judgment::jud::Cases;
use support::{RECORDINGS, TRIAGE, TRIAGE_CASES, code, jud, scratch, stderr, stdout};

fn read(path: &Path) -> Cases {
    Cases::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn split(dir: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["split", TRIAGE_CASES, "--out", dir.to_str().unwrap()];
    args.extend_from_slice(extra);
    jud(&args, "", &[])
}

/// Every Nth case goes to the holdout half, the rest to the tune half, each
/// copied as read: put back in order, the halves are the whole set.
#[test]
fn every_nth_case_is_held_out_and_the_halves_make_the_whole() {
    let dir = scratch("split");
    let out = split(&dir, &["--every", "3"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let whole = read(Path::new(TRIAGE_CASES));
    let tune = read(&dir.join("triage-cases-tune.jud"));
    let holdout = read(&dir.join("triage-cases-holdout.jud"));

    assert_eq!(tune.name, format!("{}-tune", whole.name));
    assert_eq!(holdout.name, format!("{}-holdout", whole.name));
    assert_eq!(tune.rubric, whole.rubric);
    // 7 cases, every third held out: the 3rd and the 6th.
    let ids =
        |c: &Cases| -> Vec<String> { c.cases.iter().map(|c| c.id.clone().unwrap()).collect() };
    let all = ids(&whole);
    assert_eq!(ids(&holdout), [all[2].clone(), all[5].clone()]);
    let mut merged: Vec<_> = tune.cases.clone();
    merged.insert(2, holdout.cases[0].clone());
    merged.insert(5, holdout.cases[1].clone());
    assert_eq!(merged, whole.cases, "the halves are the whole set, as read");

    let text = stdout(&out);
    assert!(
        text.contains(&format!(
            "{}, 5 cases, cases {}",
            tune.name,
            tune.fingerprint()
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "{}, 2 cases, cases {}",
            holdout.name,
            holdout.fingerprint()
        )),
        "{text}"
    );
}

/// The halves bind to the rubric, and the recordings made over the whole set
/// answer them: a split never asks for a recording again.
#[test]
fn recordings_over_the_whole_set_answer_both_halves() {
    let dir = scratch("split-replay");
    assert_eq!(code(&split(&dir, &[])), 0);
    for half in ["triage-cases-tune.jud", "triage-cases-holdout.jud"] {
        let path = dir.join(half);
        let out = jud(
            &[
                "eval",
                TRIAGE,
                path.to_str().unwrap(),
                "--replay",
                RECORDINGS,
            ],
            "",
            &[],
        );
        assert_eq!(code(&out), 0, "{half}: {}", stderr(&out));
    }
}

/// A label the tuning set has and the holdout lacks is a warning on stderr:
/// a held-out number on an outcome never seen says little.
#[test]
fn a_label_one_half_lacks_is_a_warning() {
    let dir = scratch("split-warn");
    let out = split(&dir, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    // One case held out of 7: the holdout half cannot carry every label.
    assert!(
        stderr(&out).contains("jud: warning: the holdout half has no case labelling "),
        "{}",
        stderr(&out)
    );
}

/// Nothing is written over: not an existing half, and not the input.
#[test]
fn an_existing_half_is_refused_and_nothing_is_written() {
    let dir = scratch("split-twice");
    assert_eq!(code(&split(&dir, &[])), 0);
    let before = std::fs::read(dir.join("triage-cases-holdout.jud")).unwrap();
    let again = split(&dir, &[]);
    assert_eq!(code(&again), 2);
    assert!(
        stderr(&again).contains("already exists"),
        "{}",
        stderr(&again)
    );
    assert_eq!(
        std::fs::read(dir.join("triage-cases-holdout.jud")).unwrap(),
        before
    );
}

/// Fewer cases than N hold nothing out, and N under 2 holds out all: both
/// refused before anything is written.
#[test]
fn a_split_that_holds_nothing_out_is_refused() {
    let dir = scratch("split-few");
    let out = split(&dir, &["--every", "8"]);
    assert_eq!(code(&out), 2);
    assert!(
        stderr(&out).contains("fewer than --every 8"),
        "{}",
        stderr(&out)
    );
    assert_eq!(code(&split(&dir, &["--every", "1"])), 2);
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "nothing written"
    );
}
