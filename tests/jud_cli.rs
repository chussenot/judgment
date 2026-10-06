//! The `jud` command line, run as a subprocess: a Rubric file and the JSON
//! state on stdin in, verdicts out, with every refusal on stderr and a
//! non-zero status. The backend is a wiremock server standing in for any
//! System One server, reached through `TYPESAFE_BASE_URL` or the user's
//! configuration file; no test needs a key that works or the network.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RUBRIC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/triage.jud");
const CASES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/triage-cases.jud");

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh directory for `XDG_CONFIG_HOME`, so no test reads a real
/// `~/.config/jud/config.yaml` and each may write its own.
fn config_home() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("jud-cli-{}-{n}", std::process::id()));
    std::fs::create_dir_all(dir.join("jud")).unwrap();
    dir
}

/// Run `jud` with `args`, `stdin` and only the environment given: the
/// crate's variables are cleared first, so the host's key never leaks in.
fn jud(args: &[&str], stdin: &str, env: &[(&str, &str)], config_home: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    command
        .args(args)
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("TYPESAFE_BASE_URL")
        .env("XDG_CONFIG_HOME", config_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        command.env(k, v);
    }
    let mut child = command.spawn().unwrap();
    // A refusal before stdin is read closes the pipe early; that is the
    // behaviour under test, not a failure of the test.
    let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
    child.wait_with_output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The triage rubric's three answers, as a server sends them.
fn answers() -> Value {
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

async fn backend(status: u16, body: Value, expected_calls: u64) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .and(header("authorization", "Bearer test-key"))
        .and(body_partial_json(json!({ "model": "jev-latest", "state": { "message": "Charged twice. Fix it today." } })))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .expect(expected_calls)
        .mount(&server)
        .await;
    server
}

const STATE: &str = r#"{"message": "Charged twice. Fix it today."}"#;

#[tokio::test]
async fn a_valid_rubric_and_state_print_one_verdict_per_question() {
    let server = backend(200, answers(), 1).await;
    let out = jud(
        &[RUBRIC],
        STATE,
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &server.uri()),
        ],
        &config_home(),
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let verdicts: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
    // The verdicts are the library's own serialisation: `verdict` tags the
    // kind, the rest is the answer read through the gate.
    assert_eq!(verdicts["actionable"]["verdict"], "yes");
    assert_eq!(verdicts["actionable"]["probability"], 0.91);
    assert_eq!(verdicts["desk"]["verdict"], "option");
    assert_eq!(verdicts["desk"]["key"], "billing");
    assert_eq!(verdicts["tone"]["verdict"], "level");
    assert_eq!(verdicts["tone"]["label"], "angry");
    assert_eq!(verdicts["tone"]["index"], 2);
    assert_eq!(verdicts.as_object().unwrap().len(), 3);
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
}

#[tokio::test]
async fn stdout_is_exactly_the_verdict_map_and_nothing_else() {
    let server = backend(200, answers(), 1).await;
    let out = jud(
        &[RUBRIC],
        STATE,
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &server.uri()),
        ],
        &config_home(),
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    // One pretty-printed JSON object, newline-terminated, nothing before or after.
    assert!(text.starts_with("{\n"), "{text}");
    assert!(text.ends_with("}\n"), "{text}");
    let parsed: Value = serde_json::from_str(&text).unwrap();
    let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["actionable", "desk", "tone"], "the rubric's order");
}

#[test]
fn an_invalid_rubric_is_refused_before_any_call() {
    let home = config_home();
    let bad = home.join("bad.jud");
    std::fs::write(&bad, "apiVersion: jud/v1.3\nkind: Rubric\nmetadata: {name: r}\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n  policy:\n    n: {confidence: 0.5}\n").unwrap();
    let out = jud(
        &[bad.to_str().unwrap()],
        STATE,
        &[("TYPESAFE_API_KEY", "test-key")],
        &home,
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("is not a valid Rubric"),
        "{}",
        stderr(&out)
    );
    assert!(stdout(&out).is_empty());
    // A document of another kind is named as such.
    let out = jud(&[CASES], STATE, &[("TYPESAFE_API_KEY", "test-key")], &home);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("kind: Cases"), "{}", stderr(&out));
}

#[test]
fn a_missing_rubric_file_is_refused() {
    let out = jud(
        &["/nonexistent/rubric.jud"],
        STATE,
        &[("TYPESAFE_API_KEY", "test-key")],
        &config_home(),
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("cannot read rubric /nonexistent/rubric.jud"),
        "{}",
        stderr(&out)
    );
    let out = jud(&[], STATE, &[], &config_home());
    assert_eq!(out.status.code(), Some(2), "no argument at all");
    assert!(stderr(&out).contains("cat state.json | jud RUBRIC.jud"));
}

#[test]
fn invalid_json_on_stdin_is_refused_with_the_position() {
    let home = config_home();
    let out = jud(
        &[RUBRIC],
        "{\"message\": ",
        &[("TYPESAFE_API_KEY", "test-key")],
        &home,
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("stdin is not JSON"),
        "{}",
        stderr(&out)
    );
    // Two JSON values, as `jq '.items[]'` prints, get the hint for it.
    let out = jud(
        &[RUBRIC],
        "{\"message\": \"a\"}\n{\"message\": \"b\"}\n",
        &[("TYPESAFE_API_KEY", "test-key")],
        &home,
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("more than one JSON value"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn empty_stdin_is_refused() {
    for empty in ["", "\n", "   \n"] {
        let out = jud(
            &[RUBRIC],
            empty,
            &[("TYPESAFE_API_KEY", "test-key")],
            &config_home(),
        );
        assert_eq!(out.status.code(), Some(2), "{empty:?}");
        assert!(stderr(&out).contains("stdin is empty"), "{}", stderr(&out));
    }
}

#[tokio::test]
async fn a_backend_failure_is_exit_1_with_the_servers_message() {
    let server = backend(
        500,
        json!({ "error": { "message": "boom", "type": "server_error" } }),
        // A 5xx is retried with the SDKs' policy before it is given up on.
        3,
    )
    .await;
    let out = jud(
        &[RUBRIC],
        STATE,
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &server.uri()),
        ],
        &config_home(),
    );
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.contains("the backend at") && err.contains(&server.uri()),
        "{err}"
    );
    assert!(stdout(&out).is_empty());
}

#[tokio::test]
async fn an_answer_that_does_not_fit_the_rubric_is_exit_1() {
    let mut body = answers();
    body["answers"]["desk"]["choice"] = json!("legal");
    body["answers"]["desk"]["probabilities"] = json!({ "legal": 1.0 });
    let server = backend(200, body, 1).await;
    let out = jud(
        &[RUBRIC],
        STATE,
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &server.uri()),
        ],
        &config_home(),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("legal"),
        "an off-list option is named: {}",
        stderr(&out)
    );
}

#[test]
fn missing_credentials_are_refused_before_any_call() {
    let out = jud(
        &[RUBRIC],
        STATE,
        &[("TYPESAFE_BASE_URL", "http://127.0.0.1:9")],
        &config_home(),
    );
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(
        err.contains("no API key")
            && err.contains("TYPESAFE_API_KEY")
            && err.contains("config.yaml"),
        "{err}"
    );
}

#[test]
fn the_default_backend_is_the_hosted_typesafe_api() {
    let home = config_home();
    let out = jud(&["config"], "", &[], &home);
    assert!(out.status.success(), "{}", stderr(&out));
    let config: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(config["base_url"], "https://api.typesafe.ai");
    assert_eq!(config["base_url_from"], "default");
    assert_eq!(config["model"], "jev-latest");
    assert_eq!(config["api_key"], "missing");
    assert_eq!(config["config_file_present"], false);
    assert!(
        config["config_file"]
            .as_str()
            .unwrap()
            .ends_with("jud/config.yaml")
    );
    // With a key in the environment, the key itself is never printed.
    let out = jud(
        &["config"],
        "",
        &[("TYPESAFE_API_KEY", "sk-secret-value")],
        &home,
    );
    let text = stdout(&out);
    assert!(text.contains("\"api_key\": \"environment\""), "{text}");
    assert!(!text.contains("sk-secret-value"));
}

#[tokio::test]
async fn a_configured_system_one_server_is_used_without_any_environment() {
    let server = backend(200, answers(), 1).await;
    let home = config_home();
    std::fs::write(
        home.join("jud/config.yaml"),
        format!(
            "base_url: {}\napi_key: test-key\ntimeout_secs: 5\n",
            server.uri()
        ),
    )
    .unwrap();
    let out = jud(&["config"], "", &[], &home);
    let config: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(config["base_url"], server.uri());
    assert_eq!(config["base_url_from"], "config_file");
    assert_eq!(config["api_key"], "config_file");
    assert_eq!(config["timeout_secs"], 5);
    let out = jud(&[RUBRIC], STATE, &[], &home);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&out)).unwrap()["desk"]["key"],
        "billing"
    );
    // The environment wins over the file, as the crate's own variables do.
    let out = jud(
        &["config"],
        "",
        &[("TYPESAFE_BASE_URL", "http://example.invalid")],
        &home,
    );
    let config: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(config["base_url"], "http://example.invalid");
    assert_eq!(config["base_url_from"], "environment");
}

#[test]
fn a_malformed_configuration_file_is_refused_by_name() {
    let home = config_home();
    std::fs::write(
        home.join("jud/config.yaml"),
        "base_url: http://x\nmodle: jev-latest\n",
    )
    .unwrap();
    let out = jud(&["config"], "", &[], &home);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("is not a jud configuration"),
        "{}",
        stderr(&out)
    );
}
