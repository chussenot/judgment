//! The documentation's code is held to the code. Every Rust block a tutorial
//! or the README shows is an `examples/` file, compared verbatim; every whole
//! `.jud` document or configuration file a page shows is a file under
//! `examples/`, named in an HTML comment before the fence, compared verbatim
//! (and read by `jud check` in `mise run jud:check`); every command
//! transcript is reproduced by the binary, offline. A guide's Rust
//! fragments are compiled as documentation tests instead (`src/lib.rs`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn page(path: &str) -> String {
    std::fs::read_to_string(Path::new(ROOT).join(path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

/// The part of an example between its README markers, as the rust block a
/// page must hold.
fn example_block(name: &str) -> String {
    let example = page(&format!("examples/{name}.rs"));
    let begin = example
        .find("// README:BEGIN\n")
        .unwrap_or_else(|| panic!("examples/{name}.rs marks the page's part with // README:BEGIN"));
    let end = example.find("// README:END").unwrap_or_else(|| {
        panic!("examples/{name}.rs marks the end of the page's part with // README:END")
    });
    example[begin + "// README:BEGIN\n".len()..end]
        .trim_end()
        .to_owned()
}

/// Where `text` holds `name`'s block, verbatim, as a rust fence.
fn position_of(text: &str, where_: &str, name: &str) -> usize {
    let block = format!("```rust\n{}\n```", example_block(name));
    text.find(&block).unwrap_or_else(|| {
        panic!("{where_} must contain examples/{name}.rs between its README markers, verbatim, as a rust block")
    })
}

#[test]
fn the_readme_opens_with_the_quickstart() {
    let readme = page("README.md");
    let quickstart = position_of(&readme, "README.md", "quickstart");
    let first_block = readme.find("```rust").expect("README.md has a rust block");
    assert_eq!(
        quickstart, first_block,
        "the quickstart is the README's first code block"
    );
}

#[test]
fn the_rust_tutorial_holds_its_three_examples_in_order() {
    let tutorial = page("docs/start/first-decision-rust.md");
    let where_ = "docs/start/first-decision-rust.md";
    let quickstart = position_of(&tutorial, where_, "quickstart");
    let rubric = position_of(&tutorial, where_, "jud_quickstart");
    let live = position_of(&tutorial, where_, "jud_live");
    assert!(
        quickstart < rubric && rubric < live,
        "the tutorial runs the Fake, then the rubric, then the client"
    );
}

/// Every dependency line a page shows pins the minor Cargo.toml declares.
#[test]
fn the_dependency_line_names_this_minor() {
    let minor = format!(
        "\"{}.{}\"",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
    );
    for path in [
        "README.md",
        "docs/start/install.md",
        "docs/reference/crate.md",
        "docs/start/first-decision-rust.md",
    ] {
        let text = page(path);
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| l.trim_start().starts_with("judgment = "))
            .collect();
        assert!(
            !lines.is_empty(),
            "{path} shows a `judgment = ` dependency line"
        );
        for line in lines {
            assert!(
                line.contains(&minor),
                "{path} pins another version than {minor}: {line}"
            );
        }
    }
}

/// Every `<!-- file: PATH -->` comment before a fence names a file whose
/// content is the fence's body, byte for byte.
#[test]
fn every_document_on_a_page_is_the_file_it_names() {
    let mut seen = 0;
    for path in markdown_pages() {
        let text = std::fs::read_to_string(&path).unwrap();
        let rel = path.strip_prefix(ROOT).unwrap().display().to_string();
        let mut rest = text.as_str();
        while let Some(at) = rest.find("<!-- file: ") {
            rest = &rest[at + "<!-- file: ".len()..];
            let (file, after) = rest
                .split_once(" -->\n")
                .expect("a file comment closes with -->");
            let fence = after
                .split_once('\n')
                .expect("a fence follows the file comment")
                .1;
            let body = fence.split("\n```").next().unwrap();
            let expected = page(file);
            assert_eq!(
                format!("{body}\n"),
                expected,
                "{rel} shows {file}: the fence after `<!-- file: {file} -->` must be the file, verbatim"
            );
            seen += 1;
            rest = after;
        }
    }
    assert!(
        seen >= 6,
        "the pages hold at least the documents the audit listed, found {seen}"
    );
}

fn markdown_pages() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    let mut out = vec![Path::new(ROOT).join("README.md")];
    walk(&Path::new(ROOT).join("docs"), &mut out);
    out
}

/// The fence after `<!-- transcript: LABEL -->` on a page.
fn transcript(path: &str, label: &str) -> String {
    let text = page(path);
    let marker = format!("<!-- transcript: {label} -->\n");
    let at = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{path} has a `{label}` transcript"));
    let after = &text[at + marker.len()..];
    let fence = after.split_once('\n').unwrap().1;
    fence.split("\n```").next().unwrap().to_owned()
}

fn config_home() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("jud-docs-{}-{n}", std::process::id()));
    std::fs::create_dir_all(dir.join("jud")).unwrap();
    dir
}

fn jud(args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Output {
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

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn assert_transcript(path: &str, label: &str, args: &[&str], stdin: &str, env: &[(&str, &str)]) {
    let out = jud(args, stdin, env);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        stdout(&out).trim_end(),
        transcript(path, label).trim_end(),
        "{path}'s `{label}` transcript must be what `jud {}` prints; regenerate it from the binary",
        args.join(" ")
    );
}

#[test]
fn the_configuration_page_shows_jud_config_with_nothing_set() {
    assert_transcript(
        "docs/reference/configuration.md",
        "env -u TYPESAFE_API_KEY -u TYPESAFE_BASE_URL XDG_CONFIG_HOME=/nonexistent jud config",
        &["config"],
        "",
        &[("XDG_CONFIG_HOME", "/nonexistent")],
    );
}

#[test]
fn the_guides_show_what_jud_check_prints() {
    assert_transcript(
        "docs/guides/write-a-rubric.md",
        "jud check examples/jud/screening.jud",
        &["check", "examples/jud/screening.jud"],
        "",
        &[],
    );
    assert_transcript(
        "docs/guides/label-cases.md",
        "jud check examples/jud/screening.jud examples/jud/screening-cases.jud",
        &[
            "check",
            "examples/jud/screening.jud",
            "examples/jud/screening-cases.jud",
        ],
        "",
        &[],
    );
    assert_transcript(
        "docs/start/first-decision-cli.md",
        "jud check examples/jud/triage.jud examples/jud/triage-cases.jud",
        &[
            "check",
            "examples/jud/triage.jud",
            "examples/jud/triage-cases.jud",
        ],
        "",
        &[],
    );
}

/// The tutorial's replayed verdicts are the binary's for that state, from the
/// example recordings, with no key and no network.
#[test]
fn the_cli_tutorial_replays_the_recorded_answer() {
    let state = "{\"message\": \"This is the third time I'm writing. I was charged twice last month and nobody has refunded me. Fix it today or I cancel.\"}";
    assert_transcript(
        "docs/start/first-decision-cli.md",
        "replay refund-angry",
        &[
            "--replay",
            "examples/recordings/jud_calibration",
            "examples/jud/triage.jud",
        ],
        state,
        &[],
    );
}
