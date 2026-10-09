//! The `jud` command line, run as a subprocess: a Rubric file and the JSON
//! state on stdin in, verdicts out, with every refusal on stderr and a
//! non-zero status. The backend is a wiremock server standing in for any
//! System One server, reached through `TYPESAFE_BASE_URL` or the user's
//! configuration file; no test needs a key that works or the network.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::collections::HashSet;
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

/// Every `TYPESAFE_*` and `JUD_*` variable in this process: the host's key,
/// server or recordings, and any other one jud would warn it does not read.
fn own_variables() -> Vec<String> {
    std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .filter(|name| name.starts_with("TYPESAFE_") || name.starts_with("JUD_"))
        .collect()
}

/// Run `jud` with `args`, `stdin` and only the environment given: the
/// crate's variables are cleared first, so the host's key never leaks in.
fn jud(args: &[&str], stdin: &str, env: &[(&str, &str)], config_home: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    command
        .args(args)
        .env("XDG_CONFIG_HOME", config_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in own_variables() {
        command.env_remove(name);
    }
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
    // A document of another kind is named as such, never as an unknown kind.
    let out = jud(&[CASES], STATE, &[("TYPESAFE_API_KEY", "test-key")], &home);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(
        err.contains("is not a valid Rubric: a Cases document, not a Rubric"),
        "{err}"
    );
    assert!(!err.contains("not a document kind"), "{err}");
}

/// `jud lower` and `jud check` read files a person is still writing, so a
/// refusal names the file, what was expected of it and, for a file that is
/// not a `.jud` document at all, where the envelope is described.
#[test]
fn lower_and_check_name_the_file_and_what_was_expected() {
    let home = config_home();
    let out = jud(&["lower", CASES], "", &[], &home);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(err.contains(CASES), "{err}");
    assert!(
        err.contains("is not a valid Rubric: a Cases document, not a Rubric"),
        "{err}"
    );
    assert!(!err.contains("not a document kind"), "{err}");

    let out = jud(&["lower", "/nonexistent/rubric.jud"], "", &[], &home);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("cannot read rubric /nonexistent/rubric.jud"),
        "{}",
        stderr(&out)
    );

    // `--cases` given a rubric: the same refusal, the other way round.
    let out = jud(&["lower", RUBRIC, "--cases", RUBRIC], "", &[], &home);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("is not a valid Cases: a Rubric document, not a Cases"),
        "{}",
        stderr(&out)
    );

    let out = jud(&["lower", RUBRIC, "--state", "{not json"], "", &[], &home);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("--state is not JSON"),
        "{}",
        stderr(&out)
    );

    // A YAML file that is not a document at all: `check` says so with the
    // pointer, and counts it as refused.
    let plain = home.join("config.yaml");
    std::fs::write(
        &plain,
        "base_url: http://127.0.0.1:11434\nmodel: tev1:0.8b\n",
    )
    .unwrap();
    let out = jud(&["check", plain.to_str().unwrap()], "", &[], &home);
    assert_eq!(out.status.code(), Some(2));
    let text = stdout(&out);
    assert!(text.contains("the document has no `apiVersion` (not a jud/v1.3 document; docs/reference/jud-format.md has the envelope)"), "{text}");
    assert!(text.contains("1 documents, 1 refused"), "{text}");
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
    // Nothing set that jud does not read, so no list of them.
    assert!(config.get("ignored_environment").is_none(), "{config}");
}

/// A misspelt variable is why a run asks a server it was not meant to: with
/// `JUD_BASE_URL` set in place of `TYPESAFE_BASE_URL`, the hosted API would
/// be asked and billed without a word. Every `TYPESAFE_*` or `JUD_*`
/// variable jud does not read is named, by `jud config` and before a call,
/// and the call still goes where the variables jud does read say.
#[tokio::test]
async fn a_variable_jud_does_not_read_is_named_before_the_call() {
    let server = backend(200, answers(), 1).await;
    let home = config_home();
    let uri = server.uri();
    let env = [
        ("TYPESAFE_API_KEY", "test-key"),
        ("TYPESAFE_BASE_URL", uri.as_str()),
        ("JUD_BASE_URL", "http://127.0.0.1:9"),
        ("TYPESAFE_MODEL", "jev-preview"),
    ];

    let out = jud(&["config"], "", &env, &home);
    let config: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(
        config["ignored_environment"],
        json!(["JUD_BASE_URL", "TYPESAFE_MODEL"])
    );

    let out = jud(&[RUBRIC], STATE, &env, &home);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&out)).unwrap()["desk"]["key"],
        "billing"
    );
    let warnings = stderr(&out);
    let lines: Vec<&str> = warnings.lines().collect();
    assert_eq!(lines.len(), 2, "{warnings}");
    for (line, name) in lines.iter().zip(["JUD_BASE_URL", "TYPESAFE_MODEL"]) {
        assert!(
            line.starts_with(&format!("jud: {name} is set but jud does not read it")),
            "{line}"
        );
        assert!(line.contains("TYPESAFE_BASE_URL"), "{line}");
        // Where the model comes from instead, for `TYPESAFE_MODEL` above all.
        assert!(line.contains("`model` in "), "{line}");
        assert!(
            line.ends_with(&format!(
                "asking jev-latest at {}, the base URL from environment",
                server.uri()
            )),
            "{line}"
        );
    }
    // The value is never printed: a misspelt key variable holds a key.
    let out = jud(
        &["config"],
        "",
        &[("TYPESAFE_APIKEY", "sk-secret-value")],
        &home,
    );
    assert!(!stdout(&out).contains("sk-secret-value"));
    assert!(stdout(&out).contains("TYPESAFE_APIKEY"), "{}", stdout(&out));
}

/// `jud record --dry-run` is where a run's target is checked, so it names
/// what the run would ignore, though it asks nothing and needs no key.
#[test]
fn a_dry_run_names_a_variable_jud_does_not_read() {
    let out_dir = std::env::temp_dir().join(format!(
        "jud-cli-dry-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let out = jud(
        &[
            "record",
            RUBRIC,
            CASES,
            "--out",
            out_dir.to_str().unwrap(),
            "--dry-run",
        ],
        "",
        &[("JUD_BASE_URL", "http://127.0.0.1:9")],
        &config_home(),
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).starts_with("jud: JUD_BASE_URL is set but jud does not read it"),
        "{}",
        stderr(&out)
    );
    assert!(!out_dir.exists(), "a dry run writes nothing");
}

/// Under a replay nothing is asked or billed, so nothing is named.
#[test]
fn a_replay_names_no_variable() {
    let root = env!("CARGO_MANIFEST_DIR");
    let recordings = format!("{root}/examples/recordings/jud_calibration");
    let recorded = r#"{"message": "This is the third time I'm writing. I was charged twice last month and nobody has refunded me. Fix it today or I cancel."}"#;
    let out = jud(
        &["--replay", &recordings, RUBRIC],
        recorded,
        &[("JUD_BASE_URL", "http://127.0.0.1:9")],
        &config_home(),
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
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

/// The command tree as the binary itself describes it: every subcommand
/// named by `jud --help`, and for each (the root included) every long flag
/// its `--help` lists. Read from the help rather than written out, so a
/// subcommand or a flag added later is covered without this test moving.
fn command_tree(config_home: &Path) -> Vec<(Vec<String>, Vec<String>)> {
    fn section<'a>(help: &'a str, heading: &str) -> impl Iterator<Item = &'a str> {
        help.lines()
            .skip_while(move |l| l.trim_end() != heading)
            .skip(1)
            .take_while(|l| l.starts_with("  "))
    }
    fn long_flags(help: &str) -> Vec<String> {
        section(help, "Options:")
            .filter_map(|l| l.split_whitespace().find(|w| w.starts_with("--")))
            .map(|w| w.trim_end_matches(',').to_owned())
            .collect()
    }
    let root = jud(&["--help"], "", &[], config_home);
    assert!(root.status.success(), "{}", stderr(&root));
    let root_help = stdout(&root);
    let mut tree = vec![(Vec::new(), long_flags(&root_help))];
    for line in section(&root_help, "Commands:") {
        let name = line.split_whitespace().next().unwrap().to_owned();
        // `jud help <sub>` rather than `jud <sub> --help`: clap's own `help`
        // command takes no flag, and this form covers it too.
        let out = jud(&["help", &name], "", &[], config_home);
        assert!(out.status.success(), "`jud help {name}`: {}", stderr(&out));
        tree.push((vec![name], long_flags(&stdout(&out))));
    }
    tree
}

/// The names a generated completion script offers, as opposed to the ones
/// it merely mentions: `script.contains("check")` would pass on the word in
/// any description. Each generator has its own structure, so each has its
/// own reader.
fn completion_candidates(shell: &str, script: &str) -> HashSet<String> {
    fn ident(s: &str) -> String {
        s.chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect()
    }
    let mut out = HashSet::new();
    for line in script.lines() {
        let line = line.trim();
        match shell {
            // One `opts="config check … --help --version"` per command node.
            "bash" => {
                if let Some(list) = line
                    .strip_prefix("opts=\"")
                    .and_then(|r| r.strip_suffix('"'))
                {
                    out.extend(list.split_whitespace().map(str::to_owned));
                }
            }
            // `'check:desc'`, `'--state=[desc]'`, `'(--cases)--state-file=[desc]'`.
            "zsh" => {
                if let Some(rest) = line.strip_prefix('\'') {
                    let rest = match rest.strip_prefix('(') {
                        Some(r) => r.split_once(')').map_or(r, |(_, tail)| tail),
                        None => rest,
                    };
                    let name = ident(rest.trim_start_matches('*'));
                    if !name.is_empty() {
                        out.insert(name);
                    }
                }
            }
            // `-a "check"` offers a subcommand, `-l state` a long flag; the
            // description after ` -d ` is prose and is cut off first.
            "fish" => {
                let head = line.split_once(" -d ").map_or(line, |(h, _)| h);
                out.extend(head.split(" -a \"").skip(1).map(ident));
                out.extend(
                    head.split(" -l ")
                        .skip(1)
                        .map(|p| format!("--{}", ident(p))),
                );
            }
            // `cand check 'desc'` / `cand --state 'desc'`.
            "elvish" => {
                if let Some(rest) = line.strip_prefix("cand ") {
                    out.insert(ident(rest));
                }
            }
            // `[CompletionResult]::new('check', 'check', …)`.
            "powershell" => {
                if let Some((_, rest)) = line.split_once("::new('") {
                    out.insert(ident(rest));
                }
            }
            other => panic!("no candidate reader for {other}"),
        }
    }
    out
}

const SHELLS: [&str; 5] = ["bash", "zsh", "fish", "elvish", "powershell"];

/// Why `completion` is a command rather than scripts checked in: it is
/// generated from the tree clap parses with, so it cannot drift. Asserted
/// directly, every subcommand and every long flag offered by every shell,
/// rather than as a snapshot that would need updating on every change.
#[test]
fn the_completion_script_offers_every_subcommand_and_flag_the_binary_has() {
    let home = config_home();
    let tree = command_tree(&home);
    let subcommands = tree.len() - 1;
    assert!(subcommands >= 4, "walked too little of the tree: {tree:?}");
    for shell in SHELLS {
        let out = jud(&["completion", shell], "", &[], &home);
        assert!(out.status.success(), "{shell}: {}", stderr(&out));
        let script = stdout(&out);
        assert!(
            script.lines().count() > 20,
            "{shell} produced a suspiciously short script"
        );
        let offered = completion_candidates(shell, &script);
        for (path, flags) in &tree {
            for name in path.iter().chain(flags.iter()) {
                assert!(
                    offered.contains(name),
                    "{shell} completion does not offer `{name}` (from `jud {}`): {offered:?}",
                    path.join(" ")
                );
            }
        }
    }
}

/// The names being in the script is not enough: clap's zsh script put the
/// optional RUBRIC before the subcommand, so `jud eval <TAB>` read `eval` as
/// the rubric and never completed eval's own arguments. The subcommand slot
/// must be the first positional, offering subcommands and files, and the
/// dispatch must read it from there.
#[test]
fn the_zsh_script_completes_a_subcommand_s_own_arguments() {
    let home = config_home();
    let out = jud(&["completion", "zsh"], "", &[], &home);
    assert!(out.status.success(), "{}", stderr(&out));
    let script = stdout(&out);
    assert!(
        !script.contains("'::rubric -- "),
        "a rubric slot before the subcommand"
    );
    assert!(
        script.contains("\":: :{_jud_commands; _files}\""),
        "the first slot offers subcommands and files"
    );
    assert!(
        script.contains("case $line[1] in") && !script.contains("$line[2]"),
        "the dispatch reads the subcommand from the first slot"
    );
}

/// A shell clap cannot generate for is a usage error that names the ones it
/// can, as is no shell at all; neither is a backend failure.
#[test]
fn an_unknown_shell_is_a_usage_error_naming_the_known_ones() {
    let home = config_home();
    let out = jud(&["completion", "tcsh"], "", &[], &home);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(err.contains("tcsh"), "{err}");
    for shell in SHELLS {
        assert!(
            err.contains(shell),
            "the possible values should name {shell}: {err}"
        );
    }
    assert!(stdout(&out).is_empty(), "nothing on stdout for an error");
    let none = jud(&["completion"], "", &[], &home);
    assert_eq!(none.status.code(), Some(2));
    assert!(stderr(&none).contains("<SHELL>"), "{}", stderr(&none));
}

/// The script is text generation only: no key, no configuration file, no
/// stdin and no network are consulted, so a shell profile can source it
/// before anything is set up.
#[test]
fn completion_needs_no_configuration_and_reads_no_stdin() {
    let home = config_home();
    std::fs::write(home.join("jud/config.yaml"), "not: [valid").unwrap();
    let out = jud(&["completion", "bash"], "", &[], &home);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("complete -F _jud"),
        "{}",
        stdout(&out)
    );
}

/// `--replay DIR` (or `JUD_REPLAY`) answers from the crate's recordings
/// instead of a server: the example rubric and a recorded case produce the
/// verdicts of the recorded answer with no key, no configuration and no
/// network; a state nobody recorded is a backend failure (1), not a guess,
/// and a directory that does not exist is a usage error (2).
#[test]
fn replay_answers_from_recordings_without_a_server() {
    let home = config_home();
    let root = env!("CARGO_MANIFEST_DIR");
    let rubric = format!("{root}/examples/jud/triage.jud");
    let recordings = format!("{root}/examples/recordings/jud_calibration");
    let recorded = r#"{"message": "This is the third time I'm writing. I was charged twice last month and nobody has refunded me. Fix it today or I cancel."}"#;

    let out = jud(&["--replay", &recordings, &rubric], recorded, &[], &home);
    assert!(out.status.success(), "{}", stderr(&out));
    let verdicts: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(verdicts["desk"]["key"], "billing");
    assert_eq!(verdicts["tone"]["label"], "angry");
    assert_eq!(verdicts["actionable"]["verdict"], "yes");

    let by_env = jud(&[&rubric], recorded, &[("JUD_REPLAY", &recordings)], &home);
    assert_eq!(stdout(&by_env), stdout(&out), "{}", stderr(&by_env));

    let unrecorded = jud(
        &["--replay", &recordings, &rubric],
        r#"{"message": "hi"}"#,
        &[],
        &home,
    );
    assert_eq!(unrecorded.status.code(), Some(1));
    assert!(
        stderr(&unrecorded).contains("no recording"),
        "{}",
        stderr(&unrecorded)
    );

    let missing = jud(
        &["--replay", "/nonexistent/recordings", &rubric],
        recorded,
        &[],
        &home,
    );
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        stderr(&missing).contains("cannot replay from"),
        "{}",
        stderr(&missing)
    );
}

/// The README's command-line example is a transcript of the binary: the
/// state after `$ cat event.json` piped into `jud triage.jud` prints exactly
/// the verdicts the README shows, replayed from the example recordings.
#[test]
fn the_readme_shows_what_the_binary_prints() {
    let readme = include_str!("../README.md");
    let block = readme
        .split("```sh\n$ cat event.json\n")
        .nth(1)
        .expect("README.md has the `$ cat event.json` transcript")
        .split("\n```")
        .next()
        .unwrap();
    let (state, rest) = block
        .split_once("\n$ cat event.json | jud triage.jud\n")
        .expect("the transcript pipes event.json into `jud triage.jud`");
    let home = config_home();
    let root = env!("CARGO_MANIFEST_DIR");
    let rubric = format!("{root}/examples/jud/triage.jud");
    let recordings = format!("{root}/examples/recordings/jud_calibration");
    let out = jud(&["--replay", &recordings, &rubric], state, &[], &home);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out).trim_end(),
        rest.trim_end(),
        "README.md's transcript must be what `jud` prints for that state; regenerate it from the binary"
    );
}

/// Run `jud ARGS` with its stdout pipe closed at once, as `jud ... | true`
/// does, and give the status and what it wrote to stderr.
fn with_stdout_closed(args: &[&str]) -> (Option<i32>, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jud"));
    for name in own_variables() {
        command.env_remove(name);
    }
    let mut child = command
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("XDG_CONFIG_HOME", config_home())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    (out.status.code(), stderr(&out))
}

/// `check`, `lower` and `config` print their result a line at a time; a
/// reader that has gone is not an error and never a panic (status 101), and
/// the status still says what the command found.
#[test]
fn a_closed_stdout_never_panics_the_reader_commands() {
    let state = r#"{"message": "x"}"#;
    for (args, status) in [
        (vec!["check", RUBRIC, CASES], 0),
        (vec!["check", "Cargo.toml"], 2),
        (vec!["lower", RUBRIC, "--state", state], 0),
        (vec!["config"], 0),
    ] {
        let (code, err) = with_stdout_closed(&args);
        assert_eq!(code, Some(status), "jud {args:?}: {err}");
        assert!(!err.contains("panicked"), "jud {args:?}: {err}");
    }
}
