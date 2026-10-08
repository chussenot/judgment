//! `jud eval` (decision 0021), run as a subprocess: no key and no network.
//!
//! The triage cases are replayed from the recordings under
//! `examples/recordings/jud_calibration/`, so the numbers asserted here are
//! the ones `examples/jud_calibration.rs` prints; one test asks a wiremock
//! server instead, to show the same code path serves a backend.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

mod support;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Output;

use judgment::jud::{Cases, Rubric};
use serde_json::{Value, json};
use support::{
    HANDOFF, HANDOFF_CASES, RECORDINGS, TRIAGE, TRIAGE_CASES, code, copy_dir, jud, scratch, stderr,
    stdout,
};
use wiremock::matchers::{any, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// `jud eval RUBRIC CASES --replay RECORDINGS EXTRA...`
fn eval(rubric: &str, cases: &str, extra: &[&str]) -> Output {
    let mut args = vec!["eval", rubric, cases, "--replay", RECORDINGS];
    args.extend_from_slice(extra);
    jud(&args, "", &[])
}

/// The triage cases against the example recordings.
fn triage(extra: &[&str]) -> Output {
    eval(TRIAGE, TRIAGE_CASES, extra)
}

/// The report `--json` prints, parsed; the run must have succeeded or
/// failed only on a bar, and stdout must be exactly one JSON object.
fn report(output: &Output) -> Value {
    let text = stdout(output);
    assert!(
        text.starts_with("{\n") && text.ends_with("}\n"),
        "stdout is not one pretty JSON object: {text}\nstderr: {}",
        stderr(output)
    );
    serde_json::from_str(&text).unwrap()
}

fn question<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["id"] == id)
        .unwrap_or_else(|| panic!("no question {id} in {report}"))
}

/// Sorted key names of a JSON object.
fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// `text` written to a file in a fresh scratch directory; its path.
fn write(name: &str, text: &str) -> String {
    let file = scratch("eval").join(name);
    std::fs::write(&file, text).unwrap();
    file.to_str().unwrap().to_owned()
}

/// `text` with `from` replaced by `to`, which must be there: a fixture that
/// no longer matches fails here and not as a silent no-op.
fn replaced(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "fixture lost `{from}`");
    text.replacen(from, to, 1)
}

/// The triage rubric with the `desk` gate's bar moved.
fn triage_with_desk_bar(bar: &str) -> String {
    write(
        "triage.jud",
        &replaced(
            &read(TRIAGE),
            "      confidence: 0.45\n",
            &format!("      confidence: {bar}\n"),
        ),
    )
}

#[test]
fn the_triage_cases_replayed_give_the_accuracies_and_briers_the_calibration_example_prints() {
    let out = triage(&["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
    let report = report(&out);

    // examples/jud_calibration.rs prints these: 6/7, 6/7 and 6/6, and the
    // Brier scores to three places.
    let expected = [
        ("actionable", 7, 6, "0.857", "0.090"),
        ("desk", 7, 6, "0.857", "0.155"),
        ("tone", 6, 6, "1.000", "0.059"),
    ];
    for (id, labelled, correct, accuracy, brier) in expected {
        let q = question(&report, id);
        assert_eq!(q["labelled"], labelled, "{id}");
        assert_eq!(q["correct"], correct, "{id}");
        assert_eq!(
            format!("{:.3}", q["accuracy"].as_f64().unwrap()),
            accuracy,
            "{id}"
        );
        assert_eq!(
            format!("{:.3}", q["brier"].as_f64().unwrap()),
            brier,
            "{id}"
        );
        assert!(q["ece"].is_f64(), "{id}");
        assert!(q["confidence_when_right"].is_f64(), "{id}");
    }
    // The order is the rubric's.
    let order: Vec<&str> = report["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["id"].as_str().unwrap())
        .collect();
    assert_eq!(order, ["actionable", "desk", "tone"]);
    // Nobody was wrong on tone, so there is no confidence to report for it.
    assert!(question(&report, "tone")["confidence_when_wrong"].is_null());
    // Seven cases, seven requests, one model.
    assert_eq!(report["requests"], 7);
    assert_eq!(report["cases"]["count"], 7);
    assert_eq!(report["models"], json!(["jev-1.13.0"]));
}

#[test]
fn the_accuracy_interval_is_the_wilson_interval_as_a_pair() {
    let report = report(&triage(&["--json"]));
    let interval = question(&report, "desk")["accuracy_interval95"]
        .as_array()
        .unwrap();
    assert_eq!(interval.len(), 2);
    let (low, high) = (interval[0].as_f64().unwrap(), interval[1].as_f64().unwrap());
    // 6 of 7 at 95%: the library's interval, 0.49 to 0.97.
    assert!(
        (low - 0.487).abs() < 1e-3 && (high - 0.974).abs() < 1e-3,
        "{low} {high}"
    );
}

#[test]
fn the_text_report_names_the_documents_and_each_question() {
    let out = triage(&[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
    let text = stdout(&out);
    let rubric = Rubric::parse(&read(TRIAGE)).unwrap();
    let cases = Cases::parse(&read(TRIAGE_CASES)).unwrap();
    // The header names the rubric and the cases by what identifies them.
    assert!(text.contains("rubric inbox-triage"), "{text}");
    assert!(text.contains(&rubric.fingerprint()), "{text}");
    assert!(text.contains(&rubric.policy_fingerprint()), "{text}");
    assert!(text.contains("cases inbox-triage-cases"), "{text}");
    assert!(text.contains(&cases.fingerprint()), "{text}");
    assert!(text.contains("7 requests, model jev-1.13.0"), "{text}");
    // One block per question, with the numbers the example prints.
    assert!(text.contains("labelled 7, correct 6, accuracy 0.86 (95% interval 0.49 to 0.97)"));
    assert!(text.contains("labelled 6, correct 6, accuracy 1.00"));
    assert!(text.contains("brier 0.090"), "{text}");
    assert!(text.contains("brier 0.155"), "{text}");
    assert!(text.contains("brier 0.059"), "{text}");
    // No bar given, so no gate section.
    assert!(!text.contains("--min-accuracy"), "{text}");
}

#[test]
fn the_misses_name_the_case_what_was_expected_and_what_was_given() {
    let report = report(&triage(&["--json"]));
    // The automated receipt is the one both the Noul (0.52, so yes) and the
    // desk (billing at 0.40) got wrong.
    let actionable = question(&report, "actionable")["misses"]
        .as_array()
        .unwrap();
    assert_eq!(actionable.len(), 1);
    assert_eq!(actionable[0]["case"], "receipt");
    assert_eq!(actionable[0]["expected"], "no");
    assert_eq!(actionable[0]["predicted"], "yes");
    assert!((actionable[0]["confidence"].as_f64().unwrap() - 0.52).abs() < 1e-9);
    let desk = question(&report, "desk")["misses"].as_array().unwrap();
    assert_eq!(desk.len(), 1);
    assert_eq!(desk[0]["case"], "receipt");
    assert_eq!(desk[0]["expected"], "none_of_these");
    assert_eq!(desk[0]["predicted"], "billing");
    assert!((desk[0]["confidence"].as_f64().unwrap() - 0.40).abs() < 1e-9);
    assert!(
        question(&report, "tone")["misses"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let text = stdout(&triage(&[]));
    assert!(text.contains("misses (2)"), "{text}");
    assert!(
        text.contains("receipt: expected none_of_these, predicted billing, confidence 0.40"),
        "{text}"
    );
}

#[test]
fn the_json_report_has_every_documented_key_and_no_other() {
    let report = report(&triage(&["--json", "--min-accuracy", "desk=0.5"]));
    assert_eq!(
        keys(&report),
        [
            "cases",
            "min_accuracy",
            "models",
            "questions",
            "requests",
            "rubric"
        ]
    );
    assert_eq!(
        keys(&report["rubric"]),
        ["fingerprint", "name", "policy_fingerprint"]
    );
    assert_eq!(keys(&report["cases"]), ["count", "fingerprint", "name"]);
    let desk = question(&report, "desk");
    assert_eq!(
        keys(desk),
        [
            "accuracy",
            "accuracy_interval95",
            "brier",
            "confidence_when_right",
            "confidence_when_wrong",
            "correct",
            "ece",
            "gate",
            "id",
            "labelled",
            "misses"
        ]
    );
    assert_eq!(
        keys(&desk["gate"]),
        ["accuracy_when_acted", "acted", "deferred"]
    );
    assert_eq!(
        keys(&desk["misses"][0]),
        ["case", "confidence", "expected", "predicted"]
    );
    assert_eq!(
        keys(&report["min_accuracy"][0]),
        ["accuracy", "bar", "labelled", "met", "question"]
    );
    // The fingerprints are the crate's own.
    let rubric = Rubric::parse(&read(TRIAGE)).unwrap();
    let cases = Cases::parse(&read(TRIAGE_CASES)).unwrap();
    assert_eq!(report["rubric"]["fingerprint"], rubric.fingerprint());
    assert_eq!(
        report["rubric"]["policy_fingerprint"],
        rubric.policy_fingerprint()
    );
    assert_eq!(report["cases"]["fingerprint"], cases.fingerprint());
    // Without a bar the list is there and empty, so a script need not test for it.
    let none = self::report(&triage(&["--json"]));
    assert_eq!(none["min_accuracy"], json!([]));
}

#[test]
fn the_gate_reports_what_the_policy_acts_on_and_defers() {
    let report = report(&triage(&["--json"]));
    // The committed desk bar (0.45) defers the receipt (billing at 0.40), the
    // one wrong desk: 6 acted, all right.
    let desk = &question(&report, "desk")["gate"];
    assert_eq!(
        (desk["acted"].as_u64(), desk["deferred"].as_u64()),
        (Some(6), Some(1))
    );
    assert_eq!(desk["accuracy_when_acted"], 1.0);
    // A Noul's gate never defers; the Score's bar 0.3 lets all seven through,
    // and the six that carry a label are all right.
    let actionable = &question(&report, "actionable")["gate"];
    assert_eq!(
        (
            actionable["acted"].as_u64(),
            actionable["deferred"].as_u64()
        ),
        (Some(7), Some(0))
    );
    let tone = &question(&report, "tone")["gate"];
    assert_eq!(
        (tone["acted"].as_u64(), tone["deferred"].as_u64()),
        (Some(7), Some(0))
    );
    assert_eq!(tone["accuracy_when_acted"], 1.0);
    let text = stdout(&triage(&[]));
    assert!(
        text.contains("gate: acts on 6 of 7, defers 1, accuracy when acted 1.00"),
        "{text}"
    );
}

#[test]
fn the_gate_coverage_follows_the_policys_bar_not_a_constant() {
    // Confidences on desk, by case: 0.87 0.77 0.89 0.40 0.73 0.47 0.81.
    // At 0.8 only three act (refund-angry, login-loop, invoice-vat) and all
    // three are right; at 0.35 all seven act, so the wrong one counts.
    for (bar, acted, deferred, accuracy) in [("0.8", 3, 4, 1.0), ("0.35", 7, 0, 6.0 / 7.0)] {
        let rubric = triage_with_desk_bar(bar);
        let out = eval(&rubric, TRIAGE_CASES, &["--json"]);
        assert_eq!(code(&out), 0, "{}", stderr(&out));
        let gate = question(&report(&out), "desk")["gate"].clone();
        assert_eq!(gate["acted"], acted, "bar {bar}");
        assert_eq!(gate["deferred"], deferred, "bar {bar}");
        assert!(
            (gate["accuracy_when_acted"].as_f64().unwrap() - accuracy).abs() < 1e-9,
            "bar {bar}"
        );
    }
}

#[test]
fn a_question_without_a_gate_has_no_gate_in_the_report() {
    let rubric = write(
        "triage.jud",
        &replaced(&read(TRIAGE), "    tone:\n      confidence: 0.3\n", ""),
    );
    let out = eval(&rubric, TRIAGE_CASES, &["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let report = report(&out);
    assert!(question(&report, "tone")["gate"].is_null());
    assert!(question(&report, "desk")["gate"].is_object());
    // The metrics do not depend on the gate.
    assert_eq!(question(&report, "tone")["correct"], 6);
    let text = stdout(&eval(&rubric, TRIAGE_CASES, &[]));
    assert!(text.contains("gate: none"), "{text}");
}

#[test]
fn a_question_nobody_labelled_is_listed_as_not_labelled_without_metrics() {
    let cases = write(
        "cases.jud",
        &read(TRIAGE_CASES)
            .lines()
            .filter(|line| !line.starts_with("        tone:"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let out = eval(TRIAGE, &cases, &["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let report = report(&out);
    let tone = question(&report, "tone");
    assert_eq!(tone["labelled"], 0);
    for key in [
        "accuracy",
        "accuracy_interval95",
        "brier",
        "ece",
        "confidence_when_right",
    ] {
        assert!(tone[key].is_null(), "{key}");
    }
    // What the gate does needs no label: it still acts on all seven.
    assert_eq!(tone["gate"]["acted"], 7);
    assert!(tone["gate"]["accuracy_when_acted"].is_null());
    assert!(stdout(&eval(TRIAGE, &cases, &[])).contains("tone\n  not labelled\n"));
}

#[test]
fn a_bar_that_is_met_exits_0_and_says_so() {
    let out = triage(&["--min-accuracy", "0.8"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("--min-accuracy\n"), "{text}");
    assert!(text.contains("  met      actionable 0.86 >= 0.8"), "{text}");
    assert!(text.contains("  met      desk 0.86 >= 0.8"), "{text}");
    assert!(text.contains("  met      tone 1.00 >= 0.8"), "{text}");
    // Both ends of the range are bars: 0 is always met by a labelled question.
    assert_eq!(code(&triage(&["--min-accuracy", "0"])), 0);
}

#[test]
fn a_bare_bar_applies_to_every_labelled_question_and_exits_3_when_one_falls_short() {
    let out = triage(&["--json", "--min-accuracy", "0.9"]);
    assert_eq!(code(&out), 3);
    // stdout is still the report; stderr is the one line.
    let checks = report(&out)["min_accuracy"].clone();
    let checks = checks.as_array().unwrap();
    assert_eq!(checks.len(), 3);
    let met: Vec<(&str, bool)> = checks
        .iter()
        .map(|c| (c["question"].as_str().unwrap(), c["met"].as_bool().unwrap()))
        .collect();
    assert_eq!(
        met,
        [("actionable", false), ("desk", false), ("tone", true)]
    );
    assert_eq!(checks[0]["bar"], 0.9);
    assert_eq!(checks[0]["labelled"], 7);
    assert_eq!(
        stderr(&out),
        "jud: 2 question(s) below --min-accuracy: actionable 0.86 < 0.9, desk 0.86 < 0.9\n"
    );
}

#[test]
fn a_bar_on_one_question_gates_that_question_only() {
    let out = triage(&["--min-accuracy", "desk=0.95"]);
    assert_eq!(code(&out), 3);
    assert_eq!(
        stderr(&out),
        "jud: 1 question(s) below --min-accuracy: desk 0.86 < 0.95\n"
    );
    let text = stdout(&out);
    // The report is printed as usual, then the gate lines.
    assert!(text.contains("misses (2)"), "{text}");
    assert!(
        text.ends_with("--min-accuracy\n  NOT MET  desk 0.86 < 0.95\n"),
        "{text}"
    );
    // Another question's shortfall is not this gate's business.
    let tone = triage(&["--min-accuracy", "tone=1"]);
    assert_eq!(code(&tone), 0, "{}", stderr(&tone));
    assert!(
        stdout(&tone).contains("  met      tone 1.00 >= 1"),
        "{}",
        stdout(&tone)
    );
}

#[test]
fn gates_repeat_and_every_unmet_one_is_named() {
    let out = triage(&[
        "--min-accuracy",
        "0.8",
        "--min-accuracy",
        "desk=0.95",
        "--min-accuracy",
        "desk=0.9",
    ]);
    assert_eq!(code(&out), 3);
    // Two bars on desk are two entries but one question.
    assert_eq!(
        stderr(&out),
        "jud: 1 question(s) below --min-accuracy: desk 0.86 < 0.95, desk 0.86 < 0.9\n"
    );
}

#[test]
fn six_sevenths_never_rounds_into_a_pass() {
    // 6/7 is 0.857..., and prints as 0.86: a bar of 0.86 is still not met,
    // and the line says why in digits that show it.
    let out = triage(&["--min-accuracy", "desk=0.86"]);
    assert_eq!(code(&out), 3);
    assert_eq!(
        stderr(&out),
        "jud: 1 question(s) below --min-accuracy: desk 0.857 < 0.86\n"
    );
    // The tolerance is 1e-9 and no more: a bar 4e-10 above 6/7 is the same
    // number as far as a float can tell, 9e-9 above is not.
    assert_eq!(code(&triage(&["--min-accuracy", "desk=0.8571428575"])), 0);
    assert_eq!(code(&triage(&["--min-accuracy", "desk=0.857142866"])), 3);
}

#[test]
fn a_question_with_no_labelled_case_does_not_meet_a_bar_on_it() {
    let cases = write(
        "cases.jud",
        &read(TRIAGE_CASES)
            .lines()
            .filter(|line| !line.starts_with("        tone:"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let out = eval(TRIAGE, &cases, &["--min-accuracy", "tone=0"]);
    assert_eq!(code(&out), 3, "{}", stderr(&out));
    assert_eq!(
        stderr(&out),
        "jud: 1 question(s) below --min-accuracy: tone not labelled < 0\n"
    );
    assert!(
        stdout(&out).contains("  NOT MET  tone not labelled < 0"),
        "{}",
        stdout(&out)
    );
    // A bare bar applies to the questions that have labels, so it passes here.
    let bare = eval(TRIAGE, &cases, &["--min-accuracy", "0.8"]);
    assert_eq!(code(&bare), 0, "{}", stderr(&bare));
}

#[test]
fn a_bare_bar_over_cases_with_no_labels_at_all_is_not_met() {
    let cases = write(
        "cases.jud",
        "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: unlabelled\nspec:\n  rubric: inbox-triage\n  cases:\n    - id: thanks\n      state:\n        message: Sorted now, thanks a lot for the quick help!\n",
    );
    let free = eval(TRIAGE, &cases, &[]);
    assert_eq!(code(&free), 0, "{}", stderr(&free));
    assert_eq!(
        stdout(&free).matches("not labelled").count(),
        3,
        "{}",
        stdout(&free)
    );
    // A gate that passes because nothing was measured is the one CI must not have.
    let gated = eval(TRIAGE, &cases, &["--min-accuracy", "0.5"]);
    assert_eq!(code(&gated), 3, "{}", stderr(&gated));
    assert!(
        stderr(&gated).contains("(no question is labelled) < 0.5"),
        "{}",
        stderr(&gated)
    );
}

#[test]
fn without_a_bar_the_status_is_0_whatever_the_accuracy() {
    // Relabel the billing cases as account: the model is now wrong about
    // three of seven desks (the receipt as before), and the command still
    // succeeds, since nobody asked it to hold a bar.
    let cases = write(
        "cases.jud",
        &read(TRIAGE_CASES).replace("desk: billing", "desk: account"),
    );
    let out = eval(TRIAGE, &cases, &["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
    let report = report(&out);
    let desk = question(&report, "desk");
    assert_eq!(
        (desk["labelled"].as_u64(), desk["correct"].as_u64()),
        (Some(7), Some(4))
    );
    assert_eq!(desk["misses"].as_array().unwrap().len(), 3);
    // The same labels under a bar are status 3.
    assert_eq!(
        code(&eval(TRIAGE, &cases, &["--min-accuracy", "desk=0.8"])),
        3
    );
}

#[tokio::test]
async fn a_bar_naming_an_unknown_question_is_refused_before_any_call() {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let out = jud(
        &["eval", TRIAGE, TRIAGE_CASES, "--min-accuracy", "dsk=0.9"],
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &server.uri()),
        ],
    );
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    let message = stderr(&out);
    assert!(message.starts_with("jud: "), "{message}");
    assert!(message.contains("`dsk`"), "{message}");
    assert!(message.contains("actionable, desk, tone"), "{message}");
    assert!(stdout(&out).is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());

    // Before the recordings too: nothing is opened for a typo.
    let replayed = jud(
        &[
            "eval",
            TRIAGE,
            TRIAGE_CASES,
            "--replay",
            "/nonexistent/recordings",
            "--min-accuracy",
            "dsk=0.9",
        ],
        "",
        &[],
    );
    assert_eq!(code(&replayed), 2);
    assert!(stderr(&replayed).contains("`dsk`"), "{}", stderr(&replayed));
}

#[test]
fn a_malformed_bar_is_a_usage_error() {
    for (bar, why) in [
        ("1.5", "1.5 is not from 0 to 1"),
        ("-0.1", "-0.1 is not from 0 to 1"),
        ("high", "`high` is not a number from 0 to 1"),
        ("desk=", "`` is not a number from 0 to 1"),
        ("=0.9", "the question before `=` is empty"),
    ] {
        // `=` keeps clap from reading a leading `-` as another flag.
        let out = triage(&[&format!("--min-accuracy={bar}")]);
        assert_eq!(code(&out), 2, "{bar}");
        assert!(stderr(&out).contains(why), "{bar}: {}", stderr(&out));
        assert!(stdout(&out).is_empty(), "{bar}");
    }
}

#[test]
fn a_missing_recording_is_exit_1_and_names_every_case_that_has_none() {
    let dir = copy_dir(RECORDINGS);
    std::fs::remove_file(dir.join("invoice-vat.jud")).unwrap();
    std::fs::remove_file(dir.join("login-loop.jud")).unwrap();
    let out = jud(
        &[
            "eval",
            TRIAGE,
            TRIAGE_CASES,
            "--replay",
            dir.to_str().unwrap(),
            "--min-accuracy",
            "0.5",
        ],
        "",
        &[],
    );
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    let message = stderr(&out);
    assert!(message.starts_with("jud: "), "{message}");
    assert!(
        message.contains("no recording answers 2 cases: login-loop, invoice-vat"),
        "{message}"
    );
    // No half a report on stdout, and no gate ran.
    assert!(stdout(&out).is_empty(), "{}", stdout(&out));
}

#[test]
fn the_handoff_conversations_are_graded_turn_by_turn() {
    let out = eval(HANDOFF, HANDOFF_CASES, &["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let report = report(&out);
    // Two cases of three turns: six requests, each graded against the label
    // `from_turn` resolves to at that turn.
    assert_eq!(report["cases"]["count"], 2);
    assert_eq!(report["requests"], 6);
    let q = question(&report, "wants_human");
    assert_eq!(q["labelled"], 6);
    assert_eq!(q["correct"], 6);
    assert_eq!(q["accuracy"], 1.0);
    assert_eq!(q["gate"]["acted"], 6);
    assert!(q["misses"].as_array().unwrap().is_empty());
    assert_eq!(report["rubric"]["name"], "handoff");
}

#[test]
fn a_miss_in_a_conversation_names_the_turn() {
    // Labelled as wanting a person from the first turn on, the conversation
    // `escalates` is wrong at the turns where the model did not yet say so.
    let cases = write(
        "handoff-cases.jud",
        &replaced(
            &read(HANDOFF_CASES),
            "wants_human: {from_turn: 2}",
            "wants_human: {from_turn: 0}",
        ),
    );
    let out = eval(HANDOFF, &cases, &["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let report = report(&out);
    let misses = question(&report, "wants_human")["misses"]
        .as_array()
        .unwrap()
        .clone();
    let names: Vec<&str> = misses.iter().map(|m| m["case"].as_str().unwrap()).collect();
    assert_eq!(names, ["escalates-turn-0", "escalates-turn-1"]);
    assert!(
        misses
            .iter()
            .all(|m| m["expected"] == "yes" && m["predicted"] == "no")
    );
    assert_eq!(report["requests"], 6);
    assert_eq!(question(&report, "wants_human")["correct"], 4);
    let text = stdout(&eval(HANDOFF, &cases, &[]));
    assert!(
        text.contains("escalates-turn-0: expected yes, predicted no"),
        "{text}"
    );
}

#[test]
fn jud_replay_in_the_environment_works_like_the_flag() {
    let flag = triage(&["--json"]);
    let by_env = jud(
        &["eval", TRIAGE, TRIAGE_CASES, "--json"],
        "",
        &[("JUD_REPLAY", RECORDINGS)],
    );
    assert_eq!(code(&by_env), 0, "{}", stderr(&by_env));
    assert_eq!(stdout(&by_env), stdout(&flag));
}

#[test]
fn without_recordings_or_a_key_it_is_refused_before_any_call() {
    let out = jud(&["eval", TRIAGE, TRIAGE_CASES], "", &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(
        stderr(&out).starts_with("jud: no API key: set TYPESAFE_API_KEY"),
        "{}",
        stderr(&out)
    );
    assert!(stdout(&out).is_empty());
}

#[test]
fn a_recordings_directory_that_does_not_exist_is_refused() {
    let out = jud(
        &[
            "eval",
            TRIAGE,
            TRIAGE_CASES,
            "--replay",
            "/nonexistent/recordings",
        ],
        "",
        &[],
    );
    assert_eq!(code(&out), 2);
    assert!(
        stderr(&out).contains("cannot replay from /nonexistent/recordings"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_file_that_is_not_the_right_document_is_refused_by_name() {
    // The cases file where the rubric goes.
    let out = eval(TRIAGE_CASES, TRIAGE_CASES, &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(
        stderr(&out).contains(&format!("{TRIAGE_CASES} is not a valid Rubric")),
        "{}",
        stderr(&out)
    );
    // The rubric where the cases go.
    let out = eval(TRIAGE, TRIAGE, &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(
        stderr(&out).contains(&format!("{TRIAGE} is not a valid Cases")),
        "{}",
        stderr(&out)
    );
    // Not a document at all, and not a file at all.
    let notes = write("notes.jud", "just some notes\n");
    let out = eval(&notes, TRIAGE_CASES, &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(stderr(&out).contains(&notes), "{}", stderr(&out));
    let out = eval(TRIAGE, "/nonexistent/cases.jud", &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(
        stderr(&out).contains("cannot read cases /nonexistent/cases.jud"),
        "{}",
        stderr(&out)
    );
    assert!(stdout(&out).is_empty());
}

#[test]
fn cases_that_do_not_bind_to_the_rubric_are_refused_naming_both_files() {
    let other = write(
        "other.jud",
        &replaced(
            &read(TRIAGE_CASES),
            "rubric: inbox-triage",
            "rubric: another-rubric",
        ),
    );
    let out = eval(TRIAGE, &other, &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(
        stderr(&out).contains(&format!("{other} does not fit {TRIAGE}")),
        "{}",
        stderr(&out)
    );

    let wrong_label = write(
        "wrong-label.jud",
        &replaced(&read(TRIAGE_CASES), "desk: billing", "desk: refunds"),
    );
    let out = eval(TRIAGE, &wrong_label, &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(
        stderr(&out).contains(&format!("{wrong_label} does not fit")),
        "{}",
        stderr(&out)
    );
    assert!(stderr(&out).contains("refunds"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty());
}

/// Every file under `dir` and the two documents, by path, as bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files: BTreeMap<String, Vec<u8>> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .map(|p| (p.display().to_string(), std::fs::read(&p).unwrap()))
        .collect();
    for document in [TRIAGE, TRIAGE_CASES] {
        files.insert(document.to_owned(), std::fs::read(document).unwrap());
    }
    files
}

#[test]
fn eval_is_read_only_and_ignores_stdin() {
    let dir = copy_dir(RECORDINGS);
    let before = snapshot(&dir);
    let args = [
        "eval",
        TRIAGE,
        TRIAGE_CASES,
        "--replay",
        dir.to_str().unwrap(),
        "--json",
    ];
    // Whatever is piped in is not read as a state, and not echoed.
    let out = jud(&args, "this is not JSON at all", &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(stdout(&out), stdout(&triage(&["--json"])));
    assert_eq!(snapshot(&dir), before, "eval wrote to a file");
}

/// The triage rubric's answers, as a server sends them: the same for every
/// case, so what the report says follows from the labels alone.
fn server_answers() -> Value {
    json!({
        "model": "jev-1.13.0",
        "answers": {
            "actionable": { "type": "noul", "noul": 0.91 },
            "desk": { "type": "choice", "choice": "billing",
                      "probabilities": { "billing": 0.85, "technical": 0.05, "account": 0.05, "none_of_these": 0.05 },
                      "confidence": 0.8 },
            "tone": { "type": "score", "score": 1.9,
                      "legend": { "0": "calm", "1": "annoyed", "2": "angry" },
                      "probabilities": { "0": 0.05, "1": 0.0, "2": 0.95 },
                      "confidence": 0.9 }
        },
        "usage": { "input_tokens": 300, "output_tokens": 30 }
    })
}

#[tokio::test]
async fn without_recordings_it_asks_the_server_once_per_case() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(server_answers()))
        .expect(7)
        .mount(&server)
        .await;
    let out = jud(
        &[
            "eval",
            TRIAGE,
            TRIAGE_CASES,
            "--json",
            "--min-accuracy",
            "actionable=0.7",
        ],
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &server.uri()),
        ],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
    assert_eq!(server.received_requests().await.unwrap().len(), 7);
    let report = report(&out);
    assert_eq!(report["requests"], 7);
    assert_eq!(report["models"], json!(["jev-1.13.0"]));
    // The same answer for every case, graded against the seven labels: yes is
    // right for the five requests, billing for the two billing cases, angry
    // for the one angry writer of the six that have a tone.
    let correct: Vec<(&str, u64)> = ["actionable", "desk", "tone"]
        .into_iter()
        .map(|id| (id, question(&report, id)["correct"].as_u64().unwrap()))
        .collect();
    assert_eq!(correct, [("actionable", 5), ("desk", 2), ("tone", 1)]);
    assert_eq!(question(&report, "tone")["labelled"], 6);
    // The server's confidence is read through the committed policy: desk's
    // bar is 0.45 and the answers say 0.8, so nothing is deferred.
    assert_eq!(question(&report, "desk")["gate"]["deferred"], 0);
    assert_eq!(report["min_accuracy"][0]["met"], true);

    // A bar the same answers do not reach is status 3, from a server too.
    let second = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(server_answers()))
        .expect(7)
        .mount(&second)
        .await;
    let unmet = jud(
        &["eval", TRIAGE, TRIAGE_CASES, "--min-accuracy", "desk=0.5"],
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &second.uri()),
        ],
    );
    assert_eq!(code(&unmet), 3, "{}", stderr(&unmet));
    assert_eq!(
        stderr(&unmet),
        "jud: 1 question(s) below --min-accuracy: desk 0.29 < 0.5\n"
    );
}

#[tokio::test]
async fn a_failing_server_is_exit_1_and_stops_at_the_first_failure() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error": "bad key"})))
        .expect(1)
        .mount(&server)
        .await;
    let out = jud(
        &["eval", TRIAGE, TRIAGE_CASES, "--json"],
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &server.uri()),
        ],
    );
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    assert!(
        stderr(&out).starts_with("jud: the backend at "),
        "{}",
        stderr(&out)
    );
    assert!(stdout(&out).is_empty());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
