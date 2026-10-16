//! `jud` with OTLP export (feature `cli-otel`): what reaches a collector, and
//! what must not change when there is one, or when there is not. The server
//! `jud` asks and the collector it exports to are both wiremock servers.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RUBRIC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/triage.jud");
const STATE: &str = r#"{"message": "Charged twice. Fix it today."}"#;

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

/// A System One server that answers every request with `status` and `body`.
async fn backend(status: u16, body: Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

/// A collector that accepts every OTLP/HTTP request.
async fn collector() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    server
}

/// `jud RUBRIC` on [`STATE`] against `backend_uri`, exporting to `endpoint`
/// when given. Every `TYPESAFE_*`, `JUD_*` and `OTEL_*` variable of this
/// process is cleared first, so the host's own never leaks in.
fn jud(backend_uri: &str, endpoint: Option<&str>) -> Output {
    let home = std::env::temp_dir().join(format!("jud-otel-{}", std::process::id()));
    std::fs::create_dir_all(home.join("jud")).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    command
        .arg(RUBRIC)
        .env("XDG_CONFIG_HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, _) in std::env::vars_os() {
        let name = name.to_string_lossy().into_owned();
        if ["TYPESAFE_", "JUD_", "OTEL_"]
            .iter()
            .any(|p| name.starts_with(p))
        {
            command.env_remove(name);
        }
    }
    command
        .env("TYPESAFE_API_KEY", "test-key")
        .env("TYPESAFE_BASE_URL", backend_uri);
    if let Some(endpoint) = endpoint {
        command.env("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint);
    }
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(STATE.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// The paths a collector was posted to, and every body it received.
async fn received(collector: &MockServer) -> (Vec<String>, Vec<u8>) {
    let requests = collector.received_requests().await.unwrap();
    let paths = requests.iter().map(|r| r.url.path().to_owned()).collect();
    let bodies = requests.iter().flat_map(|r| r.body.clone()).collect();
    (paths, bodies)
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w == needle.as_bytes())
}

/// A call's span and its metrics reach the collector before the process
/// exits, and the verdicts on stdout are the bytes a run without telemetry
/// prints.
#[tokio::test]
async fn a_call_is_exported_and_stdout_is_unchanged() {
    let server = backend(200, answers()).await;
    let collector = collector().await;
    let plain = jud(&server.uri(), None);
    let traced = jud(&server.uri(), Some(&collector.uri()));
    assert!(
        traced.status.success(),
        "{}",
        String::from_utf8_lossy(&traced.stderr)
    );
    assert_eq!(traced.stdout, plain.stdout);
    assert!(
        traced.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&traced.stderr)
    );

    let (paths, bodies) = received(&collector).await;
    assert!(paths.contains(&"/v1/traces".to_owned()), "{paths:?}");
    assert!(paths.contains(&"/v1/metrics".to_owned()), "{paths:?}");
    assert!(
        contains(&bodies, "gen_ai.operation.name"),
        "no GenAI span attribute exported"
    );
    assert!(
        contains(&bodies, "gen_ai.client.token.usage"),
        "no token usage metric exported"
    );
    // Bounded attributes only: nothing of the state is ever an attribute.
    assert!(!contains(&bodies, "Charged twice"));
}

/// A failed call exits 1 as it always did, and its span is exported all the
/// same: the flush runs on every exit path, not only on success.
#[tokio::test]
async fn a_failing_run_still_exports_and_keeps_its_status() {
    let server = backend(
        400,
        json!({ "error": { "message": "bad", "type": "invalid_request" } }),
    )
    .await;
    let collector = collector().await;
    let plain = jud(&server.uri(), None);
    let traced = jud(&server.uri(), Some(&collector.uri()));
    assert_eq!(plain.status.code(), Some(1));
    assert_eq!(traced.status.code(), Some(1));
    assert_eq!(traced.stdout, plain.stdout);
    let (paths, bodies) = received(&collector).await;
    assert!(paths.contains(&"/v1/traces".to_owned()), "{paths:?}");
    assert!(contains(&bodies, "error.type"));
}

/// A collector that is not there is said once on stderr and changes neither
/// stdout nor the status.
#[tokio::test]
async fn an_unreachable_collector_changes_nothing_but_one_stderr_line() {
    let server = backend(200, answers()).await;
    let plain = jud(&server.uri(), None);
    let traced = jud(&server.uri(), Some("http://127.0.0.1:9"));
    assert_eq!(traced.status.code(), plain.status.code());
    assert_eq!(traced.stdout, plain.stdout);
    let err = String::from_utf8_lossy(&traced.stderr);
    assert_eq!(
        err.lines().filter(|l| l.contains("telemetry")).count(),
        1,
        "{err}"
    );
}

/// No endpoint, no telemetry: nothing on stderr either.
#[tokio::test]
async fn without_an_endpoint_nothing_starts() {
    let server = backend(200, answers()).await;
    let out = jud(&server.uri(), None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
}
