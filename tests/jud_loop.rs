//! `jud record`, `jud eval` and `jud tune` together, and the command line's
//! guidance (decision 0021).
//!
//! The other `jud_*` suites hold each command to its own contract against
//! recordings that were written by hand or by an example. What nobody checks
//! there is the loop itself: that the files `jud record` writes are the ones
//! `jud eval` and `jud tune` read, and that a case's own options (a Choice
//! with `options_from: request`) and the questions a `when` leaves out are
//! recorded, graded and tuned as the ADR says. These tests run the three
//! commands one after the other, `record` against a wiremock server that
//! answers whatever it is asked and the others with no key and no network.
//!
//! The command's own guidance, which belongs to no one subcommand (what
//! `jud --replay DIR` with no rubric says, where a misplaced flag is pointed,
//! what the top-level help promises), and the helpers of `tests/support` the
//! subprocess tests rely on, are checked at the end.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

mod support;

use std::path::Path;
use std::process::{Output, Stdio};

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use support::{
    RECORDINGS, ROOT, TRIAGE, TRIAGE_CASES, code, command, jud, scratch, server, stderr, stdout,
    triage_answers,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const ROUTING: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/routing.jud");
const ROUTING_CASES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/jud/routing-cases.jud"
);

// ---- a server that answers whatever it is asked ----------------------------

/// What a request carries that these tests look at: the questions, in the
/// order they were sent (a sorted map would hide the order the model sees).
#[derive(Deserialize)]
struct Sent {
    questions: IndexMap<String, SentQuestion>,
}

#[derive(Deserialize)]
struct SentQuestion {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    instructions: Value,
    #[serde(default)]
    criteria: Option<Criteria>,
}

/// A Choice's options keyed by name, or a Score's levels.
#[derive(Deserialize)]
#[serde(untagged)]
enum Criteria {
    Options(IndexMap<String, String>),
    Levels(Vec<String>),
}

impl Sent {
    /// The ids of the questions asked, in the order sent.
    fn ids(&self) -> Vec<&str> {
        self.questions.keys().map(String::as_str).collect()
    }

    /// The option keys offered for a Choice, in the order sent.
    fn options(&self, id: &str) -> Vec<&str> {
        match &self.questions[id].criteria {
            Some(Criteria::Options(options)) => options.keys().map(String::as_str).collect(),
            _ => panic!("{id} is not a Choice"),
        }
    }
}

/// The valid answer to one question as it was put: a Noul at one half, a
/// Choice naming the first option it offers with all the probability and a
/// confidence of 0.8, a Score at level 0 with the same confidence. Whatever
/// the rubric asked, the answer fits it, so what is under test is what the
/// commands do with the request and the recording and not the model.
fn answer_to(id: &str, question: &SentQuestion) -> Value {
    match (question.kind.as_str(), &question.criteria) {
        ("noul", _) => json!({ "type": "noul", "noul": 0.5 }),
        ("choice", Some(Criteria::Options(options))) => {
            let first = options.keys().next().unwrap();
            let probabilities: Map<String, Value> = options
                .keys()
                .map(|option| (option.clone(), json!(f64::from(option == first))))
                .collect();
            json!({ "type": "choice", "choice": first, "probabilities": probabilities,
                    "confidence": 0.8 })
        }
        ("score", Some(Criteria::Levels(levels))) => {
            let legend: Map<String, Value> = levels
                .iter()
                .enumerate()
                .map(|(level, name)| (level.to_string(), json!(name)))
                .collect();
            let probabilities: Map<String, Value> = (0..levels.len())
                .map(|level| (level.to_string(), json!(f64::from(level == 0))))
                .collect();
            json!({ "type": "score", "score": 0, "legend": legend,
                    "probabilities": probabilities, "confidence": 0.8 })
        }
        _ => panic!(
            "cannot answer {id}: a {} the test does not know",
            question.kind
        ),
    }
}

/// Answer every question of the request.
fn answering(request: &Request) -> ResponseTemplate {
    let sent: Sent = request.body_json().unwrap();
    let answers: Map<String, Value> = sent
        .questions
        .iter()
        .map(|(id, question)| (id.clone(), answer_to(id, question)))
        .collect();
    ResponseTemplate::new(200).set_body_json(json!({
        "model": "jev-1.13.0",
        "answers": answers,
        "usage": { "input_tokens": 10, "output_tokens": 1 },
    }))
}

/// A server that answers whatever it is asked, and is asked `calls` times.
async fn answering_server(calls: u64) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(answering)
        .expect(calls)
        .mount(&server)
        .await;
    server
}

/// The requests the server received, in the order they came.
async fn received(server: &MockServer) -> Vec<Sent> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| request.body_json().unwrap())
        .collect()
}

// ---- running the commands ----------------------------------------------------

/// `jud record RUBRIC CASES --out OUT` against `server`.
fn record(server: &MockServer, rubric: &str, cases: &str, out: &Path) -> Output {
    let uri = server.uri();
    jud(
        &["record", rubric, cases, "--out", out.to_str().unwrap()],
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &uri),
        ],
    )
}

/// `jud eval RUBRIC CASES --replay DIR --json`, with no key: the recordings
/// answer. The report parsed, the run having exited 0.
fn eval_json(rubric: &str, cases: &str, recordings: &Path) -> Value {
    let out = jud(
        &[
            "eval",
            rubric,
            cases,
            "--replay",
            recordings.to_str().unwrap(),
            "--json",
        ],
        "",
        &[],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    serde_json::from_str(&stdout(&out)).unwrap_or_else(|e| panic!("{e}: {}", stdout(&out)))
}

/// The recordings in `dir`, as paths, sorted by name.
fn recordings_in(dir: &Path) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path().to_string_lossy().into_owned())
        .collect();
    files.sort();
    files
}

/// The names of the files in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    recordings_in(dir)
        .iter()
        .map(|file| {
            Path::new(file)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

/// `jud check RUBRIC CASES RECORDINGS...`: the recordings verified against
/// the requests the cases lower to. Every document must be accepted and every
/// recording's fingerprint must match its request; the report is returned.
fn checked(rubric: &str, cases: &str, recordings: &Path) -> String {
    let files = recordings_in(recordings);
    let mut args = vec!["check", rubric, cases];
    args.extend(files.iter().map(String::as_str));
    let out = jud(&args, "", &[]);
    assert_eq!(code(&out), 0, "{}{}", stdout(&out), stderr(&out));
    let report = stdout(&out);
    assert_eq!(
        report.matches("fingerprint matches").count(),
        files.len(),
        "{report}"
    );
    assert!(
        report
            .trim_end()
            .ends_with(&format!("{} documents, 0 refused", files.len() + 2)),
        "{report}"
    );
    report
}

fn question<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["id"] == id)
        .unwrap_or_else(|| panic!("no question {id} in {report}"))
}

/// How many requests the gate of `id` was asked about, acting or not: the
/// requests that asked the question, which is the number a question left out
/// by `when` must not be counted in.
fn asked(report: &Value, id: &str) -> u64 {
    let gate = &question(report, id)["gate"];
    gate["acted"].as_u64().unwrap() + gate["deferred"].as_u64().unwrap()
}

// ---- the loop ---------------------------------------------------------------

/// What the three routing cases sent. The case's own options come first, in
/// the order the case wrote them, then the rubric's static one; a question
/// whose `when` does not hold is not sent, and an instruction part whose
/// `part_when` does not hold is left out of the one that is.
fn routing_requests(sent: &[Sent]) {
    assert_eq!(sent.len(), 3);
    assert_eq!(sent[0].ids(), ["desk", "tone", "refund_request"]);
    assert_eq!(sent[1].ids(), ["desk", "tone"]);
    assert_eq!(sent[2].ids(), ["desk", "tone", "duplicate_of"]);
    assert_eq!(
        sent[0].options("desk"),
        ["billing", "technical", "none_of_these"]
    );
    assert_eq!(sent[2].options("duplicate_of"), ["T-4821", "none"]);
    let account = |request: &Sent| {
        request.questions["desk"]
            .instructions
            .get("account")
            .is_some()
    };
    assert!(account(&sent[0]), "the case has a customer.account");
    assert!(
        !account(&sent[1]),
        "the case has none, so the part is not sent"
    );
}

/// What eval makes of the recordings of the three routing cases, with no
/// key. Every request asked the desk and the tone; the open ticket was asked
/// of one case and the refund of another, and a case that did not ask a
/// question counts for neither side of its gate. The server's answer was the
/// first option of each Choice and level 0 of the Score, so what is right
/// follows from the labels.
fn routing_report(report: &Value) {
    assert_eq!(report["requests"], 3);
    assert_eq!(report["models"], json!(["jev-1.13.0"]));
    assert_eq!(asked(report, "desk"), 3);
    assert_eq!(asked(report, "tone"), 3);
    assert_eq!(asked(report, "duplicate_of"), 1);
    assert_eq!(asked(report, "refund_request"), 1);
    // Billing was the first desk offered: right for the billing case, wrong
    // for the technical one. T-4821 was the first ticket offered, and the
    // case is about it. Level 0 is calm: right for one of three.
    for (id, labelled, correct) in [("desk", 2, 1), ("duplicate_of", 1, 1), ("tone", 3, 1)] {
        assert_eq!(question(report, id)["labelled"], labelled, "{id}");
        assert_eq!(question(report, id)["correct"], correct, "{id}");
    }
}

#[tokio::test]
async fn a_rubric_whose_options_come_with_the_case_goes_round_the_loop() {
    // routing.jud has a Choice with `options_from: request` (the desks staffed
    // now, which only a case can say), a `part_when` and two `when`s: the
    // rubric that `jud RUBRIC` cannot lower and `jud record` can.
    let server = answering_server(3).await;
    let recordings = scratch("loop").join("recordings");

    let out = record(&server, ROUTING, ROUTING_CASES, &recordings);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(
        stderr(&out).contains("recorded 3, kept 0 in "),
        "{}",
        stderr(&out)
    );
    assert_eq!(
        names(&recordings),
        ["double-charge.jud", "export-question.jud", "follow-up.jud"]
    );

    routing_requests(&received(&server).await);

    // What `record` wrote is what the reader of the crate accepts, against the
    // requests the cases lower to.
    checked(ROUTING, ROUTING_CASES, &recordings);

    routing_report(&eval_json(ROUTING, ROUTING_CASES, &recordings));

    // tune reads the same files. The sweeps run over the cases that asked.
    let tune = jud(
        &[
            "tune",
            ROUTING,
            ROUTING_CASES,
            "--replay",
            recordings.to_str().unwrap(),
        ],
        "",
        &[],
    );
    assert_eq!(code(&tune), 0, "{}", stderr(&tune));
    let err = stderr(&tune);
    assert!(
        err.starts_with("jud: tuning support-routing on 3 answers to 3 cases"),
        "{err}"
    );
    assert!(
        err.contains("jud: duplicate_of (choice): 1 labelled cases"),
        "{err}"
    );
    assert!(
        err.contains("jud: desk (choice): 2 labelled cases"),
        "{err}"
    );
    let proposal: Value = serde_saphyr::from_str(&stdout(&tune)).unwrap();
    let gates: Vec<&str> = proposal["policy"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    // A parsed `Value` sorts its keys; every gate of the rubric is there,
    // the ones nothing was proposed for as they were written.
    assert_eq!(gates, ["desk", "duplicate_of", "refund_request", "tone"]);
    assert_eq!(proposal["tuning"]["model"], "jev-1.13.0");

    // Only record spoke to the server.
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

/// A rubric with one question asked of every case and one asked only when the
/// state has an `order`, and cases where the order is there, absent and empty
/// (an empty object is not present).
const WHEN_RUBRIC: &str = "\
apiVersion: jud/v1.3
kind: Rubric
metadata:
  name: refund-routing
spec:
  questions:
    urgent:
      type: noul
      instructions: Is `message` urgent?
    refund:
      type: noul
      instructions: Does `message` ask for `order` to be refunded?
      when: order
  policy:
    urgent:
      threshold: 0.5
    refund:
      threshold: 0.5
";

const WHEN_CASES: &str = "\
apiVersion: jud/v1.3
kind: Cases
metadata:
  name: refund-routing-cases
spec:
  rubric: refund-routing
  cases:
    - id: with-order
      state:
        message: Please refund order 7.
        order: {id: 7}
      expect:
        urgent: false
        refund: true
    - id: no-order
      state:
        message: Where is your pricing page?
      expect:
        urgent: false
    - id: empty-order
      state:
        message: Anything new?
        order: {}
      expect:
        urgent: false
";

#[tokio::test]
async fn a_question_asked_only_when_the_state_has_what_it_is_about_is_asked_and_graded_only_then() {
    let dir = scratch("when");
    let rubric = dir.join("rubric.jud");
    let cases = dir.join("cases.jud");
    std::fs::write(&rubric, WHEN_RUBRIC).unwrap();
    std::fs::write(&cases, WHEN_CASES).unwrap();
    let (rubric, cases) = (rubric.to_str().unwrap(), cases.to_str().unwrap());
    let recordings = dir.join("recordings");

    let server = answering_server(3).await;
    let out = record(&server, rubric, cases, &recordings);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    // Three cases, three requests, and the second question in only one.
    let sent = received(&server).await;
    assert_eq!(sent[0].ids(), ["urgent", "refund"]);
    assert_eq!(sent[1].ids(), ["urgent"]);
    assert_eq!(sent[2].ids(), ["urgent"]);

    checked(rubric, cases, &recordings);

    let report = eval_json(rubric, cases, &recordings);
    assert_eq!(report["requests"], 3);
    assert_eq!(asked(&report, "urgent"), 3);
    assert_eq!(asked(&report, "refund"), 1);
    assert_eq!(question(&report, "urgent")["labelled"], 3);
    assert_eq!(question(&report, "refund")["labelled"], 1);
}

#[tokio::test]
async fn the_triage_documents_go_round_the_loop_and_a_tuned_rubric_still_replays() {
    let dir = scratch("triage");
    let recordings = dir.join("recordings");
    let server = server(triage_answers(), 7).await;

    // record -> eval: the accuracies are the ones the server's one answer
    // earns against the seven labels (yes is right for five requests, billing
    // for two cases, angry for the one of the six with a tone that was angry).
    let out = record(&server, TRIAGE, TRIAGE_CASES, &recordings);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(
        stderr(&out).contains("recorded 7, kept 0 in "),
        "{}",
        stderr(&out)
    );
    assert_eq!(names(&recordings).len(), 7);
    let report = eval_json(TRIAGE, TRIAGE_CASES, &recordings);
    assert_eq!(report["requests"], 7);
    let graded: Vec<(u64, u64)> = ["actionable", "desk", "tone"]
        .into_iter()
        .map(|id| {
            let q = question(&report, id);
            (
                q["labelled"].as_u64().unwrap(),
                q["correct"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(graded, [(7, 5), (7, 2), (6, 1)]);

    // eval -> tune --out: a whole rubric with the proposal applied.
    let tuned = dir.join("tuned.jud");
    let tune = jud(
        &[
            "tune",
            TRIAGE,
            TRIAGE_CASES,
            "--replay",
            recordings.to_str().unwrap(),
            "--out",
            tuned.to_str().unwrap(),
        ],
        "",
        &[],
    );
    assert_eq!(code(&tune), 0, "{}", stderr(&tune));
    assert!(stdout(&tune).is_empty(), "{}", stdout(&tune));
    assert!(tuned.is_file());

    // The tuned rubric is a document the reader accepts, and says it is tuned.
    let tuned = tuned.to_str().unwrap();
    let alone = jud(&["check", tuned], "", &[]);
    assert_eq!(code(&alone), 0, "{}{}", stdout(&alone), stderr(&alone));
    assert!(stdout(&alone).contains("(tuned)"), "{}", stdout(&alone));

    // Tuning changes the policy and never the questions, so the recordings
    // are still the answers to the requests its cases lower to, and the same
    // recordings replay under it with no call and no key.
    checked(tuned, TRIAGE_CASES, &recordings);
    let again = eval_json(tuned, TRIAGE_CASES, &recordings);
    assert_eq!(again["requests"], 7);
    assert_eq!(
        again["rubric"]["fingerprint"],
        report["rubric"]["fingerprint"]
    );
    assert_ne!(
        again["rubric"]["policy_fingerprint"], report["rubric"]["policy_fingerprint"],
        "the proposal moved a bar"
    );
    for id in ["actionable", "desk", "tone"] {
        assert_eq!(
            question(&again, id)["correct"],
            question(&report, id)["correct"]
        );
    }

    // And no run but the first spoke to the server.
    assert_eq!(server.received_requests().await.unwrap().len(), 7);
}

// ---- the command line's guidance -----------------------------------------------

/// What a newcomer is told when the arguments are wrong: status 2, nothing on
/// stdout, and an error on stderr.
fn refused(args: &[&str], env: &[(&str, &str)]) -> String {
    let out = jud(args, "", env);
    assert_eq!(code(&out), 2, "{args:?}: {}", stderr(&out));
    assert!(stdout(&out).is_empty(), "{args:?}: {}", stdout(&out));
    stderr(&out)
}

const SUBCOMMANDS: &str = "config, check, lower, record, eval, tune, split, completion";

#[test]
fn a_replay_directory_with_no_rubric_says_a_rubric_is_required() {
    // `arg_required_else_help` prints the help only for an empty command
    // line; the flag and the variable are arguments, and used to end in a
    // status 2 that said nothing at all.
    let by_flag = refused(&["--replay", RECORDINGS], &[]);
    let by_env = refused(&[], &[("JUD_REPLAY", RECORDINGS)]);
    for message in [by_flag, by_env] {
        assert_eq!(
            message,
            format!(
                "jud: a rubric is required: `jud RUBRIC < state.json`, or a subcommand ({SUBCOMMANDS}); see `jud --help`\n"
            )
        );
    }
    // No argument at all is still the help, on stderr, as clap prints it.
    let bare = refused(&[], &[]);
    assert!(bare.contains("Usage: jud [OPTIONS] [RUBRIC]"), "{bare}");
    assert!(!bare.starts_with("jud: "), "{bare}");
}

#[test]
fn a_replay_flag_before_a_subcommand_is_pointed_after_it() {
    let advice = "--replay belongs after the subcommand: jud eval RUBRIC CASES --replay DIR";
    let hint = format!("jud: {advice}\n");
    // With the files: clap's own error, which names neither the flag's place
    // nor the subcommand, and then the hint.
    let message = refused(&["--replay", RECORDINGS, "eval", TRIAGE, TRIAGE_CASES], &[]);
    assert!(message.starts_with("error: the subcommand "), "{message}");
    assert!(message.contains("--replay <DIR>"), "{message}");
    assert!(message.ends_with(&hint), "{message}");
    // Without: the word is read as a rubric, which it is not.
    assert_eq!(
        refused(&["--replay", RECORDINGS, "eval"], &[]),
        format!("jud: eval is a subcommand, not a rubric. {advice}\n")
    );
    // Where the hint says it belongs, it works.
    let out = jud(
        &["eval", TRIAGE, TRIAGE_CASES, "--replay", RECORDINGS],
        "",
        &[],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
}

#[test]
fn a_file_named_like_a_subcommand_is_still_read_as_the_rubric() {
    // `jud --replay DIR eval < state` has always run the rubric file `eval`
    // when there is one; only a name that is no file is a mistake.
    let dir = scratch("named");
    std::fs::copy(TRIAGE, dir.join("eval")).unwrap();
    let out = command(&["--replay", RECORDINGS, "eval"])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    // Read as the rubric: stdin is empty, which only a rubric run says.
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("stdin is empty"), "{}", stderr(&out));
    assert!(
        !stderr(&out).contains("is a subcommand"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_mistyped_subcommand_says_it_is_not_a_subcommand_either() {
    let hint = format!(" (not a subcommand either: {SUBCOMMANDS})");
    for word in ["evaluate", "recrod", "tuen", "records"] {
        let message = refused(&[word], &[]);
        assert_eq!(
            message,
            format!(
                "jud: cannot read rubric {word}: No such file or directory (os error 2){hint}\n"
            )
        );
    }
    // A path or a file with an extension that is not there is a missing file
    // and nothing else, and a file that is there is judged as a document.
    for word in ["evaluate.jud", "dir/evaluate", "./evaluate"] {
        let message = refused(&[word], &[]);
        assert!(message.starts_with("jud: cannot read rubric "), "{message}");
        assert!(!message.contains("subcommand"), "{message}");
    }
    let dir = scratch("bare");
    std::fs::write(dir.join("notes"), "just words\n").unwrap();
    let out = command(&["notes"]).current_dir(&dir).output().unwrap();
    assert_eq!(code(&out), 2);
    assert!(
        stderr(&out).contains("notes is not a valid Rubric"),
        "{}",
        stderr(&out)
    );
    assert!(!stderr(&out).contains("subcommand"), "{}", stderr(&out));
}

#[test]
fn a_mistyped_subcommand_before_a_file_is_explained() {
    // clap reads the first word as the RUBRIC and the file as a subcommand,
    // and says the file cannot be used with the rubric.
    let message = refused(&["evl", TRIAGE], &[]);
    assert!(message.starts_with("error: the subcommand "), "{message}");
    assert!(
        message.ends_with(&format!(
            "jud: `jud RUBRIC` takes one file, and the first word is not a subcommand ({SUBCOMMANDS})\n"
        )),
        "{message}"
    );
    // A real subcommand with a wrong argument is clap's alone: no hint.
    let message = refused(&["eval"], &[]);
    assert!(
        message.contains("required arguments were not provided"),
        "{message}"
    );
    assert!(!message.contains("jud: "), "{message}");
}

#[test]
fn the_top_level_help_states_the_exit_statuses_of_every_command_and_the_loop() {
    let out = jud(&["--help"], "", &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    // The help wraps its lines; the sentences are what is asserted.
    let help = stdout(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        help.contains(
            "Exit status: 0 success; 1 a backend call failed, or a recording is missing under --replay; 2 wrong before any call: the invocation, a file, the state or the configuration; 3 `jud eval` only: a --min-accuracy gate was not met."
        ),
        "{help}"
    );
    assert!(!help.contains("verdicts printed"), "{help}");
    for line in [
        "jud record rubric.jud cases.jud --out recordings/",
        "jud eval rubric.jud cases.jud --replay recordings/",
        "jud tune rubric.jud cases.jud --replay recordings/",
    ] {
        assert!(help.contains(line), "{line}: {help}");
    }
}

#[test]
fn a_completion_script_nobody_reads_is_still_a_success() {
    // `jud completion bash | head -1`: the reader goes away, the script was
    // complete. Best effort: a child that wrote before the pipe closed ends
    // the same way, so this fails only when a closed reader changes the status.
    let mut child = command(&["completion", "bash"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
}

// ---- what the subprocess tests stand on --------------------------------------

#[test]
fn the_subprocess_is_cut_off_from_the_developers_proxy_key_and_replay_directory() {
    let spawned = command(&["config"]);
    let envs: Vec<(String, Option<String>)> = spawned
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    let set = |name: &str| envs.iter().find(|(k, _)| k == name).cloned();
    // `None` is a variable removed from the child's environment, not merely
    // left unset by the test: it is what the developer's shell exports that
    // must not reach the binary.
    for name in [
        "TYPESAFE_API_KEY",
        "TYPESAFE_BASE_URL",
        "JUD_REPLAY",
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        assert_eq!(set(name), Some((name.to_owned(), None)), "{name}");
    }
    // Loopback is exempt from any proxy the child finds anyway.
    for name in ["NO_PROXY", "no_proxy"] {
        assert_eq!(
            set(name),
            Some((name.to_owned(), Some("127.0.0.1,localhost".to_owned())))
        );
    }
    // A configuration directory of its own, never the developer's.
    let (_, home) = set("XDG_CONFIG_HOME").unwrap();
    assert!(Path::new(&home.unwrap()).starts_with(env!("CARGO_TARGET_TMPDIR")),);
    assert_eq!(spawned.get_current_dir(), Some(Path::new(ROOT)));
}

#[test]
fn scratch_directories_are_under_cargos_own_and_start_empty() {
    let dir = scratch("probe");
    assert!(
        dir.starts_with(env!("CARGO_TARGET_TMPDIR")),
        "{}",
        dir.display()
    );
    assert_eq!(names(&dir), Vec::<String>::new());
    // A directory left by an earlier process of the same id is not inherited.
    std::fs::write(dir.join("left-behind"), "stale").unwrap();
    assert_eq!(names(&support::emptied(&dir)), Vec::<String>::new());
    assert!(dir.is_dir());
}
