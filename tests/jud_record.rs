//! `jud record RUBRIC CASES --out DIR`, run as a subprocess against a wiremock
//! server standing in for any System One server: no key that works, no
//! network. The mock answers every triage request with the same valid
//! answers (accuracy is not what is under test), so what these tests hold
//! the command to is what it does with them: how many calls it makes, which
//! files it writes, what it keeps, how it stops, and what it refuses before
//! the first call. Decision 0021 is the contract.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

mod support;

use std::path::Path;
use std::process::Output;

use judgment::jud::Cases;
use serde_json::{Value, json};
use support::{
    HANDOFF, HANDOFF_CASES, TRIAGE, TRIAGE_CASES, code, copy_dir, jud, scratch, stderr, stdout,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The seven case ids of `examples/jud/triage-cases.jud`, in document order,
/// which are also the file names a recording takes.
const TRIAGE_IDS: [&str; 7] = [
    "refund-angry",
    "thanks",
    "login-loop",
    "receipt",
    "close-account",
    "how-to-export",
    "invoice-vat",
];

/// The triage rubric's three answers, as a server sends them.
fn triage_answers() -> Value {
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

/// The handoff rubric's one answer.
fn handoff_answers() -> Value {
    json!({
        "model": "jev-1.13.0",
        "answers": { "wants_human": { "type": "noul", "noul": 0.9 } },
        "usage": { "input_tokens": 100, "output_tokens": 10 }
    })
}

/// A server that answers `calls` requests with `body` and fails the test, when
/// it is dropped, if it was asked any other number of times.
async fn server(body: Value, calls: u64) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(calls)
        .mount(&server)
        .await;
    server
}

/// `jud record RUBRIC CASES --out OUT [EXTRA]` against `server`.
fn record(server: &MockServer, rubric: &str, cases: &str, out: &Path, extra: &[&str]) -> Output {
    let uri = server.uri();
    let mut args = vec!["record", rubric, cases, "--out", out.to_str().unwrap()];
    args.extend_from_slice(extra);
    jud(
        &args,
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &uri),
        ],
    )
}

/// The names of the files in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The names `jud record` gives the triage recordings, sorted.
fn triage_files() -> Vec<String> {
    let mut files: Vec<String> = TRIAGE_IDS.iter().map(|id| format!("{id}.jud")).collect();
    files.sort();
    files
}

/// `dir`'s recordings, by file name and content, to compare before and after.
fn contents(dir: &Path) -> Vec<(String, String)> {
    names(dir)
        .into_iter()
        .map(|n| {
            let text = std::fs::read_to_string(dir.join(&n)).unwrap();
            (n, text)
        })
        .collect()
}

/// The triage cases document with the `id:` line of the named cases removed,
/// written to a scratch file.
fn triage_cases_without_ids(ids: &[&str]) -> String {
    let mut text = std::fs::read_to_string(TRIAGE_CASES).unwrap();
    for id in ids {
        let with = format!("- id: {id}\n      state:");
        assert!(text.contains(&with), "{id}");
        text = text.replace(&with, "- state:");
    }
    let file = scratch("cases").join("cases.jud");
    std::fs::write(&file, text).unwrap();
    file.to_str().unwrap().to_owned()
}

/// A Cases document written to a scratch file.
fn cases_file(text: &str) -> String {
    let file = scratch("cases").join("cases.jud");
    std::fs::write(&file, text).unwrap();
    file.to_str().unwrap().to_owned()
}

#[tokio::test]
async fn the_first_run_asks_every_case_once_and_writes_one_recording_each() {
    let server = server(triage_answers(), 7).await;
    let out = scratch("record").join("recordings");
    let run = record(&server, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));

    // The result is the files; stdout carries nothing.
    assert!(stdout(&run).is_empty(), "{}", stdout(&run));
    // One file per case, named after it, and nothing else in the directory:
    // no temporary file survives a successful run.
    assert_eq!(names(&out), triage_files());

    // Progress, one line per case, and the tally.
    let err = stderr(&run);
    assert!(err.contains("recorded refund-angry (1/7, "), "{err}");
    assert!(err.contains("recorded invoice-vat (7/7, "), "{err}");
    assert!(err.contains(" ms)"), "{err}");
    assert!(
        err.contains(&format!("recorded 7, kept 0 in {}", out.display())),
        "{err}"
    );

    // What a recording says: the case, the rubric, the server, the time, the
    // fingerprint of the request, under the comment the examples carry.
    let text = std::fs::read_to_string(out.join("refund-angry.jud")).unwrap();
    assert!(
        text.starts_with("# Recorded answer for case `refund-angry`; found by fingerprint.\n"),
        "{text}"
    );
    for wanted in [
        "apiVersion: jud/v1.3",
        "kind: Recording",
        "name: refund-angry",
        "rubric: inbox-triage",
        &format!("server: {}", server.uri()),
        "fingerprint: sha256:",
        "recorded_at: ",
        "elapsed_ms: ",
        "model: jev-1.13.0",
    ] {
        assert!(text.contains(wanted), "{wanted} in\n{text}");
    }
    assert!(
        !text.contains("request_hash"),
        "a CLI recording is found by fingerprint alone:\n{text}"
    );
}

#[tokio::test]
async fn jud_check_verifies_what_was_written_and_the_fingerprints_match() {
    let server = server(triage_answers(), 7).await;
    let out = scratch("record");
    let run = record(&server, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));

    let files: Vec<String> = names(&out)
        .iter()
        .map(|n| out.join(n).to_str().unwrap().to_owned())
        .collect();
    let mut args = vec!["check", TRIAGE, TRIAGE_CASES];
    args.extend(files.iter().map(String::as_str));
    let check = jud(&args, "", &[]);
    let report = stdout(&check);
    assert_eq!(code(&check), 0, "{report}{}", stderr(&check));
    assert_eq!(report.matches("fingerprint matches").count(), 7, "{report}");
    assert_eq!(report.matches("verified").count(), 7, "{report}");
    assert!(!report.contains("differs"), "{report}");
    assert!(
        report.trim_end().ends_with("9 documents, 0 refused"),
        "{report}"
    );
}

#[tokio::test]
async fn a_second_run_asks_nothing_and_writes_nothing() {
    let first = server(triage_answers(), 7).await;
    let out = scratch("record");
    assert_eq!(code(&record(&first, TRIAGE, TRIAGE_CASES, &out, &[])), 0);
    let before = contents(&out);

    // A fresh server that expects no call at all: a request would fail it.
    let second = server(triage_answers(), 0).await;
    let run = record(&second, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));
    assert!(stdout(&run).is_empty());
    let err = stderr(&run);
    assert!(err.contains("kept refund-angry"), "{err}");
    assert!(err.contains("kept invoice-vat"), "{err}");
    assert!(!err.contains("recorded refund-angry"), "{err}");
    assert!(err.contains("recorded 0, kept 7 in"), "{err}");
    // Not a byte moved, so not a timestamp either.
    assert_eq!(contents(&out), before);
}

#[tokio::test]
async fn a_deleted_recording_costs_exactly_one_call() {
    let first = server(triage_answers(), 7).await;
    let out = scratch("record");
    assert_eq!(code(&record(&first, TRIAGE, TRIAGE_CASES, &out, &[])), 0);
    let before = contents(&out);
    std::fs::remove_file(out.join("receipt.jud")).unwrap();

    let second = server(triage_answers(), 1).await;
    let run = record(&second, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));
    let err = stderr(&run);
    assert!(err.contains("recorded receipt (4/7, "), "{err}");
    assert!(err.contains("kept thanks"), "{err}");
    assert!(err.contains("recorded 1, kept 6 in"), "{err}");
    assert_eq!(names(&out), triage_files());
    // The six that were there are untouched; the seventh is back.
    let after = contents(&out);
    for (name, text) in &before {
        if name != "receipt.jud" {
            assert!(after.contains(&(name.clone(), text.clone())), "{name}");
        }
    }
}

#[tokio::test]
async fn refresh_asks_every_case_again_and_replaces_what_was_there() {
    let first = server(triage_answers(), 7).await;
    let out = scratch("record");
    assert_eq!(code(&record(&first, TRIAGE, TRIAGE_CASES, &out, &[])), 0);

    // The second answers differ, so a replaced file is visible as one.
    let mut again = triage_answers();
    again["answers"]["actionable"]["noul"] = json!(0.33);
    let second = server(again, 7).await;
    let run = record(&second, TRIAGE, TRIAGE_CASES, &out, &["--refresh"]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));
    let err = stderr(&run);
    assert!(err.contains("recorded 7, kept 0 in"), "{err}");
    assert!(!err.contains("kept refund-angry"), "{err}");
    assert_eq!(names(&out), triage_files());
    for id in TRIAGE_IDS {
        let text = std::fs::read_to_string(out.join(format!("{id}.jud"))).unwrap();
        assert!(text.contains("noul: 0.33"), "{id}:\n{text}");
    }
}

#[tokio::test]
async fn the_recordings_replay_for_the_states_they_answer() {
    let server = server(triage_answers(), 7).await;
    let out = scratch("record");
    assert_eq!(code(&record(&server, TRIAGE, TRIAGE_CASES, &out, &[])), 0);

    // The first case's state, as JSON, through the rubric from the recordings.
    let cases = Cases::parse(&std::fs::read_to_string(TRIAGE_CASES).unwrap()).unwrap();
    let state = serde_json::to_string(&cases.cases[0].state).unwrap();
    let replayed = jud(&["--replay", out.to_str().unwrap(), TRIAGE], &state, &[]);
    assert_eq!(code(&replayed), 0, "{}", stderr(&replayed));
    // The verdicts the mock's answers produce through the committed policy.
    let verdicts: Value = serde_json::from_str(&stdout(&replayed)).unwrap();
    assert_eq!(verdicts["actionable"]["verdict"], "yes");
    assert_eq!(verdicts["actionable"]["probability"], 0.91);
    assert_eq!(verdicts["desk"]["key"], "billing");
    assert_eq!(verdicts["tone"]["label"], "angry");

    // A state nobody recorded has no answer: record asked what the cases hold.
    let other = jud(
        &["--replay", out.to_str().unwrap(), TRIAGE],
        r#"{"message": "something else"}"#,
        &[],
    );
    assert_eq!(code(&other), 1, "{}", stderr(&other));
}

#[tokio::test]
async fn a_conversation_is_recorded_turn_by_turn() {
    // Two conversations of three turns each: six requests, one per prefix.
    let server = server(handoff_answers(), 6).await;
    let out = scratch("record");
    let run = record(&server, HANDOFF, HANDOFF_CASES, &out, &[]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));
    assert_eq!(
        names(&out),
        [
            "escalates-turn-0.jud",
            "escalates-turn-1.jud",
            "escalates-turn-2.jud",
            "patient-turn-0.jud",
            "patient-turn-1.jud",
            "patient-turn-2.jud",
        ]
    );
    let err = stderr(&run);
    assert!(err.contains("recorded escalates-turn-0 (1/6, "), "{err}");
    assert!(err.contains("recorded patient-turn-2 (6/6, "), "{err}");

    // Each prefix is a request of its own, so each has its own fingerprint.
    let mut fingerprints: Vec<String> = contents(&out)
        .iter()
        .map(|(_, text)| {
            text.lines()
                .find_map(|l| l.trim().strip_prefix("fingerprint: "))
                .unwrap()
                .to_owned()
        })
        .collect();
    fingerprints.sort();
    fingerprints.dedup();
    assert_eq!(fingerprints.len(), 6);

    // jud check finds each recording's turn by name and the fingerprints match.
    let mut args = vec!["check", HANDOFF, HANDOFF_CASES];
    let files: Vec<String> = names(&out)
        .iter()
        .map(|n| out.join(n).to_str().unwrap().to_owned())
        .collect();
    args.extend(files.iter().map(String::as_str));
    let check = jud(&args, "", &[]);
    let report = stdout(&check);
    assert_eq!(code(&check), 0, "{report}{}", stderr(&check));
    assert_eq!(report.matches("fingerprint matches").count(), 6, "{report}");
    assert!(
        report.trim_end().ends_with("8 documents, 0 refused"),
        "{report}"
    );
}

#[tokio::test]
async fn a_failing_call_stops_the_run_and_keeps_what_was_recorded() {
    let server = MockServer::start().await;
    // Two answers, then the key is refused. A 401 is not retried.
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(triage_answers()))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(401).set_body_json(
            json!({ "error": { "message": "bad key", "type": "authentication_error" } }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let out = scratch("record");
    let run = record(&server, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&run), 1, "{}", stderr(&run));
    assert!(stdout(&run).is_empty());
    let err = stderr(&run);
    // It names the case it stopped at, and how far it had got; the progress
    // before it is bare, the failure is the one `jud: ` line.
    assert_eq!(err.matches("jud: ").count(), 1, "{err}");
    let last = err.lines().last().unwrap();
    assert!(last.starts_with("jud: stopped at case login-loop"), "{err}");
    assert!(err.contains("login-loop"), "{err}");
    assert!(err.contains("3/7"), "{err}");
    assert!(err.contains("2 recorded and 0 kept"), "{err}");
    assert!(err.contains("the backend at"), "{err}");
    // The two answers already paid for are on disk, and nothing else is.
    assert_eq!(names(&out), ["refund-angry.jud", "thanks.jud"]);

    // The next run, with a key that works, resumes: five calls, two kept.
    let working = self::server(triage_answers(), 5).await;
    let resumed = record(&working, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&resumed), 0, "{}", stderr(&resumed));
    assert!(
        stderr(&resumed).contains("recorded 5, kept 2 in"),
        "{}",
        stderr(&resumed)
    );
    assert_eq!(names(&out), triage_files());
}

#[tokio::test]
async fn an_answer_that_does_not_fit_is_never_written() {
    // The desk is a Choice; "legal" is not one of the options asked.
    let mut body = triage_answers();
    body["answers"]["desk"]["choice"] = json!("legal");
    body["answers"]["desk"]["probabilities"] = json!({ "legal": 1.0 });
    let server = server(body, 1).await;
    let out = scratch("record");
    let run = record(&server, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&run), 1, "{}", stderr(&run));
    let err = stderr(&run);
    assert!(err.contains("refund-angry"), "{err}");
    assert!(err.contains("legal"), "an off-list option is named: {err}");
    assert!(err.contains("0 recorded"), "{err}");
    assert!(names(&out).is_empty(), "{:?}", names(&out));
}

#[tokio::test]
async fn a_case_without_an_id_is_refused_before_any_call() {
    // Two of the seven lose their id: a recording is a file named after its
    // case, and `#1` is a position.
    let cases = triage_cases_without_ids(&["thanks", "receipt"]);
    let server = server(triage_answers(), 0).await;
    let out = scratch("record").join("recordings");
    let run = record(&server, TRIAGE, &cases, &out, &[]);
    assert_eq!(code(&run), 2, "{}", stderr(&run));
    assert!(stdout(&run).is_empty());
    let err = stderr(&run);
    // Every offender is listed at once, with the file and the remedy.
    assert!(err.starts_with("jud: "), "{err}");
    assert!(err.contains(&cases), "{err}");
    assert!(err.contains("#1") && err.contains("#3"), "{err}");
    assert!(err.contains("give the case an id"), "{err}");
    // Nothing was made: not even the directory.
    assert!(!out.exists());
}

#[tokio::test]
async fn two_requests_that_would_share_a_file_are_refused_before_any_call() {
    // `talk` is a conversation recorded as talk-turn-0 and talk-turn-1; a
    // case with the id `talk-turn-0` would be written over its first turn.
    let cases = cases_file(
        "apiVersion: jud/v1.3
kind: Cases
metadata:
  name: clash
spec:
  rubric: handoff
  cases:
    - id: talk
      state:
        - {role: user, text: Hello}
        - {role: user, text: A person please}
      expect:
        wants_human: {from_turn: 1}
    - id: talk-turn-0
      state:
        - {role: user, text: Something else}
      expect:
        wants_human: false
",
    );
    let server = server(handoff_answers(), 0).await;
    let out = scratch("record").join("recordings");
    let run = record(&server, HANDOFF, &cases, &out, &[]);
    assert_eq!(code(&run), 2, "{}", stderr(&run));
    let err = stderr(&run);
    assert!(err.contains("talk-turn-0.jud"), "{err}");
    assert!(err.contains(&cases), "{err}");
    assert!(!out.exists());
}

#[tokio::test]
async fn a_turn_that_does_not_lower_is_refused_before_the_first_call() {
    // The question's only instruction part is sent from the second turn on.
    // The whole conversation lowers, so the cases bind; its first turn, one
    // request on its own, would send a Noul that asks nothing. The case
    // before it is fine: asking it first would spend a call on a run that
    // cannot finish.
    let dir = scratch("late");
    let rubric = dir.join("rubric.jud");
    std::fs::write(
        &rubric,
        "apiVersion: jud/v1.3
kind: Rubric
metadata:
  name: late-handoff
spec:
  questions:
    wants_human:
      type: noul
      instructions:
        ask: Does the user want a person to take over?
      part_when:
        ask: \"1\"
",
    )
    .unwrap();
    let cases = cases_file(
        "apiVersion: jud/v1.3
kind: Cases
metadata:
  name: late-handoff-cases
spec:
  rubric: late-handoff
  cases:
    - id: whole
      state:
        - {role: user, text: Hello}
        - {role: user, text: Hello again}
      expect:
        wants_human: false
    - id: talk
      state:
        - {role: user, text: Hello}
        - {role: user, text: A person please}
      expect:
        wants_human: {from_turn: 1}
",
    );
    let server = server(handoff_answers(), 0).await;
    let out = dir.join("recordings");
    let run = record(&server, rubric.to_str().unwrap(), &cases, &out, &[]);
    assert_eq!(code(&run), 2, "{}", stderr(&run));
    assert!(
        stderr(&run).contains("does not lower for case talk-turn-0"),
        "{}",
        stderr(&run)
    );
    assert!(!out.exists());
}

#[tokio::test]
async fn two_cases_with_one_request_share_one_recording() {
    // Same state, same questions: the same fingerprint, so the answer
    // recorded for `first` is the answer for `second`, and asking again
    // would pay twice for it.
    let cases = cases_file(
        "apiVersion: jud/v1.3
kind: Cases
metadata:
  name: twins
spec:
  rubric: inbox-triage
  cases:
    - id: first
      state: {message: Please refund the duplicate charge.}
      expect: {desk: billing}
    - id: second
      state: {message: Please refund the duplicate charge.}
      expect: {desk: billing}
",
    );
    let server = server(triage_answers(), 1).await;
    let out = scratch("record");
    let run = record(&server, TRIAGE, &cases, &out, &[]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));
    let err = stderr(&run);
    assert!(err.contains("recorded first (1/2, "), "{err}");
    assert!(err.contains("kept second"), "{err}");
    assert!(err.contains("recorded 1, kept 1 in"), "{err}");
    assert_eq!(names(&out), ["first.jud"]);

    // --refresh asks every case again; the twins are still one request.
    let again = self::server(triage_answers(), 1).await;
    let refreshed = record(&again, TRIAGE, &cases, &out, &["--refresh"]);
    assert_eq!(code(&refreshed), 0, "{}", stderr(&refreshed));
    assert!(
        stderr(&refreshed).contains("recorded 1, kept 1 in"),
        "{}",
        stderr(&refreshed)
    );
}

#[tokio::test]
async fn a_recording_that_no_longer_verifies_is_asked_again_and_replaced() {
    let first = server(triage_answers(), 7).await;
    let out = scratch("record");
    assert_eq!(code(&record(&first, TRIAGE, TRIAGE_CASES, &out, &[])), 0);
    // Edited by hand: the desk now chose an option the request never offered.
    let file = out.join("refund-angry.jud");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("choice: billing"));
    std::fs::write(&file, text.replace("choice: billing", "choice: legal")).unwrap();
    let untouched = contents(&out)
        .into_iter()
        .filter(|(n, _)| n != "refund-angry.jud")
        .collect::<Vec<_>>();

    let second = server(triage_answers(), 1).await;
    let run = record(&second, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));
    let err = stderr(&run);
    assert!(err.contains("replaced refund-angry (stale)"), "{err}");
    assert!(err.contains("recorded 1, kept 6 in"), "{err}");
    let fixed = std::fs::read_to_string(&file).unwrap();
    assert!(fixed.contains("choice: billing"), "{fixed}");
    let after: Vec<_> = contents(&out)
        .into_iter()
        .filter(|(n, _)| n != "refund-angry.jud")
        .collect();
    assert_eq!(after, untouched);
}

#[tokio::test]
async fn a_directory_with_a_file_the_reader_refuses_is_a_usage_error_unless_refreshed() {
    // A Rubric where a recording should be: whether the request is recorded
    // is unknown, so no call is made on a guess.
    let out = scratch("record");
    std::fs::copy(TRIAGE, out.join("not-a-recording.jud")).unwrap();
    let none = server(triage_answers(), 0).await;
    let run = record(&none, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&run), 2, "{}", stderr(&run));
    let err = stderr(&run);
    assert!(err.contains("not-a-recording.jud"), "{err}");
    assert!(err.contains("--refresh"), "{err}");
    assert_eq!(names(&out), ["not-a-recording.jud"]);

    // --refresh asks everything again and does not read what is there.
    let all = server(triage_answers(), 7).await;
    let refreshed = record(&all, TRIAGE, TRIAGE_CASES, &out, &["--refresh"]);
    assert_eq!(code(&refreshed), 0, "{}", stderr(&refreshed));
    assert_eq!(names(&out).len(), 8);
}

#[tokio::test]
async fn an_output_path_that_is_a_file_is_refused_before_any_call() {
    let dir = scratch("record");
    let out = dir.join("a-file");
    std::fs::write(&out, "not a directory").unwrap();
    let server = server(triage_answers(), 0).await;
    let run = record(&server, TRIAGE, TRIAGE_CASES, &out, &[]);
    assert_eq!(code(&run), 2, "{}", stderr(&run));
    assert!(stderr(&run).contains("a-file"), "{}", stderr(&run));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "not a directory");
}

#[tokio::test]
async fn the_output_directory_is_created_with_its_parents() {
    let server = server(handoff_answers(), 6).await;
    let out = scratch("record").join("deep").join("er").join("recordings");
    let run = record(&server, HANDOFF, HANDOFF_CASES, &out, &[]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));
    assert_eq!(names(&out).len(), 6);
}

#[tokio::test]
async fn record_never_replays_it_asks_the_configured_server() {
    // JUD_REPLAY points `jud RUBRIC` at recordings; `record` exists to make
    // them, so it neither reads the variable nor takes a --replay flag. The
    // directory the variable names answers every triage case, and the
    // server is asked for all seven all the same.
    let server = server(triage_answers(), 7).await;
    let out = scratch("record");
    let uri = server.uri();
    let run = jud(
        &[
            "record",
            TRIAGE,
            TRIAGE_CASES,
            "--out",
            out.to_str().unwrap(),
        ],
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &uri),
            ("JUD_REPLAY", support::RECORDINGS),
        ],
    );
    assert_eq!(code(&run), 0, "{}", stderr(&run));
    assert!(
        stderr(&run).contains("recorded 7, kept 0 in"),
        "{}",
        stderr(&run)
    );

    let flag = jud(
        &[
            "record",
            TRIAGE,
            TRIAGE_CASES,
            "--out",
            out.to_str().unwrap(),
            "--replay",
            support::RECORDINGS,
        ],
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &uri),
        ],
    );
    assert_eq!(code(&flag), 2, "{}", stderr(&flag));
    assert!(stderr(&flag).contains("--replay"), "{}", stderr(&flag));
}

#[test]
fn a_missing_out_is_a_usage_error() {
    let out = jud(&["record", TRIAGE, TRIAGE_CASES], "", &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(stderr(&out).contains("--out"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty());
}

#[test]
fn no_key_and_no_server_is_refused_before_any_call() {
    let dir = scratch("record").join("recordings");
    let out = jud(
        &[
            "record",
            TRIAGE,
            TRIAGE_CASES,
            "--out",
            dir.to_str().unwrap(),
        ],
        "",
        &[],
    );
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(stderr(&out).contains("no API key"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty());
    assert!(!dir.exists(), "a refusal makes nothing");
}

#[test]
fn cases_that_do_not_fit_the_rubric_are_refused_by_file_before_any_call() {
    // The handoff rubric asks nothing the triage cases label.
    let dir = scratch("record").join("recordings");
    let out = jud(
        &[
            "record",
            HANDOFF,
            TRIAGE_CASES,
            "--out",
            dir.to_str().unwrap(),
        ],
        "",
        &[("TYPESAFE_API_KEY", "test-key")],
    );
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(stderr(&out).contains(TRIAGE_CASES), "{}", stderr(&out));
    assert!(!dir.exists());

    let missing = jud(
        &[
            "record",
            TRIAGE,
            "/nonexistent/cases.jud",
            "--out",
            dir.to_str().unwrap(),
        ],
        "",
        &[("TYPESAFE_API_KEY", "test-key")],
    );
    assert_eq!(code(&missing), 2, "{}", stderr(&missing));
    assert!(
        stderr(&missing).contains("/nonexistent/cases.jud"),
        "{}",
        stderr(&missing)
    );
}

#[tokio::test]
async fn recordings_already_in_the_directory_are_found_by_fingerprint_not_by_name() {
    // The committed calibration recordings answer the triage cases; a copy of
    // them is "recorded" already, whatever a file is called.
    let dir = copy_dir(support::RECORDINGS);
    let before = names(&dir);
    let server = server(triage_answers(), 0).await;
    let run = record(&server, TRIAGE, TRIAGE_CASES, &dir, &[]);
    assert_eq!(code(&run), 0, "{}", stderr(&run));
    assert!(
        stderr(&run).contains("recorded 0, kept 7 in"),
        "{}",
        stderr(&run)
    );
    assert_eq!(names(&dir), before);

    // The same holds turn by turn: the committed conversation recordings are
    // the requests this command lowers for each prefix.
    let turns = record(&server, HANDOFF, HANDOFF_CASES, &dir, &[]);
    assert_eq!(code(&turns), 0, "{}", stderr(&turns));
    assert!(
        stderr(&turns).contains("recorded 0, kept 6 in"),
        "{}",
        stderr(&turns)
    );
    assert_eq!(names(&dir), before);
}
