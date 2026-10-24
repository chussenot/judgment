//! `jud eval --swap` and `--tone-perturb`, and `jud record` with them, run
//! as a subprocess: no key and no network.
//!
//! A wiremock judge with pure position bias (it picks whichever option is
//! listed first) pins what the report counts; the pairwise-judge recordings
//! of Jev 1.13 and Jev 1.14 pin what the guide
//! (`docs/guides/test-a-judge-for-position-and-tone-bias.md`) reports.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

mod support;

use std::process::Output;

use serde_json::{Value, json};
use support::{ROOT, code, jud, scratch, stderr, stdout};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const PAIRWISE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/jud/pairwise-judge.jud"
);
const PAIRWISE_CASES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/jud/pairwise-judge-cases.jud"
);
const PAIRWISE_TONE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/jud/pairwise-judge-tone.yaml"
);

const RUBRIC: &str = "\
apiVersion: jud/v1
kind: Rubric
metadata: {name: pair, minor: 4}
spec:
  questions:
    better:
      type: choice
      instructions: Which answer is better?
      criteria: {}
      options_from: request
";

const CASES: &str = "\
apiVersion: jud/v1
kind: Cases
metadata: {name: pair-cases, minor: 4}
spec:
  rubric: pair
  cases:
    - id: one
      state: {instruction: say hi}
      options: {better: {opt_k1: hi, opt_k2: bye}}
      expect: {better: opt_k1}
    - id: two
      state: {instruction: say bye}
      options: {better: {opt_k1: hi, opt_k2: bye}}
      expect: {better: opt_k2}
";

/// `text` in a file of a fresh scratch directory; its path.
fn file(name: &str, text: &str) -> String {
    let path = scratch("perturb").join(name);
    std::fs::write(&path, text).unwrap();
    path.display().to_string()
}

/// A judge that picks the option listed first, 0.8 against 0.2, for every
/// Choice in the request. The order is read from the body as sent, since a
/// parsed object need not keep it.
struct FirstListed;

impl Respond for FirstListed {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body = String::from_utf8_lossy(&request.body).into_owned();
        let mut answers = serde_json::Map::new();
        for id in ["better", "better.swapped"] {
            let Some(at) = body.find(&format!("\"{id}\":{{")) else {
                continue;
            };
            let rest = &body[at..];
            let k1 = rest.find("\"opt_k1\"").unwrap_or(usize::MAX);
            let k2 = rest.find("\"opt_k2\"").unwrap_or(usize::MAX);
            let (pick, other) = if k1 < k2 {
                ("opt_k1", "opt_k2")
            } else {
                ("opt_k2", "opt_k1")
            };
            answers.insert(
                id.to_owned(),
                json!({"type": "choice", "choice": pick,
                       "probabilities": {pick: 0.8, other: 0.2}, "confidence": 0.6}),
            );
        }
        ResponseTemplate::new(200).set_body_json(json!({"model": "jev-test", "answers": answers}))
    }
}

async fn first_listed() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(FirstListed)
        .mount(&server)
        .await;
    server
}

fn asking(server: &MockServer, args: &[&str]) -> Output {
    jud(
        args,
        "",
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &server.uri()),
        ],
    )
}

fn robustness(output: &Output) -> Value {
    assert_eq!(code(output), 0, "{}", stderr(output));
    let report: Value = serde_json::from_str(&stdout(output)).unwrap();
    report["robustness"].clone()
}

async fn bodies(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .collect()
}

/// A position-biased judge flips every pair, each verdict confident, and
/// is right on one pair in each order: what the four columns say.
fn assert_position_biased(swap: &Value) {
    assert_eq!(swap["question"], "better");
    assert_eq!(swap["test"], "swap");
    assert_eq!(
        (swap["pairs"].as_u64(), swap["flips"].as_u64()),
        (Some(2), Some(2))
    );
    assert_eq!(swap["flip_rate"], 1.0);
    assert_eq!(swap["confident_flips"], 2);
    assert!((swap["mean_abs_dp"].as_f64().unwrap() - 0.6).abs() < 1e-9);
    assert_eq!(swap["original"]["correct"], 1);
    assert_eq!(swap["perturbed"]["correct"], 1);
    assert_eq!(swap["flipped"], json!(["one", "two"]));
    let interval = swap["flip_rate_interval95"].as_array().unwrap();
    assert!(
        interval[0].as_f64().unwrap() < 1.0 && interval[1] == 1.0,
        "{interval:?}"
    );
}

#[tokio::test]
async fn a_separate_swap_asks_the_reversed_order_on_its_own_under_its_own_id() {
    let server = first_listed().await;
    let (rubric, cases) = (file("pair.jud", RUBRIC), file("pair-cases.jud", CASES));
    let out = asking(&server, &["eval", &rubric, &cases, "--swap", "--json"]);
    let r = robustness(&out);
    assert_eq!(r["perturbed_requests"], 2);
    assert_position_biased(&r["comparisons"][0]);
    let sent = bodies(&server).await;
    assert_eq!(sent.len(), 4, "two cases, each in both orders");
    let swapped: Vec<&String> = sent
        .iter()
        .filter(|b| b.contains("better.swapped"))
        .collect();
    assert_eq!(swapped.len(), 2);
    for body in swapped {
        assert!(
            !body.contains("\"better\":"),
            "the reversed request asks nothing else: {body}"
        );
        assert!(
            body.find("opt_k2") < body.find("opt_k1"),
            "reversed: {body}"
        );
    }
}

#[tokio::test]
async fn a_joint_swap_asks_both_orders_in_one_request() {
    let server = first_listed().await;
    let (rubric, cases) = (file("pair.jud", RUBRIC), file("pair-cases.jud", CASES));
    let out = asking(
        &server,
        &["eval", &rubric, &cases, "--swap=joint", "--json"],
    );
    let r = robustness(&out);
    assert_eq!(
        r["perturbed_requests"], 0,
        "the copies ride in the cases' own requests"
    );
    assert_position_biased(&r["comparisons"][0]);
    let sent = bodies(&server).await;
    assert_eq!(sent.len(), 2);
    assert!(
        sent.iter()
            .all(|b| b.contains("\"better\":") && b.contains("\"better.swapped\":"))
    );
}

#[tokio::test]
async fn a_tone_rewrite_replaces_the_labelled_option_only() {
    let server = first_listed().await;
    let (rubric, cases) = (file("pair.jud", RUBRIC), file("pair-cases.jud", CASES));
    let tone = file(
        "tone.yaml",
        "two: {better: \"BYE, and I'm FURIOUS about it\"}\n",
    );
    let out = asking(
        &server,
        &["eval", &rubric, &cases, "--tone-perturb", &tone, "--json"],
    );
    let r = robustness(&out);
    let c = &r["comparisons"][0];
    assert_eq!(
        (c["test"].as_str(), c["pairs"].as_u64()),
        (Some("tone"), Some(1))
    );
    assert_eq!(
        c["flips"], 0,
        "the first-listed judge reads places, not tone"
    );
    let sent = bodies(&server).await;
    let toned: Vec<&String> = sent.iter().filter(|b| b.contains("FURIOUS")).collect();
    assert_eq!(toned.len(), 1);
    assert!(
        toned[0].contains("\"opt_k1\":\"hi\""),
        "the other answer is untouched: {}",
        toned[0]
    );
    assert!(!toned[0].contains("\"opt_k2\":\"bye\""), "{}", toned[0]);
}

#[tokio::test]
async fn a_rewrite_that_lands_nowhere_is_refused_before_any_call() {
    let server = first_listed().await;
    let (rubric, cases) = (file("pair.jud", RUBRIC), file("pair-cases.jud", CASES));
    let unlabelled = CASES.replace("      expect: {better: opt_k2}\n", "");
    let unlabelled = file("unlabelled-cases.jud", &unlabelled);
    for (cases, tone, says) in [
        (
            &cases,
            "three: {better: x}\n",
            "which the cases do not have",
        ),
        (
            &cases,
            "one: {worse: x}\n",
            "a question its request does not ask",
        ),
        (
            &unlabelled,
            "two: {better: x}\n",
            "does not label with an option",
        ),
    ] {
        let tone = file("tone.yaml", tone);
        let out = asking(&server, &["eval", &rubric, cases, "--tone-perturb", &tone]);
        assert_eq!(code(&out), 2, "{}", stderr(&out));
        assert!(stderr(&out).contains(says), "{}", stderr(&out));
    }
    let noul = "apiVersion: jud/v1\nkind: Rubric\nmetadata: {name: n, minor: 4}\nspec:\n  questions:\n    ok: {type: noul, instructions: ok?}\n";
    let noul_cases = "apiVersion: jud/v1\nkind: Cases\nmetadata: {name: nc, minor: 4}\nspec:\n  cases:\n    - {id: c, state: s, expect: {ok: true}}\n";
    let out = asking(
        &server,
        &[
            "eval",
            &file("n.jud", noul),
            &file("nc.jud", noul_cases),
            "--swap",
        ],
    );
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(stderr(&out).contains("asks no Choice"), "{}", stderr(&out));
    assert!(bodies(&server).await.is_empty(), "nothing was asked");
}

#[tokio::test]
async fn record_keeps_the_swap_apart_and_eval_replays_it_with_no_key() {
    let server = first_listed().await;
    let (rubric, cases) = (file("pair.jud", RUBRIC), file("pair-cases.jud", CASES));
    let dir = scratch("perturb-record");
    let dir = dir.display().to_string();
    let out = asking(
        &server,
        &["record", &rubric, &cases, "--swap", "--out", &dir],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["one.jud", "one.swapped.jud", "two.jud", "two.swapped.jud"]
    );
    // A swap asked again is kept, not answered from the original order.
    let again = asking(
        &server,
        &["record", &rubric, &cases, "--swap", "--out", &dir],
    );
    assert!(
        stderr(&again).contains("recorded 0, kept 4"),
        "{}",
        stderr(&again)
    );
    let replayed = jud(
        &[
            "eval", &rubric, &cases, "--swap", "--replay", &dir, "--json",
        ],
        "",
        &[],
    );
    assert_position_biased(&robustness(&replayed)["comparisons"][0]);
}

#[test]
fn the_text_report_ends_with_the_markdown_table() {
    let dir = format!("{ROOT}/examples/recordings/pairwise-judge-jev-1.14");
    let out = jud(
        &[
            "eval",
            PAIRWISE,
            PAIRWISE_CASES,
            "--replay",
            &dir,
            "--swap",
            "--confident-at",
            "0.8",
        ],
        "",
        &[],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("| question | test | pairs | flip rate | flip rate, confidence ≥ 0.80 |"),
        "{text}"
    );
    assert!(text.contains("|---|---|---:|"), "{text}");
    assert!(text.contains("| better | swap | 50 |"), "{text}");
}

/// The guide's numbers: each pinned model's recordings replayed under both
/// tests. A re-recording that moves a count fails here.
#[test]
fn the_pairwise_recordings_reproduce_the_guides_numbers() {
    // (model, swap flips, swap accuracy perturbed, tone flips, tone accuracy perturbed)
    for (model, swap_flips, swapped_right, tone_flips, toned_right) in
        [("1.13", 1, 48, 8, 39), ("1.14", 1, 48, 6, 43)]
    {
        let dir = format!("{ROOT}/examples/recordings/pairwise-judge-jev-{model}");
        let out = jud(
            &[
                "eval",
                PAIRWISE,
                PAIRWISE_CASES,
                "--replay",
                &dir,
                "--swap",
                "--tone-perturb",
                PAIRWISE_TONE,
                "--json",
            ],
            "",
            &[],
        );
        let r = robustness(&out);
        assert_eq!(r["perturbed_requests"], 100, "jev-{model}");
        let [swap, tone] = [&r["comparisons"][0], &r["comparisons"][1]];
        assert_eq!(
            (swap["test"].as_str(), tone["test"].as_str()),
            (Some("swap"), Some("tone"))
        );
        for c in [swap, tone] {
            assert_eq!(c["pairs"], 50, "jev-{model}");
            assert_eq!(c["original"]["correct"], 47, "jev-{model}");
        }
        assert_eq!(swap["flips"], swap_flips, "jev-{model}");
        assert_eq!(
            swap["confident_flips"], 0,
            "jev-{model}: only an unsure verdict moved"
        );
        assert_eq!(swap["perturbed"]["correct"], swapped_right, "jev-{model}");
        assert_eq!(tone["flips"], tone_flips, "jev-{model}");
        assert_eq!(tone["perturbed"]["correct"], toned_right, "jev-{model}");
    }
}

#[test]
fn jev_1_14_picks_what_jev_1_13_picks_on_every_pairwise_case() {
    let dir = |model: &str| format!("{ROOT}/examples/recordings/pairwise-judge-jev-{model}");
    let out = jud(
        &[
            "eval",
            PAIRWISE,
            PAIRWISE_CASES,
            "--replay",
            &dir("1.13"),
            "--against",
            &dir("1.14"),
            "--json",
        ],
        "",
        &[],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    let agreement = &report["agreement"];
    assert_eq!(agreement["models"], json!(["jev-1.14.0"]));
    let better = &agreement["comparisons"][0];
    assert_eq!(
        (better["pairs"].as_u64(), better["flips"].as_u64()),
        (Some(50), Some(0))
    );
    let dp = better["mean_abs_dp"].as_f64().unwrap();
    assert!((dp - 0.016).abs() < 1e-9, "{dp}");
    // The same 47 right on both sides: agreeing, they also miss together.
    assert_eq!(better["original"]["correct"], 47);
    assert_eq!(better["perturbed"]["correct"], 47);
}
