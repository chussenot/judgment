//! What the `jud` subcommand tests share: the binary run as a subprocess in
//! a scratch configuration directory with no key and no `TYPESAFE_*`
//! variable, the example documents, and a scratch directory per test.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// A fresh directory under the system temp dir, removed by nobody: the
/// process id and a counter keep tests apart.
pub fn scratch(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("jud-{label}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A fresh `XDG_CONFIG_HOME`, so no test reads a real configuration file.
pub fn config_home() -> PathBuf {
    let dir = scratch("config");
    std::fs::create_dir_all(dir.join("jud")).unwrap();
    dir
}

/// Run `jud ARGS` with `stdin`, a scratch configuration directory and no
/// `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL` or `JUD_REPLAY` unless `env`
/// sets them.
pub fn jud(args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    command
        .current_dir(ROOT)
        .args(args)
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("TYPESAFE_BASE_URL")
        .env_remove("JUD_REPLAY")
        .env("XDG_CONFIG_HOME", config_home())
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
