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
    jud_in(Path::new(ROOT), args, stdin, env)
}

/// `jud ARGS` run in `cwd`, with no key, no server and a scratch configuration.
fn jud_in(cwd: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    command
        .current_dir(cwd)
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

/// The record guide's dry run is what `jud record --dry-run` prints for the
/// screening documents, which have no recordings, with no key. It writes
/// nothing: the directory it names is not there afterwards.
#[test]
fn the_record_guide_shows_a_dry_run() {
    let label = "jud record examples/jud/screening.jud examples/jud/screening-cases.jud --out recordings/screening --dry-run";
    assert_transcript(
        "docs/guides/record-replay-and-test.md",
        label,
        &[
            "record",
            "examples/jud/screening.jud",
            "examples/jud/screening-cases.jud",
            "--out",
            "recordings/screening",
            "--dry-run",
        ],
        "",
        &[],
    );
    assert!(
        !Path::new(ROOT).join("recordings").exists(),
        "a dry run creates no directory"
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

const TRIAGE: &str = "examples/jud/triage.jud";
const TRIAGE_CASES: &str = "examples/jud/triage-cases.jud";
const TRIAGE_RECORDINGS: &str = "examples/recordings/jud_calibration";
const TUNE_GUIDE: &str = "docs/guides/tune-thresholds.md";

/// The label a transcript of `jud eval` over the triage documents has on the
/// tune guide: the command as the page shows it, and what part of its output
/// the block holds.
fn triage_label(command: &str, part: &str) -> String {
    let base = format!("jud {command} {TRIAGE} {TRIAGE_CASES} --replay {TRIAGE_RECORDINGS}");
    if part.is_empty() {
        base
    } else {
        format!("{base} {part}")
    }
}

/// `text` with the value of every `tuned_at` line replaced, since a proposal
/// carries the time of the run that made it. The value itself is checked to
/// be a timestamp, so the mask hides a clock and nothing else.
fn without_the_clock(text: &str) -> String {
    text.lines()
        .map(|line| match line.trim_start().strip_prefix("tuned_at: ") {
            Some(value) => {
                assert!(
                    value.starts_with("\"20") && value.ends_with("Z\""),
                    "tuned_at is an RFC 3339 time in UTC: {line}"
                );
                format!(
                    "{}tuned_at: <time>",
                    &line[..line.len() - line.trim_start().len()]
                )
            }
            None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The tune guide's report is what `jud eval` prints over the triage
/// documents and their recordings, with no key and no network.
#[test]
fn the_tune_guide_shows_the_report_of_jud_eval() {
    assert_transcript(
        TUNE_GUIDE,
        &triage_label("eval", ""),
        &["eval", TRIAGE, TRIAGE_CASES, "--replay", TRIAGE_RECORDINGS],
        "",
        &[],
    );
}

/// A gate that is not met: the whole report on stdout, the section the gate
/// adds at its end, the message on stderr and the exit status 3.
#[test]
fn the_tune_guide_shows_a_gate_that_is_not_met() {
    let out = jud(
        &[
            "eval",
            TRIAGE,
            TRIAGE_CASES,
            "--replay",
            TRIAGE_RECORDINGS,
            "--min-accuracy",
            "0.9",
        ],
        "",
        &[],
    );
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));

    let section = transcript(
        TUNE_GUIDE,
        &triage_label("eval", "--min-accuracy 0.9 (end of stdout)"),
    );
    let report = stdout(&out);
    assert!(
        report
            .trim_end()
            .ends_with(&format!("\n\n{}", section.trim_end())),
        "the page's section must end the report, after a blank line:\n{report}"
    );
    assert_eq!(
        stderr(&out).trim_end(),
        transcript(
            TUNE_GUIDE,
            &triage_label("eval", "--min-accuracy 0.9 (stderr)")
        )
        .trim_end(),
        "the page's message must be what the gate writes on stderr"
    );
}

/// The proposal on stdout, and the notes `jud tune` writes on stderr, one
/// line for each decision (the tables, which start with spaces, are left out).
#[test]
fn the_tune_guide_shows_what_jud_tune_proposes() {
    let out = jud(
        &["tune", TRIAGE, TRIAGE_CASES, "--replay", TRIAGE_RECORDINGS],
        "",
        &[],
    );
    assert!(out.status.success(), "{}", stderr(&out));

    assert_eq!(
        without_the_clock(stdout(&out).trim_end()),
        without_the_clock(transcript(TUNE_GUIDE, &triage_label("tune", "(stdout)")).trim_end()),
        "the page's proposal must be what `jud tune` prints on stdout, but for the time"
    );
    let notes: Vec<String> = stderr(&out)
        .lines()
        .filter(|line| line.starts_with("jud:"))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        notes.join("\n"),
        transcript(TUNE_GUIDE, &triage_label("tune", "(stderr notes)")).trim_end(),
        "the page's notes must be the `jud:` lines `jud tune` writes on stderr"
    );
}

/// `--out` writes a rubric that `jud check` reads and binds to its cases. Run
/// in a scratch directory holding the example documents and recordings at
/// their paths, so the page's relative paths are the ones the test runs.
#[test]
fn the_tune_guide_shows_the_rubric_tuned_into_a_new_file_checked() {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("jud-docs-tune-{}-{n}", std::process::id()));
    for from in ["examples/jud", TRIAGE_RECORDINGS] {
        copy_tree(&Path::new(ROOT).join(from), &dir.join(from));
    }

    let tuned = jud_in(
        &dir,
        &[
            "tune",
            TRIAGE,
            TRIAGE_CASES,
            "--replay",
            TRIAGE_RECORDINGS,
            "--out",
            "triage-tuned.jud",
        ],
        "",
        &[],
    );
    assert!(tuned.status.success(), "{}", stderr(&tuned));
    assert!(
        stdout(&tuned).is_empty(),
        "--out writes the file and prints no proposal"
    );

    let checked = jud_in(&dir, &["check", "triage-tuned.jud", TRIAGE_CASES], "", &[]);
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert_eq!(
        stdout(&checked).trim_end(),
        transcript(
            TUNE_GUIDE,
            &format!(
                "jud tune --out triage-tuned.jud, then jud check triage-tuned.jud {TRIAGE_CASES}"
            )
        )
        .trim_end()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// `from`'s files, and its directories' files, copied under `to`.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}
