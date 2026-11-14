//! The `jud` plugin's two hooks (`plugins/jud/hooks/`), run as the processes
//! Claude Code runs: the tool call as JSON on stdin, a decision on stdout or
//! a status and stderr out. A hook is the one part of the plugin that acts
//! without the agent choosing to, so what it says has to be true: a deny the
//! host can parse whatever the path holds, and a "refused" only when the
//! crate's reader refused.
//!
//! `PATH` is the test's own. The bare one holds `sh` and the tools the
//! scripts call and nothing else, so the branch without `jq` and the one
//! with no `jud` always run; the host's is added where the `jq` branch is
//! wanted, which CI's runners take. The reader is the crate's own binary, so
//! the summary line the post-write hook reads is the one `jud check` prints.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::ffi::OsString;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::LazyLock;

use serde_json::{Value, json};

const HOOKS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/plugins/jud/hooks");
const RUBRIC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/triage.jud");

/// File names JSON has to escape: the deny carries the path, and the branch
/// without `jq` reads it out of the tool call by hand.
const NAMES: [&str; 4] = [
    "plain.jud",
    "a \"quoted\" name.jud",
    "back\\slash.jud",
    "both \"\\\" at once.jud",
];

/// A document the reader refuses: a rubric with nothing in it.
const REFUSED: &str = "apiVersion: jud/v1\nkind: Rubric\n";

/// The directories a test puts on `PATH`, and where it writes.
struct Rig {
    /// `sh` and the tools the scripts call: no `jq`, no `jud`, no `cargo`.
    tools: PathBuf,
    /// The crate's own `jud`.
    jud: PathBuf,
    /// A `jud` that fails for a reason of its own, as a shim with no version
    /// or a crash does.
    broken: PathBuf,
    /// A `cargo` that leaves a line in `$HOOK_LOG` when it is run.
    cargo: PathBuf,
    work: PathBuf,
}

/// Built once and before any test starts a process: a script written while
/// another thread forks stays open in that child, and running it then fails
/// as "text file busy".
static RIG: LazyLock<Rig> = LazyLock::new(|| {
    let root = std::env::temp_dir().join(format!("jud-plugin-hooks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dir = |name: &str| {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    };
    let rig = Rig {
        tools: dir("tools"),
        jud: dir("jud"),
        broken: dir("broken"),
        cargo: dir("cargo"),
        work: dir("work"),
    };
    for tool in ["sh", "cat", "sed", "head", "grep", "tr", "dirname"] {
        let found = host_path()
            .into_iter()
            .map(|dir| dir.join(tool))
            .find(|path| path.is_file())
            .unwrap_or_else(|| panic!("no {tool} on PATH"));
        symlink(found, rig.tools.join(tool)).unwrap();
    }
    symlink(env!("CARGO_BIN_EXE_jud"), rig.jud.join("jud")).unwrap();
    script(
        &rig.broken.join("jud"),
        "case \"$1\" in --version) echo 'jud 0.17.0'; exit 0 ;; esac\necho 'error[E0432]: unresolved import' >&2\nexit 101",
    );
    script(
        &rig.cargo.join("cargo"),
        "echo \"cargo $*\" >> \"$HOOK_LOG\"\necho 'error: could not compile `judgment`' >&2\nexit 101",
    );
    rig
});

fn script(path: &Path, body: &str) {
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn host_path() -> Vec<PathBuf> {
    std::env::var_os("PATH").map_or_else(Vec::new, |path| std::env::split_paths(&path).collect())
}

impl Rig {
    /// `first`, then the bare tools.
    fn bare(&self, first: &[&Path]) -> OsString {
        let dirs = first.iter().copied().chain([self.tools.as_path()]);
        std::env::join_paths(dirs).unwrap()
    }

    /// `first`, then the host's own `PATH`: with `jq` where the host has it.
    fn host(first: &[&Path]) -> OsString {
        let dirs = first.iter().map(|dir| dir.to_path_buf()).chain(host_path());
        std::env::join_paths(dirs).unwrap()
    }

    /// A file of this test's own under the working directory.
    fn write(&self, group: &str, name: &str, text: &str) -> PathBuf {
        let dir = self.work.join(group);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(name);
        std::fs::write(&file, text).unwrap();
        file
    }
}

/// Run a hook as `hooks.json` does, on an Edit of `file`, with `path` as its
/// `PATH` and nothing else of the host's environment.
fn hook(name: &str, file: &Path, path: &OsString, log: &Path) -> Output {
    let call = json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Edit",
        "tool_input": { "file_path": file.to_str().unwrap(), "old_string": "a", "new_string": "b" },
    });
    let mut child = Command::new(Path::new(HOOKS).join(name))
        .env_clear()
        .env("PATH", path)
        .env("HOME", &RIG.work)
        .env("XDG_CONFIG_HOME", &RIG.work)
        .env("HOOK_LOG", log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(call.to_string().as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The package ships without `plugins/` (`exclude` in Cargo.toml), and there
/// is then no hook to hold.
fn plugin_is_here() -> bool {
    let here = Path::new(HOOKS).is_dir();
    if !here {
        eprintln!("skipped: {HOOKS} is not in this tree");
    }
    here
}

#[test]
fn the_guard_denies_an_edit_of_a_recording_in_json_the_host_can_read() {
    if !plugin_is_here() {
        return;
    }
    let rig = &*RIG;
    let log = rig.work.join("guard.log");
    for name in NAMES {
        let file = rig.write("guard", name, "apiVersion: jud/v1\nkind: Recording\n");
        for path in [rig.bare(&[]), Rig::host(&[])] {
            let out = hook("guard-recording.sh", &file, &path, &log);
            assert!(out.status.success(), "{name}: {}", text(&out.stderr));
            let decision: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
                panic!("{name}: the deny is not JSON ({e}): {}", text(&out.stdout))
            });
            let decision = &decision["hookSpecificOutput"];
            assert_eq!(decision["hookEventName"], "PreToolUse", "{name}");
            assert_eq!(decision["permissionDecision"], "deny", "{name}");
            let reason = decision["permissionDecisionReason"].as_str().unwrap();
            assert!(
                reason.starts_with(&format!("{} is a Recording", file.display())),
                "{name}: {reason}"
            );
        }
    }
}

#[test]
fn the_guard_lets_through_what_is_not_an_existing_recording() {
    if !plugin_is_here() {
        return;
    }
    let rig = &*RIG;
    let log = rig.work.join("allowed.log");
    let allowed = [
        // A rubric, by a name the branch without `jq` has to unescape.
        rig.write(
            "allowed",
            "a \"quoted\" rubric.jud",
            "apiVersion: jud/v1\nkind: Rubric\n",
        ),
        // A recording that is not a `.jud` document.
        rig.write("allowed", "notes.yaml", "kind: Recording\n"),
        // A recording not written yet: the hand-written fixture.
        rig.work.join("allowed").join("new.jud"),
    ];
    for file in &allowed {
        for path in [rig.bare(&[]), Rig::host(&[])] {
            let out = hook("guard-recording.sh", file, &path, &log);
            assert!(out.status.success(), "{}", text(&out.stderr));
            assert!(
                out.stdout.is_empty(),
                "{}: {}",
                file.display(),
                text(&out.stdout)
            );
        }
    }
}

#[test]
fn the_post_write_hook_says_nothing_and_builds_nothing_with_no_jud_on_path() {
    if !plugin_is_here() {
        return;
    }
    let rig = &*RIG;
    let log = rig.work.join("no-jud.log");
    // The script sits in a judgment checkout, which is where it used to build.
    let out = hook(
        "check-after-write.sh",
        Path::new(RUBRIC),
        &rig.bare(&[&rig.cargo]),
        &log,
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
    assert!(
        !log.exists(),
        "the hook ran {}",
        std::fs::read_to_string(&log).unwrap_or_default()
    );
}

#[test]
fn the_post_write_hook_is_silent_on_a_document_that_reads() {
    if !plugin_is_here() {
        return;
    }
    let rig = &*RIG;
    let log = rig.work.join("reads.log");
    let out = hook(
        "check-after-write.sh",
        Path::new(RUBRIC),
        &rig.bare(&[&rig.jud]),
        &log,
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
}

#[test]
fn the_post_write_hook_hands_the_agent_what_the_reader_refused() {
    if !plugin_is_here() {
        return;
    }
    let rig = &*RIG;
    let log = rig.work.join("refused.log");
    for name in NAMES {
        let file = rig.write("refused", name, REFUSED);
        for path in [rig.bare(&[&rig.jud]), Rig::host(&[&rig.jud])] {
            let out = hook("check-after-write.sh", &file, &path, &log);
            let stderr = text(&out.stderr);
            assert_eq!(out.status.code(), Some(2), "{name}: {stderr}");
            assert!(
                stderr.starts_with(&format!(
                    "jud check {} refused the document",
                    file.display()
                )),
                "{name}: {stderr}"
            );
            let named = format!("error     {}: ", file.display());
            assert!(
                stderr.lines().any(|line| line.starts_with(&named)),
                "{name}: {stderr}"
            );
            assert!(
                stderr.ends_with("1 documents, 1 refused\n"),
                "{name}: {stderr}"
            );
        }
    }
}

#[test]
fn the_post_write_hook_does_not_call_a_jud_that_failed_a_refusal() {
    if !plugin_is_here() {
        return;
    }
    let rig = &*RIG;
    let log = rig.work.join("broken.log");
    let file = rig.write("broken", "rubric.jud", REFUSED);
    let out = hook(
        "check-after-write.sh",
        &file,
        &rig.bare(&[&rig.broken]),
        &log,
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
}
