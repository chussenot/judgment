//! `jud mcp` (feature `mcp`), driven over stdio the way an agent's host
//! does, with raw newline-delimited JSON-RPC: one tool per rubric, a call
//! that returns the bytes `jud RUBRIC` prints, a failed call that is a tool
//! error, and stdout that carries the protocol and nothing else.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/recordings/jud_calibration"
);
const TRIAGE_STATE: &str = r#"{"message": "This is the third time I'm writing. I was charged twice last month and nobody has refunded me. Fix it today or I cancel."}"#;

fn clean(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        let name = name.to_string_lossy().into_owned();
        if ["TYPESAFE_", "JUD_", "OTEL_"]
            .iter()
            .any(|p| name.starts_with(p))
        {
            command.env_remove(name);
        }
    }
    let home = std::env::temp_dir().join(format!("jud-mcp-{}", std::process::id()));
    std::fs::create_dir_all(home.join("jud")).unwrap();
    command.env("XDG_CONFIG_HOME", home);
}

fn initialize() -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-11-25", "capabilities": {},
        "clientInfo": {"name": "test", "version": "0"}}})
}

fn call(id: u64, tool: &str, arguments: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
           "params": {"name": tool, "arguments": arguments}})
}

/// Start `jud mcp DIR ARGS`, send the handshake and `requests`, and return
/// every line of stdout as JSON once each request with an id has been
/// answered, then close stdin and wait. A line of stdout that is not JSON
/// fails the test: stdout is the protocol channel.
fn session(args: &[&str], env: &[(&str, &str)], requests: &[Value]) -> (Vec<Value>, Output) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    command
        .arg("mcp")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    clean(&mut command);
    for (k, v) in env {
        command.env(k, v);
    }
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    let expected = 1 + requests.iter().filter(|r| r.get("id").is_some()).count();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let send = |stdin: &mut std::process::ChildStdin, message: &Value| {
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    };
    send(&mut stdin, &initialize());
    send(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    );
    for request in requests {
        send(&mut stdin, request);
    }

    let mut messages = Vec::new();
    while messages.len() < expected {
        let line = rx
            .recv_timeout(Duration::from_secs(30))
            .expect("the server answered every request");
        messages.push(
            serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line}")),
        );
    }
    drop(stdin);
    for line in &rx {
        messages.push(
            serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line}")),
        );
    }
    (messages, child.wait_with_output().unwrap())
}

fn result_of(messages: &[Value], id: u64) -> &Value {
    messages
        .iter()
        .find(|m| m["id"] == id)
        .unwrap_or_else(|| panic!("no response to {id}: {messages:?}"))
}

/// What `jud RUBRIC` prints for `state`, from the same recordings.
fn cli_verdicts(rubric: &str, state: &str) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    command
        .args(["--replay", RECORDINGS, rubric])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    clean(&mut command);
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(state.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn a_rubric_is_a_tool_and_its_call_is_what_jud_prints() {
    let state: Value = serde_json::from_str(TRIAGE_STATE).unwrap();
    let (messages, out) = session(
        &[&format!("{ROOT}/examples/jud"), "--replay", RECORDINGS],
        &[],
        &[
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            call(3, "inbox-triage", &json!({ "state": state })),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert_eq!(
        result_of(&messages, 1)["result"]["serverInfo"]["name"],
        "jud"
    );
    let tools = result_of(&messages, 2)["result"]["tools"]
        .as_array()
        .unwrap()
        .clone();
    let tool = tools
        .iter()
        .find(|t| t["name"] == "inbox-triage")
        .expect("triage is a tool");
    assert!(
        tool["description"]
            .as_str()
            .unwrap()
            .contains("actionable (noul)")
    );
    assert_eq!(tool["inputSchema"]["required"], json!(["state"]));
    assert!(tool["inputSchema"]["properties"].get("options").is_some());
    // One tool per rubric: cases and recordings under the directory are not tools.
    assert!(
        tools
            .iter()
            .all(|t| !t["name"].as_str().unwrap().ends_with("cases"))
    );

    let reply = &result_of(&messages, 3)["result"];
    assert_eq!(reply["isError"], false);
    // The bytes `jud RUBRIC` prints, less the newline it ends with.
    let printed = cli_verdicts(&format!("{ROOT}/examples/jud/triage.jud"), TRIAGE_STATE);
    assert_eq!(
        reply["content"][0]["text"].as_str().unwrap(),
        printed.trim_end_matches('\n')
    );
}

#[test]
fn a_call_that_fails_is_a_tool_error_and_a_wrong_call_a_protocol_error() {
    let (messages, out) = session(
        &[&format!("{ROOT}/examples/jud"), "--replay", RECORDINGS],
        &[],
        &[
            call(
                2,
                "inbox-triage",
                &json!({ "state": {"message": "nobody recorded this"} }),
            ),
            call(3, "no-such-rubric", &json!({ "state": {} })),
            call(4, "inbox-triage", &json!({})),
        ],
    );
    assert!(out.status.success());
    let failed = &result_of(&messages, 2)["result"];
    assert_eq!(failed["isError"], true);
    assert!(
        failed["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("no recording")
    );
    assert_eq!(result_of(&messages, 3)["error"]["code"], -32602);
    assert_eq!(result_of(&messages, 4)["error"]["code"], -32602);
}

/// Options a caller supplies are asked in the order given, as `--options`
/// does, though a JSON object read into a sorted map would lose it.
#[tokio::test]
async fn supplied_options_keep_their_order() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "jev-1.13.0",
            "answers": {
                "desk": { "type": "choice", "choice": "zebra",
                          "probabilities": { "zebra": 0.9, "alpha": 0.05, "none_of_these": 0.05 },
                          "confidence": 0.85 },
                "tone": { "type": "score", "score": 1.0,
                          "legend": { "0": "calm", "1": "annoyed", "2": "angry", "3": "abusive" },
                          "probabilities": { "0": 0.1, "1": 0.8, "2": 0.1, "3": 0.0 },
                          "confidence": 0.7 },
                "duplicate_of": { "type": "choice", "choice": "T-1042",
                                  "probabilities": { "T-1042": 0.9, "none": 0.1 },
                                  "confidence": 0.8 }
            },
            "usage": { "input_tokens": 400, "output_tokens": 40 }
        })))
        .mount(&server)
        .await;
    let uri = server.uri();
    let state = json!({"message": {"text": "I was charged twice, refund me"}, "customer": {"open_tickets": ["T-1042"]}});
    // Written out, not built: a `json!` map would sort the keys.
    let options: Value = serde_json::from_str(r#"{"desk": {"zebra": "Payments and refunds", "alpha": "Login and access"}, "duplicate_of": {"T-1042": "Charge dispute opened yesterday"}}"#).unwrap();
    let (messages, out) = tokio::task::spawn_blocking(move || {
        session(
            &[&format!("{ROOT}/examples/jud")],
            &[
                ("TYPESAFE_API_KEY", "test-key"),
                ("TYPESAFE_BASE_URL", &uri),
            ],
            &[call(
                2,
                "support-routing",
                &json!({ "state": state, "options": options }),
            )],
        )
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let reply = &result_of(&messages, 2)["result"];
    assert_eq!(reply["isError"], false, "{messages:?}");
    let verdicts: Value =
        serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(verdicts["desk"]["key"], "zebra");

    let sent = server.received_requests().await.unwrap();
    let body = std::str::from_utf8(&sent[0].body).unwrap();
    let at = body.find("\"desk\"").unwrap();
    assert!(
        body[at..].find("\"zebra\"").unwrap() < body[at..].find("\"alpha\"").unwrap(),
        "zebra was supplied first: {body}"
    );
}

/// With no key and no replay, the hosted API is refused before anything is
/// served, as `jud RUBRIC` refuses it: status 2, stdout empty.
#[test]
fn without_a_key_it_refuses_to_start_as_jud_does() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    command
        .args(["mcp", &format!("{ROOT}/examples/jud")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    clean(&mut command);
    let out = command.output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no API key"));
}
