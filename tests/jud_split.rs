//! `jud split` (decision 0022), run as a subprocess: the halves it writes,
//! what it refuses, and that recordings made over the whole set answer both
//! halves.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

mod support;

use std::path::Path;

use judgment::jud::Cases;
use support::{
    HANDOFF, HANDOFF_CASES, RECORDINGS, TRIAGE, TRIAGE_CASES, code, jud, scratch, stderr, stdout,
};

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

/// A Score level written as its index (`tone: 0`) and as its text (`calm`)
/// is one level once the rubric is given: counted together, in the levels'
/// order, and never warned about as a label of its own.
#[test]
fn with_the_rubric_a_level_written_as_index_or_text_is_one_level() {
    let dir = scratch("split-rubric");
    let out = split(&dir, &["--rubric", TRIAGE]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("  tone: calm 4, annoyed 1, angry 1"),
        "{text}"
    );
    assert!(!stderr(&out).contains("tone 0"), "{}", stderr(&out));
    assert!(!stderr(&out).contains("pass --rubric"), "{}", stderr(&out));

    // Without it, the labels are read as written, and a note says why.
    let bare = scratch("split-bare");
    let out = split(&bare, &[]);
    assert!(stdout(&out).contains("0 1"), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("pass --rubric to read it as the level's text"),
        "{}",
        stderr(&out)
    );
}

/// A rubric the cases do not fit is refused before anything is written.
#[test]
fn a_rubric_the_cases_do_not_fit_is_refused() {
    let dir = scratch("split-wrong-rubric");
    let out = split(&dir, &["--rubric", HANDOFF]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("does not fit"), "{}", stderr(&out));
    assert!(!dir.exists() || std::fs::read_dir(&dir).unwrap().count() == 0);
}

/// A conversation's `from_turn` labels the turns: `true` from that turn on,
/// `false` before it. Two conversations whose turns both say yes and no are
/// no gap, whatever turn each one turns on.
#[test]
fn from_turn_labels_are_read_as_the_turns_they_label() {
    let dir = scratch("split-turns");
    let out = jud(
        &[
            "split",
            HANDOFF_CASES,
            "--every",
            "2",
            "--out",
            dir.to_str().unwrap(),
        ],
        "",
        &[],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("wants_human: false 1, true 1"), "{text}");
    assert!(!stderr(&out).contains("from_turn"), "{}", stderr(&out));
}

/// The description says "every 2nd", not "every 2th", and lists one held-out
/// case without a trailing ellipsis.
#[test]
fn the_description_counts_in_words() {
    let dir = scratch("split-words");
    assert_eq!(code(&split(&dir, &["--every", "2"])), 0);
    let holdout = read(&dir.join("triage-cases-holdout.jud"));
    let description = holdout.description.unwrap();
    assert!(
        description.contains("every 2nd case, cases 2, 4, 6;"),
        "{description}"
    );
    let dir = scratch("split-words-one");
    assert_eq!(code(&split(&dir, &[])), 0);
    let holdout = read(&dir.join("triage-cases-holdout.jud"));
    let description = holdout.description.unwrap();
    assert!(
        description.contains("every 4th case, case 4;"),
        "{description}"
    );
}
