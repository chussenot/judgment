//! What the `jud` subcommand tests share: the binary run as a subprocess in
//! a scratch configuration directory with no key, no `TYPESAFE_*` variable
//! and no proxy, the example documents, a scratch directory per test, and the
//! mock server and the answers the triage rubric gets from it.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub const ROOT: &str = env!("CARGO_MANIFEST_DIR");

pub const TRIAGE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/triage.jud");
pub const TRIAGE_CASES: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/triage-cases.jud");
pub const HANDOFF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/handoff.jud");
pub const HANDOFF_CASES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/jud/handoff-cases.jud"
);
/// The answers `jev-1.13.0` gave to the triage and handoff cases.
pub const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/recordings/jud_calibration"
);

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh, empty directory under cargo's own scratch space for integration
/// tests (`target/tmp`), so `cargo clean` removes what the tests leave. The
/// process id and a counter keep tests apart; a directory left by an earlier
/// process with the same id is removed first, so no test starts from files it
/// did not write.
pub fn scratch(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("jud-{label}-{}-{n}", std::process::id()));
    emptied(&dir)
}

/// `dir` made empty and returned: removed with whatever it holds, then
/// created, so a directory a process of the same id left behind is not
/// inherited.
pub fn emptied(dir: &Path) -> PathBuf {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    dir.to_path_buf()
}

/// A fresh `XDG_CONFIG_HOME`, so no test reads a real configuration file.
pub fn config_home() -> PathBuf {
    let dir = scratch("config");
    std::fs::create_dir_all(dir.join("jud")).unwrap();
    dir
}

/// The proxy variables a developer's shell may export, in both spellings.
const PROXY_VARIABLES: [&str; 6] = [
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
];

/// `jud ARGS` in the repository root with the environment cleaned: no
/// `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL` or `JUD_REPLAY`, a scratch
/// configuration directory, and no proxy (a proxy in the developer's shell
/// would swallow the calls to a wiremock server on 127.0.0.1, so none is
/// set and loopback is exempt from any the child still finds). The
/// standard streams are the caller's to set.
pub fn command(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    command
        .current_dir(ROOT)
        .args(args)
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("TYPESAFE_BASE_URL")
        .env_remove("JUD_REPLAY")
        .env("XDG_CONFIG_HOME", config_home());
    for variable in own_variables() {
        command.env_remove(variable);
    }
    for variable in PROXY_VARIABLES {
        command.env_remove(variable);
    }
    command
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost");
    command
}

/// Every `TYPESAFE_*` and `JUD_*` variable in this process: the ones jud
/// reads would point a run at the developer's server or key, and any other
/// one makes jud warn that it does not read it.
pub fn own_variables() -> Vec<String> {
    std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .filter(|name| name.starts_with("TYPESAFE_") || name.starts_with("JUD_"))
        .collect()
}

/// Run `jud ARGS` with `stdin`, as [`command`] sets it up, plus whatever
/// `env` sets.
pub fn jud(args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Output {
    let mut command = command(args);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        command.env(k, v);
    }
    let mut child = command.spawn().unwrap();
    let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
    child.wait_with_output().unwrap()
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn code(output: &Output) -> i32 {
    output.status.code().unwrap()
}

/// Copy the files of `from` into a fresh scratch directory.
pub fn copy_dir(from: &str) -> PathBuf {
    let to = scratch("copy");
    for entry in std::fs::read_dir(Path::new(from)).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            std::fs::copy(&path, to.join(path.file_name().unwrap())).unwrap();
        }
    }
    to
}

/// The triage rubric's three answers, as a server sends them: the same for
/// every case, so what a report says follows from the labels alone.
pub fn triage_answers() -> Value {
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

/// A server that answers `calls` requests with `body` and fails the test, when
/// it is dropped, if it was asked any other number of times.
pub async fn server(body: Value, calls: u64) -> MockServer {
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

/// The triage cases document with every label of `question` taken out, so
/// the question is asked and never graded. Fails when no line was labelled
/// that way, so a fixture that changed is not a silent no-op.
pub fn cases_without_labels(question: &str) -> String {
    let text = std::fs::read_to_string(TRIAGE_CASES).unwrap();
    let label = format!("{question}:");
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim_start().starts_with(&label))
        .collect();
    assert!(
        kept.len() < text.lines().count(),
        "no `{label}` label in the triage cases"
    );
    let mut cases = kept.join("\n");
    cases.push('\n');
    cases
}
