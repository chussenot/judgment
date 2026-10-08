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
use std::process::{Output, Stdio};

use judgment::jud::{Cases, Rubric};
use serde_json::{Value, json};
use support::{
    HANDOFF, HANDOFF_CASES, RECORDINGS, TRIAGE, TRIAGE_CASES, cases_without_labels, code, copy_dir,
    jud, scratch, server, stderr, stdout, triage_answers,
};
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// `jud eval RUBRIC CASES --replay RECORDINGS EXTRA...`
fn eval_with(recordings: &str, rubric: &str, cases: &str, extra: &[&str]) -> Output {
    let mut args = vec!["eval", rubric, cases, "--replay", recordings];
    args.extend_from_slice(extra);
    jud(&args, "", &[])
}

/// The same against the example recordings.
fn eval(rubric: &str, cases: &str, extra: &[&str]) -> Output {
    eval_with(RECORDINGS, rubric, cases, extra)
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

/// A copy of the example recordings in which the files named (every one when
/// `files` is empty) give `model`, a YAML scalar as written; the directory.
fn recordings_naming(model: &str, files: &[&str]) -> String {
    let dir = copy_dir(RECORDINGS);
    let files: Vec<String> = if files.is_empty() {
        std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect()
    } else {
        files.iter().map(|f| format!("{f}.jud")).collect()
    };
    for file in files {
        let path = dir.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        let renamed = replaced(
            &text,
            "    model: jev-1.13.0\n",
            &format!("    model: {model}\n"),
        );
        std::fs::write(&path, renamed).unwrap();
    }
    dir.to_str().unwrap().to_owned()
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
    }
    // The calibration numbers, to three places, from the recordings by hand.
    // A Noul's confidence is max(p, 1 - p), a Choice's and a Score's the one
    // the answer carries; "right" and "wrong" are by the question's own
    // accuracy. The calibration error is the ten-equal-bin sum
    // (n_b / n) * |mean confidence_b - accuracy_b|:
    //   actionable: 0.97 0.92 0.93 0.90 0.91 right (bin 9: mean 0.926, so
    //     0.074 * 5/7), 0.88 right (0.12 * 1/7), the receipt 0.52 wrong
    //     (0.52 * 1/7): 0.144; right 5.51 / 6 = 0.918; wrong 0.52.
    //   desk: 0.87 0.89 0.81 right (mean 0.857, 0.143 * 3/7), 0.77 0.73 right
    //     (0.25 * 2/7), 0.47 right with the receipt's 0.40 wrong (bin 4,
    //     0.065 * 2/7): 0.151; right 4.54 / 6 = 0.757; wrong 0.40.
    //   tone (six labelled, all right): 0.84 0.80 0.87 0.89 (0.15 * 4/6),
    //     0.93 (0.07 * 1/6), 0.46 (0.54 * 1/6): 0.202; right 4.79 / 6 = 0.798;
    //     nobody was wrong, so no confidence when wrong.
    let calibration = [
        ("actionable", "0.144", "0.918", Some("0.520")),
        ("desk", "0.151", "0.757", Some("0.400")),
        ("tone", "0.202", "0.798", None),
    ];
    for (id, ece, right, wrong) in calibration {
        let q = question(&report, id);
        assert_eq!(format!("{:.3}", q["ece"].as_f64().unwrap()), ece, "{id}");
        assert_eq!(
            format!("{:.3}", q["confidence_when_right"].as_f64().unwrap()),
            right,
            "{id}"
        );
        match wrong {
            Some(wrong) => assert_eq!(
                format!("{:.3}", q["confidence_when_wrong"].as_f64().unwrap()),
                wrong,
                "{id}"
            ),
            None => assert!(q["confidence_when_wrong"].is_null(), "{id}"),
        }
    }
    // The order is the rubric's.
    let order: Vec<&str> = report["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["id"].as_str().unwrap())
        .collect();
    assert_eq!(order, ["actionable", "desk", "tone"]);
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
    // The metrics line is brier, calibration error, then the two confidences,
    // in that order (the numbers are derived in the JSON test above).
    for line in [
        "brier 0.090, calibration error 0.144, confidence when right 0.92, when wrong 0.52",
        "brier 0.155, calibration error 0.151, confidence when right 0.76, when wrong 0.40",
        "brier 0.059, calibration error 0.202, confidence when right 0.80, when wrong -",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
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
    // Titled for whose misses they are: the model's, which is not what the
    // gate section counts.
    assert!(text.contains("\nmodel misses (2)\n"), "{text}");
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
    // The policy's verdicts at threshold 0.55 are all right, the receipt's
    // 0.52 included (a `no`, as labelled): 7 of 7, where the model's reading
    // at 0.5 gets the same receipt wrong (see the next test).
    assert_eq!(actionable["accuracy_when_acted"], 1.0);
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
    let cases = write("cases.jud", &cases_without_labels("tone"));
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
    assert!(text.contains("model misses (2)"), "{text}");
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
    let cases = write("cases.jud", &cases_without_labels("tone"));
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
    assert_eq!(code(&flag), 0, "{}", stderr(&flag));
    let by_env = jud(
        &["eval", TRIAGE, TRIAGE_CASES, "--json"],
        "",
        &[("JUD_REPLAY", RECORDINGS)],
    );
    assert_eq!(code(&by_env), 0, "{}", stderr(&by_env));
    // `report` requires one pretty JSON object, so two empty outputs cannot
    // pass for a match.
    let by_env = report(&by_env);
    assert_eq!(by_env["rubric"]["name"], "inbox-triage");
    assert_eq!(by_env, report(&flag));
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
    assert_eq!(report(&out), report(&triage(&["--json"])));
    assert_eq!(snapshot(&dir), before, "eval wrote to a file");
}

#[tokio::test]
async fn without_recordings_it_asks_the_server_once_per_case() {
    let server = server(triage_answers(), 7).await;
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
    let second = self::server(triage_answers(), 7).await;
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
    // The failure names the case it stopped at and where it was in the run,
    // since nothing is kept: the first case, of seven, and no other was asked.
    assert!(
        stderr(&out).starts_with("jud: case refund-angry (1 of 7): the backend at "),
        "{}",
        stderr(&out)
    );
    assert!(stdout(&out).is_empty());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

/// The gate's section of one question, from a run of `rubric` over the
/// triage cases and `recordings`.
fn gate_of(recordings: &str, rubric: &str, id: &str) -> (Value, Value) {
    let out = eval_with(recordings, rubric, TRIAGE_CASES, &["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let report = report(&out);
    let q = question(&report, id);
    (q["gate"].clone(), q.clone())
}

/// What the model did on a question, with the policy's gate left out: the
/// part a moved bar must not move.
fn model_side(question: &Value) -> Value {
    let mut model = question.clone();
    model.as_object_mut().unwrap().remove("gate");
    model
}

/// `value` is `numerator / denominator`, to float precision.
fn is_ratio(value: &Value, numerator: u32, denominator: u32) -> bool {
    (value.as_f64().unwrap() - f64::from(numerator) / f64::from(denominator)).abs() < 1e-12
}

#[test]
fn a_noul_gates_accuracy_is_that_of_its_own_verdicts_and_not_the_models_reading_at_half() {
    let (gate, question) = gate_of(RECORDINGS, TRIAGE, "actionable");
    // The committed threshold is 0.55, so the receipt (0.52, labelled false)
    // is a `no` and every one of the seven verdicts is right ...
    assert_eq!(
        (gate["acted"].as_u64(), gate["deferred"].as_u64()),
        (Some(7), Some(0))
    );
    assert_eq!(gate["accuracy_when_acted"], 1.0, "{gate}");
    // ... while the question's accuracy is the model's reading at 0.5, which
    // says yes to the receipt and is right 6 times of 7. Both are reported,
    // and neither stands in for the other.
    assert_eq!(
        (question["labelled"].as_u64(), question["correct"].as_u64()),
        (Some(7), Some(6))
    );
    assert!(is_ratio(&question["accuracy"], 6, 7), "{question}");
    let text = stdout(&triage(&[]));
    assert!(
        text.contains("labelled 7, correct 6, accuracy 0.86 (95% interval 0.49 to 0.97)"),
        "{text}"
    );
    assert!(
        text.contains("gate: acts on 7 of 7, defers 0, accuracy when acted 1.00"),
        "{text}"
    );
}

#[test]
fn moving_a_noul_gates_threshold_moves_the_policys_accuracy_and_not_the_models() {
    let committed = gate_of(RECORDINGS, TRIAGE, "actionable");
    // The probabilities of yes in the recordings, by case: refund-angry 0.97,
    // thanks 0.08, login-loop 0.93, receipt 0.52, close-account 0.90,
    // how-to-export 0.88, invoice-vat 0.91; the labels are yes for all but
    // thanks and the receipt. A bar of 0.95 says yes only to refund-angry:
    // right for it, thanks and the receipt, wrong for the other four, 3 of 7.
    let high = write(
        "triage-0.95.jud",
        &replaced(
            &read(TRIAGE),
            "      threshold: 0.55\n",
            "      threshold: 0.95\n",
        ),
    );
    let (gate, question) = gate_of(RECORDINGS, &high, "actionable");
    assert_eq!(
        (gate["acted"].as_u64(), gate["deferred"].as_u64()),
        (Some(7), Some(0))
    );
    assert!(is_ratio(&gate["accuracy_when_acted"], 3, 7), "{gate}");
    assert!(
        gate["accuracy_when_acted"].as_f64() < committed.0["accuracy_when_acted"].as_f64(),
        "a bar that misses four requests is not the better policy: {gate}"
    );
    // The question is the same question over the same answers.
    assert_eq!(model_side(&question), model_side(&committed.1));

    // `--min-accuracy` holds the model's accuracy, 6 of 7, whatever the
    // policy does with it: the policy at 0.95 is right 3 times of 7 and the
    // bar of 0.85 is met, and the policy at 0.55 is right 7 times of 7 and
    // a bar of 0.9 is not.
    let met = eval(&high, TRIAGE_CASES, &["--min-accuracy", "actionable=0.85"]);
    assert_eq!(code(&met), 0, "{}", stderr(&met));
    let unmet = triage(&["--min-accuracy", "actionable=0.9"]);
    assert_eq!(code(&unmet), 3, "{}", stderr(&unmet));
}

#[test]
fn a_strict_gate_changes_the_verdict_at_exactly_its_bar() {
    // close-account's recording says 0.9, labelled true, and a bar of 0.9 is
    // exactly there: met without `strict`, not met with it. The other six
    // verdicts are the same either way (0.97, 0.93, 0.91 yes; 0.08, 0.52,
    // 0.88 no), five right of six, so the policy is 6 of 7 with the bar met
    // at 0.9 and 5 of 7 with it met only above.
    let at_bar = write(
        "triage-at.jud",
        &replaced(
            &read(TRIAGE),
            "      threshold: 0.55\n",
            "      threshold: 0.9\n",
        ),
    );
    let above = write(
        "triage-above.jud",
        &replaced(
            &read(TRIAGE),
            "      threshold: 0.55\n",
            "      threshold: 0.9\n      strict: true\n",
        ),
    );
    let (gate, question) = gate_of(RECORDINGS, &at_bar, "actionable");
    assert!(is_ratio(&gate["accuracy_when_acted"], 6, 7), "{gate}");
    let (strict, strict_question) = gate_of(RECORDINGS, &above, "actionable");
    assert!(is_ratio(&strict["accuracy_when_acted"], 5, 7), "{strict}");
    // The model's reading is not the gate's: strict or not, 6 of 7.
    assert_eq!(model_side(&question), model_side(&strict_question));
    assert_eq!(question["correct"], 6);
}

#[test]
fn a_scores_gate_is_graded_by_the_level_the_policy_reads_and_not_the_most_probable_one() {
    // how-to-export (labelled calm, 0) is edited so that the model's most
    // probable level is still 0 (0.4) while the weighted score,
    // 0.35 + 2 * 0.25 = 0.85, is nearest to level 1, which is what the policy
    // reads. The question's accuracy goes by the first, the gate's by the
    // second.
    let dir = copy_dir(RECORDINGS);
    let file = dir.join("how-to-export.jud");
    let text = read(file.to_str().unwrap());
    let text = replaced(&text, "        score: 0.12\n", "        score: 0.85\n");
    let text = replaced(
        &text,
        "          \"0\": 0.9\n          \"1\": 0.08\n          \"2\": 0.02\n",
        "          \"0\": 0.4\n          \"1\": 0.35\n          \"2\": 0.25\n",
    );
    std::fs::write(&file, text).unwrap();
    let recordings = dir.to_str().unwrap();

    // The committed bar, 0.3, acts on all seven; six carry a tone label and
    // the policy's level is right for five (not for how-to-export).
    let (gate, question) = gate_of(recordings, TRIAGE, "tone");
    assert_eq!(
        (gate["acted"].as_u64(), gate["deferred"].as_u64()),
        (Some(7), Some(0))
    );
    assert!(is_ratio(&gate["accuracy_when_acted"], 5, 6), "{gate}");
    assert_eq!(
        (question["labelled"].as_u64(), question["correct"].as_u64()),
        (Some(6), Some(6))
    );

    // A bar of 0.85 defers refund-angry (0.84), close-account (0.80) and
    // login-loop (0.46), and acts on how-to-export (0.87), invoice-vat (0.89),
    // thanks (0.93) and the receipt (0.96, no tone label): three labelled,
    // of which invoice-vat and thanks read level 0, right, and how-to-export
    // reads level 1, wrong: 2 of 3, the deferred three in neither count.
    let bar = write(
        "triage-tone-0.85.jud",
        &replaced(
            &read(TRIAGE),
            "    tone:\n      confidence: 0.3\n",
            "    tone:\n      confidence: 0.85\n",
        ),
    );
    let (gate, question) = gate_of(recordings, &bar, "tone");
    assert_eq!(
        (gate["acted"].as_u64(), gate["deferred"].as_u64()),
        (Some(4), Some(3))
    );
    assert!(is_ratio(&gate["accuracy_when_acted"], 2, 3), "{gate}");
    assert_eq!(question["correct"], 6);
}

#[test]
fn a_model_named_with_control_characters_is_text_in_the_report_and_verbatim_in_json() {
    // A model's name comes from the server, or from a recording, which is a
    // file someone else may have handed over. In the text report it must not
    // reach the terminal as an escape sequence (colour, a retitled window, a
    // bell); the JSON carries it exactly, escaped as JSON escapes it.
    let model = "jev\u{1b}[31mRED\u{1b}[0m\u{1b}]0;pwned\u{7}";
    let recordings = recordings_naming(r#""jev\x1b[31mRED\x1b[0m\x1b]0;pwned\x07""#, &[]);

    let out = eval_with(&recordings, TRIAGE, TRIAGE_CASES, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        !text.bytes().any(|b| b == 0x1b || b == 0x07),
        "a raw control character reached stdout: {text:?}"
    );
    assert!(
        text.contains(r"7 requests, model jev\u{1b}[31mRED\u{1b}[0m\u{1b}]0;pwned\u{7}"),
        "{text}"
    );

    let out = eval_with(&recordings, TRIAGE, TRIAGE_CASES, &["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(!stdout(&out).bytes().any(|b| b == 0x1b || b == 0x07));
    assert_eq!(report(&out)["models"], json!([model]));
}

#[test]
fn recordings_of_several_models_are_listed_in_the_order_they_were_first_seen() {
    // refund-angry, the first request, is jev-1.13.0; the receipt, the
    // fourth, is another model; the rest are jev-1.13.0 again.
    let recordings = recordings_naming("jev-1.12.0", &["receipt"]);
    let out = eval_with(&recordings, TRIAGE, TRIAGE_CASES, &["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(report(&out)["models"], json!(["jev-1.13.0", "jev-1.12.0"]));
    assert_eq!(report(&out)["requests"], 7);
    let text = stdout(&eval_with(&recordings, TRIAGE, TRIAGE_CASES, &[]));
    assert!(
        text.contains("7 requests, models jev-1.13.0, jev-1.12.0\n"),
        "{text}"
    );
}

#[test]
fn a_report_with_no_misses_says_so() {
    let out = eval(HANDOFF, HANDOFF_CASES, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(
        stdout(&out).ends_with("\nmodel misses: none\n"),
        "{}",
        stdout(&out)
    );
    assert!(!stdout(&out).contains("model misses ("), "{}", stdout(&out));
}

#[test]
fn a_case_without_an_id_is_named_by_its_position() {
    let mut cases = read(TRIAGE_CASES);
    for id in [
        "refund-angry",
        "thanks",
        "login-loop",
        "receipt",
        "close-account",
        "how-to-export",
        "invoice-vat",
    ] {
        cases = replaced(&cases, &format!("- id: {id}\n      state:"), "- state:");
    }
    let cases = write("anonymous-cases.jud", &cases);
    let out = eval(TRIAGE, &cases, &["--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    // The receipt was the fourth case, so `#3`, in both forms of the report.
    let report = report(&out);
    assert_eq!(question(&report, "actionable")["misses"][0]["case"], "#3");
    assert_eq!(question(&report, "desk")["misses"][0]["case"], "#3");
    let text = stdout(&eval(TRIAGE, &cases, &[]));
    assert!(
        text.contains("#3: expected no, predicted yes, confidence 0.52"),
        "{text}"
    );
}

/// `jud ARGS` whose stdout reader goes away at once: the exit status and
/// what it said on stderr.
///
/// Best effort: a child that is slow to start is not yet writing when the
/// pipe closes, and a child that wrote first still ends with the same status,
/// so this can never fail for being early; it fails when a closed reader
/// changes what the command exits with.
fn run_with_stdout_closed(args: &[&str]) -> (Option<i32>, String) {
    let mut child = support::command(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    (output.status.code(), stderr(&output))
}

#[test]
fn a_reader_that_closes_early_does_not_change_the_exit_status() {
    // `jud eval ... | head -c1` in a CI step: status 3 must still say that a
    // bar was not met, and a met bar must not turn into a failure.
    let base = ["eval", TRIAGE, TRIAGE_CASES, "--replay", RECORDINGS];
    for (extra, status) in [
        (vec!["--min-accuracy", "0.99"], Some(3)),
        (vec!["--min-accuracy", "0.99", "--json"], Some(3)),
        (vec!["--min-accuracy", "0.5"], Some(0)),
        (vec![], Some(0)),
    ] {
        let mut args = base.to_vec();
        args.extend(&extra);
        let (code, err) = run_with_stdout_closed(&args);
        assert_eq!(code, status, "{extra:?}: {err}");
        // Nothing but the bar's own line is said, and no panic.
        if status == Some(3) {
            assert!(
                err.starts_with("jud: 2 question(s) below --min-accuracy: "),
                "{extra:?}: {err}"
            );
        } else {
            assert!(err.is_empty(), "{extra:?}: {err}");
        }
    }
}

#[tokio::test]
async fn a_server_that_fails_midway_names_the_case_and_its_place_in_the_run() {
    // Two answers, then a refusal that is not retried: the third case is the
    // one named, and the failure says how far the run got, since eval keeps
    // nothing of the two it paid for.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(triage_answers()))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error": "bad key"})))
        .mount(&server)
        .await;
    let out = jud(
        &["eval", TRIAGE, TRIAGE_CASES],
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &server.uri()),
        ],
    );
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    assert!(
        stderr(&out).starts_with("jud: case login-loop (3 of 7): the backend at "),
        "{}",
        stderr(&out)
    );
    assert!(stdout(&out).is_empty());
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[test]
fn the_help_says_which_accuracy_a_bar_holds_and_that_a_server_run_keeps_nothing() {
    let out = jud(&["eval", "--help"], "", &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    // clap wraps lines; the sentences are what is asserted.
    let help = stdout(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        help.contains("It is the model's accuracy per question, not the policy's."),
        "{help}"
    );
    assert!(
        help.contains("Against a server eval keeps nothing: use `jud record` to keep the answers."),
        "{help}"
    );
}
