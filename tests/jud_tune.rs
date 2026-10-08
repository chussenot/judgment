//! `jud tune` (decision 0021), run as a subprocess: no key and no network.
//! The recordings are the committed ones under
//! `examples/recordings/jud_calibration/`, replayed; a test that needs other
//! documents (a gate with bands, a label flipped, a second model) writes them
//! to a scratch directory, never over the examples.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Output;

use judgment::eval::{request_hash, write_recording};
use judgment::jud::{Cases, Rubric, parse_recording};
use serde_json::{Value, json};
use support::{
    HANDOFF, HANDOFF_CASES, RECORDINGS, TRIAGE, TRIAGE_CASES, code, copy_dir, jud, scratch, stderr,
    stdout,
};

/// `jud tune RUBRIC CASES --replay RECORDINGS` and `extra`.
fn tune(rubric: &str, cases: &str, recordings: &str, extra: &[&str]) -> Output {
    let mut args = vec!["tune", rubric, cases, "--replay", recordings];
    args.extend_from_slice(extra);
    jud(&args, "", &[])
}

/// The triage documents over the committed recordings.
fn tune_triage(extra: &[&str]) -> Output {
    tune(TRIAGE, TRIAGE_CASES, RECORDINGS, extra)
}

fn yaml(text: &str) -> Value {
    serde_saphyr::from_str(text).unwrap()
}

fn path_str(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// The triage rubric's questions with `policy` (indented four spaces under
/// `policy:`) in place of its gates and its tuning.
fn triage_with_policy(policy: &str) -> String {
    let text = std::fs::read_to_string(TRIAGE).unwrap();
    let (questions, _) = text.split_once("\n  # Gates:").unwrap();
    format!("{questions}\n  policy:\n{policy}")
}

/// The triage rubric's questions and no policy at all.
fn triage_without_policy() -> String {
    let text = std::fs::read_to_string(TRIAGE).unwrap();
    let (questions, _) = text.split_once("\n  # Gates:").unwrap();
    format!("{questions}\n")
}

/// `text` with `prefix` before every line, each line newline-terminated.
fn indented(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| [prefix, line, "\n"].concat())
        .collect()
}

/// The triage cases with every `tone` label taken out, so `tone` is asked
/// and never graded.
fn cases_without_tone_labels() -> String {
    let text = std::fs::read_to_string(TRIAGE_CASES).unwrap();
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("tone:"))
        .collect();
    indented(&kept.join("\n"), "")
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

/// A scratch directory holding a rubric with the given policy, and the
/// triage cases as they are.
fn workspace(policy: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = scratch("tune");
    let rubric = write(&dir, "rubric.jud", &triage_with_policy(policy));
    let cases = write(
        &dir,
        "cases.jud",
        &std::fs::read_to_string(TRIAGE_CASES).unwrap(),
    );
    (dir, rubric, cases)
}

/// A guess on every question, none of them what the sweep will say, so a
/// gate left as written and a gate proposed cannot be mistaken for each other.
const GUESSES: &str = "    actionable:
      threshold: 0.9
      note: guess
    desk:
      confidence: 0.9
      fallback: none_of_these
      note: guess
    tone:
      confidence: 0.9
      note: guess
";

/// Every file under `dir` with its bytes, to say nothing was written or
/// changed.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read(&path).unwrap_or_default())
        })
        .collect()
}

/// The fingerprint `jud check` prints after `label`, for the given files.
fn check_fingerprint(files: &[&str], label: &str) -> String {
    let mut args = vec!["check"];
    args.extend_from_slice(files);
    let out = jud(&args, "", &[]);
    assert_eq!(code(&out), 0, "{}{}", stdout(&out), stderr(&out));
    stdout(&out)
        .lines()
        .filter_map(|line| line.trim().strip_prefix(label))
        .map(str::trim)
        .find(|rest| rest.starts_with("sha256:"))
        .unwrap_or_else(|| panic!("no `{label}` fingerprint in {}", stdout(&out)))
        .to_owned()
}

/// The rows of the table that follows the line starting `jud: {heading}`, as
/// whitespace-separated cells, header excluded.
fn table(stderr: &str, heading: &str) -> Vec<Vec<String>> {
    let mut lines = stderr
        .lines()
        .skip_while(|l| !l.starts_with(&format!("jud: {heading}")));
    lines
        .next()
        .unwrap_or_else(|| panic!("no `{heading}` in {stderr}"));
    lines
        .take_while(|l| l.starts_with("  "))
        .skip(1)
        .map(|l| l.split_whitespace().map(str::to_owned).collect())
        .collect()
}

#[test]
fn it_proposes_the_bars_the_calibration_example_writes() {
    let out = tune_triage(&[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    let proposal = yaml(&text);

    // The two blocks, and nothing else, and nothing that is not data.
    let keys: Vec<&str> = proposal
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys.len(), 2, "{text}");
    assert!(
        keys.contains(&"policy") && keys.contains(&"tuning"),
        "{text}"
    );
    let top_level: Vec<&str> = text.lines().filter(|l| !l.starts_with(' ')).collect();
    assert_eq!(top_level, ["policy:", "tuning:"], "{text}");
    assert!(!text.lines().any(|l| l.starts_with('#')), "{text}");

    // The bars `examples/jud_calibration.rs` writes, with the rest of each
    // gate as the rubric wrote it.
    let policy = &proposal["policy"];
    assert_eq!(policy["actionable"]["threshold"], json!(0.55));
    assert_eq!(policy["desk"]["confidence"], json!(0.45));
    assert_eq!(policy["desk"]["fallback"], json!("none_of_these"));
    assert_eq!(
        policy["actionable"]["note"],
        json!("best F1 on 7 labelled cases; ties go to the lower threshold")
    );
    assert_eq!(
        policy["desk"]["note"],
        json!("lowest bar at 95% accuracy; covers 6 of 7 labelled cases")
    );
    let gates: Vec<&str> = policy
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(gates.len(), 3, "every gate of the rubric: {text}");

    // The provenance: the cases by the fingerprint `jud check` prints, the
    // model and the server the recordings carry, and when.
    let tuning = &proposal["tuning"];
    assert_eq!(
        tuning["cases"],
        json!(check_fingerprint(&[TRIAGE_CASES, TRIAGE], "cases"))
    );
    assert_eq!(tuning["model"], json!("jev-1.13.0"));
    assert_eq!(tuning["server"], json!("https://api.typesafe.ai"));
    let tuned_at = tuning["tuned_at"].as_str().unwrap();
    assert!(
        tuned_at.starts_with("20") && tuned_at.ends_with('Z'),
        "{tuned_at}"
    );
    assert_eq!(tuning["labelled"]["actionable"], json!(7));
    assert_eq!(tuning["labelled"]["desk"], json!(7));
}

/// What the sweep makes of `tone` is read off the table printed beside it,
/// not guessed: the lowest bar over at least three cases and 95% accuracy.
#[test]
fn the_tone_proposal_is_the_bar_its_printed_table_says() {
    let out = tune_triage(&[]);
    let err = stderr(&out);
    let rows = table(&err, "tone (score)");
    assert_eq!(rows.len(), 20, "bars 0.00 to 0.95: {err}");
    let acts = |row: &[String]| {
        let covered: usize = row[1].parse().unwrap();
        let accuracy: f64 = row[4].parse().unwrap_or(0.0);
        covered >= 3 && accuracy >= 0.95
    };
    let lowest = rows.iter().find(|row| acts(row)).expect("a bar qualifies");
    let proposed = rows
        .iter()
        .find(|row| row.contains(&"<-".to_owned()))
        .expect("a marked row");
    assert_eq!(proposed[0], lowest[0], "{err}");

    let policy = yaml(&stdout(&out));
    let bar: f64 = lowest[0].parse().unwrap();
    assert_eq!(policy["policy"]["tone"]["confidence"], json!(bar));
    assert_eq!(
        policy["policy"]["tone"]["note"],
        json!(format!(
            "lowest bar at 95% accuracy; covers {} of {} labelled cases",
            lowest[1], 6
        ))
    );
    assert_eq!(policy["tuning"]["labelled"]["tone"], json!(6));
}

#[test]
fn the_tables_and_the_warnings_are_on_stderr_and_stdout_is_only_the_proposal() {
    let out = tune_triage(&[]);
    let (err, text) = (stderr(&out), stdout(&out));
    // One table per primitive, each under the question it is for.
    assert!(
        err.contains(
            "jud: actionable (noul): 7 labelled cases, accuracy 0.86 (95% interval 0.49-0.97)"
        ),
        "{err}"
    );
    assert!(
        err.contains("  threshold  accuracy  precision  recall  f1"),
        "{err}"
    );
    assert!(
        err.contains("jud: desk (choice): 7 labelled cases"),
        "{err}"
    );
    assert!(
        err.contains("  bar   covered  coverage  correct  accuracy"),
        "{err}"
    );
    assert!(err.contains("jud: tone (score): 6 labelled cases"), "{err}");
    // Each proposal, with the number of cases and the interval it rests on.
    assert!(
        err.contains("jud: actionable: propose threshold 0.55 (best F1 on 7 labelled cases"),
        "{err}"
    );
    assert!(err.contains("jud: desk: propose confidence 0.45 (lowest bar at 95% accuracy; covers 6 of 7 labelled cases)"), "{err}");
    // Seven cases is a guess with a number on it, and the warning says so.
    for (question, n) in [("actionable", 7), ("desk", 7), ("tone", 6)] {
        let warning = format!("warning: {question}: {n} labelled cases, accuracy ");
        assert!(err.contains(&warning), "{err}");
    }
    assert!(
        err.contains("a bar read off so few cases is a guess with a number on it"),
        "{err}"
    );
    assert!(err.contains("(95% interval 0.49-0.97)"), "{err}");
    // None of it is on stdout.
    assert!(
        !text.contains("jud:") && !text.contains("warning"),
        "{text}"
    );
    assert!(
        !text.contains("threshold  accuracy") && !text.contains("covered"),
        "{text}"
    );
}

#[test]
fn enough_cases_narrow_the_interval_and_the_warning_goes() {
    // The seven states, eight times each under other ids: the recordings
    // answer a state by its request, so they still answer.
    let mut cases = yaml(&std::fs::read_to_string(TRIAGE_CASES).unwrap());
    let originals = cases["spec"]["cases"].as_array().unwrap().clone();
    let repeated: Vec<Value> = (0..8)
        .flat_map(|k| {
            originals.iter().map(move |case| {
                let mut case = case.clone();
                let id = format!("{}-{k}", case["id"].as_str().unwrap());
                case["id"] = json!(id);
                case
            })
        })
        .collect();
    cases["spec"]["cases"] = Value::Array(repeated);
    let dir = scratch("tune");
    let cases = write(&dir, "cases.jud", &serde_saphyr::to_string(&cases).unwrap());
    let out = tune(TRIAGE, path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("jud: desk (choice): 56 labelled cases"),
        "{err}"
    );
    assert!(!err.contains("warning"), "{err}");
    let proposal = yaml(&stdout(&out));
    assert_eq!(
        proposal["policy"]["desk"]["note"],
        json!("lowest bar at 95% accuracy; covers 48 of 56 labelled cases")
    );
    assert_eq!(proposal["tuning"]["labelled"]["desk"], json!(56));
}

#[test]
fn nothing_is_written_without_out_and_the_inputs_are_not_touched() {
    let dir = scratch("tune");
    let rubric = write(
        &dir,
        "triage.jud",
        &std::fs::read_to_string(TRIAGE).unwrap(),
    );
    let cases = write(
        &dir,
        "cases.jud",
        &std::fs::read_to_string(TRIAGE_CASES).unwrap(),
    );
    let recordings = copy_dir(RECORDINGS);
    let before = (snapshot(&dir), snapshot(&recordings));
    let out = tune(
        path_str(&rubric),
        path_str(&cases),
        path_str(&recordings),
        &[],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(!stdout(&out).is_empty());
    assert_eq!(before, (snapshot(&dir), snapshot(&recordings)));
    assert_eq!(
        std::fs::read(&rubric).unwrap(),
        std::fs::read(TRIAGE).unwrap()
    );
}

#[test]
fn out_writes_a_rubric_jud_check_accepts_and_leaves_the_input_alone() {
    let dir = scratch("tune");
    let rubric = write(
        &dir,
        "triage.jud",
        &std::fs::read_to_string(TRIAGE).unwrap(),
    );
    let out_path = dir.join("tuned.jud");
    let input_before = std::fs::read(&rubric).unwrap();
    let out = tune(
        path_str(&rubric),
        TRIAGE_CASES,
        RECORDINGS,
        &["--out", path_str(&out_path)],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(
        stderr(&out).contains("wrote the tuned rubric to"),
        "{}",
        stderr(&out)
    );
    // The result is the file; stdout carries nothing.
    assert!(stdout(&out).is_empty(), "{}", stdout(&out));
    assert_eq!(
        std::fs::read(&rubric).unwrap(),
        input_before,
        "the input is never rewritten"
    );

    // The tuned rubric is a document `jud check` reads, bound to the cases.
    let check = jud(&["check", path_str(&out_path), TRIAGE_CASES], "", &[]);
    assert_eq!(code(&check), 0, "{}{}", stdout(&check), stderr(&check));
    let text = stdout(&check);
    assert!(text.contains("3 gates (tuned)"), "{text}");
    assert!(text.contains("2 documents, 0 refused"), "{text}");
    let cases = check_fingerprint(&[TRIAGE_CASES], "cases");
    assert!(text.contains(&format!("tuned on  {cases}")), "{text}");

    // The questions are the input's; the policy moved, if only its notes.
    let tuned = path_str(&out_path);
    assert_eq!(
        check_fingerprint(&[tuned], "questions"),
        check_fingerprint(&[path_str(&rubric)], "questions")
    );
    assert_ne!(
        check_fingerprint(&[tuned], "policy"),
        check_fingerprint(&[path_str(&rubric)], "policy")
    );
    // A bar did not move on desk, whose proposal is the committed bar.
    let written = std::fs::read_to_string(&out_path).unwrap();
    let doc = yaml(&written);
    assert_eq!(doc["spec"]["policy"]["desk"]["confidence"], json!(0.45));
    assert_eq!(
        doc["spec"]["policy"]["actionable"]["threshold"],
        json!(0.55)
    );
    assert_eq!(doc["spec"]["tuning"]["model"], json!("jev-1.13.0"));
}

#[test]
fn the_blocks_pasted_under_spec_are_the_policy_out_writes() {
    let (dir, rubric, cases) = workspace(GUESSES);
    let proposal = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&proposal), 0, "{}", stderr(&proposal));
    let tuned = dir.join("out.jud");
    let written = tune(
        path_str(&rubric),
        path_str(&cases),
        RECORDINGS,
        &["--out", path_str(&tuned)],
    );
    assert_eq!(code(&written), 0, "{}", stderr(&written));

    // What a person does with the stdout: paste it under `spec:`.
    let head = triage_without_policy();
    let pasted = indented(&stdout(&proposal), "  ");
    let by_hand = write(&dir, "by-hand.jud", &format!("{head}{pasted}"));
    let (by_hand, tuned) = (path_str(&by_hand), path_str(&tuned));
    assert_eq!(
        check_fingerprint(&[by_hand], "policy"),
        check_fingerprint(&[tuned], "policy")
    );
    assert_ne!(
        check_fingerprint(&[by_hand], "policy"),
        check_fingerprint(&[path_str(&rubric)], "policy"),
        "the guesses moved"
    );
}

#[test]
fn out_is_never_the_input() {
    let dir = scratch("tune");
    let rubric = write(
        &dir,
        "triage.jud",
        &std::fs::read_to_string(TRIAGE).unwrap(),
    );
    let cases = write(
        &dir,
        "cases.jud",
        &std::fs::read_to_string(TRIAGE_CASES).unwrap(),
    );
    let rubric_text = path_str(&rubric).to_owned();
    // The same file spelled another way is the same file.
    let mut spellings = vec![
        (rubric_text.clone(), "rubric"),
        (format!("{}/./triage.jud", dir.display()), "rubric"),
        (path_str(&cases).to_owned(), "cases"),
    ];
    #[cfg(unix)]
    {
        let link = dir.join("link.jud");
        std::os::unix::fs::symlink(&rubric, &link).unwrap();
        spellings.push((path_str(&link).to_owned(), "rubric"));
    }
    let before = snapshot(&dir);
    for (spelling, what) in &spellings {
        let out = tune(
            &rubric_text,
            path_str(&cases),
            RECORDINGS,
            &["--out", spelling],
        );
        assert_eq!(code(&out), 2, "{spelling}: {}", stderr(&out));
        let err = stderr(&out);
        assert!(err.contains("refusing to overwrite the input"), "{err}");
        assert!(err.contains(&format!("is the {what}")), "{err}");
        assert!(stdout(&out).is_empty(), "{}", stdout(&out));
        assert_eq!(snapshot(&dir), before, "{spelling} was written");
    }
}

#[test]
fn out_in_a_missing_directory_is_a_usage_failure() {
    let missing = scratch("tune").join("no").join("such").join("tuned.jud");
    let out = tune_triage(&["--out", path_str(&missing)]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("cannot write"), "{}", stderr(&out));
    assert!(stderr(&out).contains("tuned.jud"), "{}", stderr(&out));
}

#[test]
fn the_target_accuracy_moves_the_confidence_bar() {
    let (_, rubric, cases) = workspace(GUESSES);
    let (rubric, cases) = (path_str(&rubric), path_str(&cases));
    // 6 of 7 desks are right with no bar at all, so 80% asks for no bar.
    let out = tune(rubric, cases, RECORDINGS, &["--target-accuracy", "0.8"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let policy = yaml(&stdout(&out))["policy"].clone();
    assert_eq!(policy["desk"]["confidence"], json!(0.0));
    assert_eq!(
        policy["desk"]["note"],
        json!("lowest bar at 80% accuracy; covers 7 of 7 labelled cases")
    );
    // And the default asks for 95%, which wants the 0.45 bar.
    let out = tune(rubric, cases, RECORDINGS, &[]);
    let policy = yaml(&stdout(&out))["policy"].clone();
    assert_eq!(policy["desk"]["confidence"], json!(0.45));
}

/// A label flipped so that the model is wrong where it is most sure: no bar
/// can then keep full accuracy over three cases, and the gate must stay as
/// it was written, which the guess makes visible.
#[test]
fn a_target_no_bar_reaches_leaves_the_gate_as_written() {
    let (dir, rubric, _) = workspace(GUESSES);
    let flipped = std::fs::read_to_string(TRIAGE_CASES).unwrap().replace(
        "desk: technical\n        tone: annoyed",
        "desk: account\n        tone: annoyed",
    );
    assert_ne!(flipped, std::fs::read_to_string(TRIAGE_CASES).unwrap());
    let cases = write(&dir, "flipped.jud", &flipped);
    let out = tune(
        path_str(&rubric),
        path_str(&cases),
        RECORDINGS,
        &["--target-accuracy", "1.0"],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("jud: desk: no bar reaches 100% accuracy over at least 3 cases; the confidence stays as written"),
        "{err}"
    );
    // The table is still printed, and nothing in it is marked.
    let rows = table(&err, "desk (choice)");
    assert_eq!(rows.len(), 20);
    assert!(
        rows.iter().all(|row| !row.contains(&"<-".to_owned())),
        "{err}"
    );
    let proposal = yaml(&stdout(&out));
    assert_eq!(
        proposal["policy"]["desk"],
        json!({"confidence": 0.9, "fallback": "none_of_these", "note": "guess"})
    );
    // The tuning block names what was tuned, not what was left as written.
    assert!(
        proposal["tuning"]["labelled"].get("desk").is_none(),
        "{proposal}"
    );
    assert_eq!(proposal["tuning"]["labelled"]["actionable"], json!(7));
}

#[test]
fn min_covered_is_how_many_cases_a_bar_must_still_cover() {
    let (_, rubric, cases) = workspace(GUESSES);
    let (rubric, cases) = (path_str(&rubric), path_str(&cases));
    let out = tune(rubric, cases, RECORDINGS, &["--min-covered", "7"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    // desk reaches 95% only over six cases, tone has only six labelled.
    assert!(
        err.contains("jud: desk: no bar reaches 95% accuracy over at least 7 cases"),
        "{err}"
    );
    assert!(
        err.contains("jud: tone: no bar reaches 95% accuracy over at least 7 cases"),
        "{err}"
    );
    let proposal = yaml(&stdout(&out));
    assert_eq!(proposal["policy"]["desk"]["confidence"], json!(0.9));
    assert_eq!(proposal["policy"]["tone"]["confidence"], json!(0.9));
    // The Noul has no such bar to cover; it is proposed as ever.
    assert_eq!(proposal["policy"]["actionable"]["threshold"], json!(0.55));

    // At six, desk's six right answers qualify again, and so do tone's.
    let out = tune(rubric, cases, RECORDINGS, &["--min-covered", "6"]);
    let proposal = yaml(&stdout(&out));
    assert_eq!(proposal["policy"]["desk"]["confidence"], json!(0.45));
    assert_eq!(proposal["policy"]["tone"]["confidence"], json!(0.0));
}

#[test]
fn a_gate_with_bands_gets_its_table_and_no_proposal() {
    let (_, rubric, cases) = workspace(
        "    actionable:
      threshold: 0.9
    desk:
      bands:
        - {at_least: 0.7, verdict: route}
        - {at_least: 0.4, verdict: confirm}
      fallback: none_of_these
      note: as written
",
    );
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    let rows = table(&err, "desk (choice)");
    assert_eq!(rows.len(), 20, "the table is printed: {err}");
    assert!(
        rows.iter().all(|row| !row.contains(&"<-".to_owned())),
        "{err}"
    );
    assert!(err.contains("jud: desk: the gate has bands"), "{err}");
    assert!(!err.contains("jud: desk: propose"), "{err}");
    let proposal = yaml(&stdout(&out));
    assert_eq!(
        proposal["policy"]["desk"],
        json!({
            "bands": [
                {"at_least": 0.7, "verdict": "route"},
                {"at_least": 0.4, "verdict": "confirm"}
            ],
            "fallback": "none_of_these",
            "note": "as written"
        })
    );
    assert_eq!(proposal["policy"]["actionable"]["threshold"], json!(0.55));
    assert!(
        proposal["tuning"]["labelled"].get("desk").is_none(),
        "{proposal}"
    );
}

#[test]
fn a_score_level_is_proposed_in_the_form_the_gate_wrote_it() {
    // By text: the level comes back as its text.
    let (_, rubric, cases) = workspace(
        "    tone:
      confidence: 0.3
      level_at_least: angry
      strict: true
",
    );
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    // Read off the printed level table: the lowest level with the best F1.
    let best = best_level_printed(&err);
    let names = ["calm", "annoyed", "angry"];
    let proposal = yaml(&stdout(&out));
    assert_eq!(
        proposal["policy"]["tone"]["level_at_least"],
        json!(names[best])
    );
    assert_eq!(
        best, 1,
        "angry and annoyed separate the cases alike; the lower wins: {err}"
    );
    // fallback, strict and the rest are kept; the bar is proposed beside it.
    assert_eq!(proposal["policy"]["tone"]["strict"], json!(true));
    assert_eq!(proposal["policy"]["tone"]["confidence"], json!(0.0));
    assert_eq!(
        proposal["policy"]["tone"]["note"],
        json!(
            "lowest bar at 95% accuracy; covers 6 of 6 labelled cases; level_at_least by best F1 on 6 labelled cases; ties go to the lower level"
        )
    );
    assert!(
        err.contains("jud: tone: propose level_at_least annoyed"),
        "{err}"
    );

    // By index: the level comes back as an index.
    let (_, rubric, cases) = workspace(
        "    tone:
      confidence: 0.3
      level_at_least: 2
",
    );
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let proposal = yaml(&stdout(&out));
    assert_eq!(proposal["policy"]["tone"]["level_at_least"], json!(1));
}

/// The level the printed level table marks as proposed, checked against the
/// table's own best F1 (the lowest level on a tie).
fn best_level_printed(stderr: &str) -> usize {
    let start = stderr.find("  level  accuracy").expect("a level table");
    let rows: Vec<Vec<String>> = stderr[start..]
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .map(|l| l.split_whitespace().map(str::to_owned).collect())
        .collect();
    let f1 = |row: &Vec<String>| row[4].parse::<f64>().unwrap();
    let best = rows.iter().map(f1).fold(f64::MIN, f64::max);
    let lowest = rows
        .iter()
        .find(|row| (f1(row) - best).abs() < 1e-9)
        .unwrap();
    let marked = rows
        .iter()
        .find(|row| row.contains(&"<-".to_owned()))
        .unwrap();
    assert_eq!(lowest[0], marked[0]);
    lowest[0].parse().unwrap()
}

#[test]
fn strict_and_the_fallback_are_kept_and_the_note_is_replaced() {
    let (_, rubric, cases) = workspace(
        "    actionable:
      threshold: 0.9
      strict: true
      note: guess
    desk:
      confidence: 0.9
      fallback: none_of_these
      strict: true
      note: guess
",
    );
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let proposal = yaml(&stdout(&out));
    assert_eq!(
        proposal["policy"]["actionable"],
        json!({
            "threshold": 0.55,
            "strict": true,
            "note": "best F1 on 7 labelled cases; ties go to the lower threshold"
        })
    );
    assert_eq!(
        proposal["policy"]["desk"],
        json!({
            "confidence": 0.45,
            "fallback": "none_of_these",
            "strict": true,
            "note": "lowest bar at 95% accuracy; covers 6 of 7 labelled cases"
        })
    );
    // A strict gate acts above its bar and the sweep counts at it: said.
    assert!(
        stderr(&out).contains("the gate is strict"),
        "{}",
        stderr(&out)
    );
    // The questions without a gate here are not in the proposal.
    assert!(proposal["policy"].get("tone").is_none());
}

#[test]
fn a_question_without_a_gate_or_without_labels_is_skipped_with_a_note() {
    let dir = scratch("tune");
    let rubric = write(
        &dir,
        "rubric.jud",
        &triage_with_policy(
            "    actionable:
      threshold: 0.9
    tone:
      confidence: 0.9
",
        ),
    );
    let unlabelled = cases_without_tone_labels();
    let cases = write(&dir, "cases.jud", &unlabelled);
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("jud: desk: skipped, it has no gate; add one under `policy` to tune it"),
        "{err}"
    );
    assert!(
        err.contains("jud: tone: skipped, no case labels it"),
        "{err}"
    );
    let proposal = yaml(&stdout(&out));
    // The gate that was tuned moved; the one without labels is as written.
    assert_eq!(proposal["policy"]["actionable"]["threshold"], json!(0.55));
    assert_eq!(proposal["policy"]["tone"], json!({"confidence": 0.9}));
    assert!(proposal["policy"].get("desk").is_none());
    assert_eq!(proposal["tuning"]["labelled"], json!({"actionable": 7}));
}

#[test]
fn when_nothing_can_be_proposed_there_is_no_tuning_block_and_no_file() {
    let dir = scratch("tune");
    let rubric = write(
        &dir,
        "rubric.jud",
        &triage_with_policy("    tone:\n      confidence: 0.9\n"),
    );
    let unlabelled = cases_without_tone_labels();
    let cases = write(&dir, "cases.jud", &unlabelled);
    let out_path = dir.join("tuned.jud");
    let before = snapshot(&dir);
    let out = tune(
        path_str(&rubric),
        path_str(&cases),
        RECORDINGS,
        &["--out", path_str(&out_path)],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).is_empty(), "no tuning block: {}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("jud: nothing was proposed"), "{err}");
    assert!(err.contains("no file was written to"), "{err}");
    assert_eq!(snapshot(&dir), before, "nothing is written");
    assert!(!out_path.exists());
}

#[test]
fn a_conversation_is_tuned_turn_by_turn() {
    let out = tune(HANDOFF, HANDOFF_CASES, RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("jud: wants_human (noul): 6 labelled cases"),
        "{err}"
    );
    let proposal = yaml(&stdout(&out));
    assert_eq!(proposal["tuning"]["labelled"], json!({"wants_human": 6}));
    assert_eq!(proposal["tuning"]["model"], json!("jev-1.13.0"));
    // The bar is the one its printed table marks.
    let rows = table(&err, "wants_human (noul)");
    let marked = rows
        .iter()
        .find(|row| row.contains(&"<-".to_owned()))
        .unwrap();
    let threshold: f64 = marked[0].parse().unwrap();
    assert_eq!(
        proposal["policy"]["wants_human"]["threshold"],
        json!(threshold)
    );
}

#[test]
fn bad_numbers_are_refused_before_any_recording_is_read() {
    // The directory does not exist: were it opened first, that would be the
    // refusal, and this one would never be seen.
    for (flag, message) in [
        (
            "--target-accuracy=1.5",
            "--target-accuracy must be from 0 to 1, got 1.5",
        ),
        (
            "--target-accuracy=-0.1",
            "--target-accuracy must be from 0 to 1, got -0.1",
        ),
        (
            "--target-accuracy=NaN",
            "--target-accuracy must be from 0 to 1",
        ),
        ("--min-covered=0", "--min-covered must be at least 1"),
    ] {
        let out = tune(TRIAGE, TRIAGE_CASES, "/nonexistent/recordings", &[flag]);
        assert_eq!(code(&out), 2, "{flag}");
        let err = stderr(&out);
        assert!(err.contains(message), "{flag}: {err}");
        assert!(!err.contains("cannot replay"), "{flag}: {err}");
        assert!(stdout(&out).is_empty(), "{flag}");
    }
    // The ends of the range are values.
    for value in ["0", "1", "1.0", "0.0"] {
        let out = tune_triage(&["--target-accuracy", value]);
        assert_eq!(code(&out), 0, "{value}: {}", stderr(&out));
    }
}

#[test]
fn a_rubric_without_gates_has_nothing_to_tune() {
    let dir = scratch("tune");
    let rubric = write(&dir, "bare.jud", &triage_without_policy());
    let out = tune(path_str(&rubric), TRIAGE_CASES, RECORDINGS, &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("the rubric has no gates to tune; add one per question first"),
        "{err}"
    );
    assert!(err.contains("bare.jud"), "the file is named: {err}");
    assert!(stdout(&out).is_empty());
}

#[test]
fn a_document_that_is_not_the_right_kind_is_refused_by_name() {
    let out = tune(TRIAGE_CASES, TRIAGE_CASES, RECORDINGS, &[]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains(TRIAGE_CASES), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("is not a valid Rubric"),
        "{}",
        stderr(&out)
    );
    let out = tune(TRIAGE, "/nonexistent/cases.jud", RECORDINGS, &[]);
    assert_eq!(code(&out), 2);
    assert!(
        stderr(&out).contains("cannot read cases /nonexistent/cases.jud"),
        "{}",
        stderr(&out)
    );
    let out = tune(TRIAGE, TRIAGE_CASES, "/nonexistent/recordings", &[]);
    assert_eq!(code(&out), 2);
    assert!(
        stderr(&out).contains("cannot replay from /nonexistent/recordings"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_missing_recording_is_status_1_naming_every_case_without_one() {
    let recordings = copy_dir(RECORDINGS);
    std::fs::remove_file(recordings.join("refund-angry.jud")).unwrap();
    std::fs::remove_file(recordings.join("thanks.jud")).unwrap();
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("no recording answers 2 cases: refund-angry, thanks"),
        "{err}"
    );
    assert!(err.contains("jud record"), "{err}");
    assert!(stdout(&out).is_empty(), "{}", stdout(&out));
}

#[test]
fn recordings_of_two_models_are_refused() {
    let recordings = copy_dir(RECORDINGS);
    let receipt = recordings.join("receipt.jud");
    let text = std::fs::read_to_string(&receipt).unwrap();
    assert!(text.contains("model: jev-1.13.0"));
    std::fs::write(
        &receipt,
        text.replacen("model: jev-1.13.0", "model: jev-1.12.0", 1),
    )
    .unwrap();
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("the recordings come from more than one model (jev-1.12.0, jev-1.13.0); a bar is tuned per model, record again with one"),
        "{err}"
    );
    // The odd one out is named, to record again.
    assert!(err.contains("jev-1.12.0 answered receipt"), "{err}");
    assert!(stdout(&out).is_empty());

    // A recording of another model that no case here asks for does not count.
    let recordings = copy_dir(RECORDINGS);
    let unrelated = recordings.join("patient-turn-0.jud");
    let text = std::fs::read_to_string(&unrelated).unwrap();
    std::fs::write(
        &unrelated,
        text.replacen("model: jev-1.13.0", "model: jev-1.12.0", 1),
    )
    .unwrap();
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(yaml(&stdout(&out))["tuning"]["model"], json!("jev-1.13.0"));
}

#[test]
fn the_server_is_the_one_the_answering_recordings_name() {
    let strip = |text: &str| -> String {
        let kept: Vec<&str> = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("server:"))
            .collect();
        indented(&kept.join("\n"), "")
    };
    // None names a server: the tuning block has none.
    let recordings = copy_dir(RECORDINGS);
    for entry in std::fs::read_dir(&recordings).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, strip(&text)).unwrap();
    }
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let tuning = yaml(&stdout(&out))["tuning"].clone();
    assert!(tuning.get("server").is_none(), "{tuning}");
    assert_eq!(tuning["model"], json!("jev-1.13.0"));

    // Two answered the cases: no server is claimed.
    let recordings = copy_dir(RECORDINGS);
    let receipt = recordings.join("receipt.jud");
    let text = std::fs::read_to_string(&receipt).unwrap();
    std::fs::write(
        &receipt,
        text.replace(
            "server: https://api.typesafe.ai",
            "server: https://other.example",
        ),
    )
    .unwrap();
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(yaml(&stdout(&out))["tuning"].get("server").is_none());

    // A recording of a request no case asks does not vote.
    let recordings = copy_dir(RECORDINGS);
    let unrelated = recordings.join("patient-turn-0.jud");
    let text = std::fs::read_to_string(&unrelated).unwrap();
    std::fs::write(
        &unrelated,
        text.replace(
            "server: https://api.typesafe.ai",
            "server: https://other.example",
        ),
    )
    .unwrap();
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(
        yaml(&stdout(&out))["tuning"]["server"],
        json!("https://api.typesafe.ai")
    );
}

#[test]
fn a_recording_keyed_by_hash_alone_still_names_its_server() {
    // The `.json` recordings of a `Recorder` carry a request hash and no
    // fingerprint; the replay finds them by it, and so does the server scan.
    let rubric = Rubric::parse(&std::fs::read_to_string(TRIAGE).unwrap()).unwrap();
    let cases = Cases::parse(&std::fs::read_to_string(TRIAGE_CASES).unwrap()).unwrap();
    let dir = scratch("tune");
    for (index, case) in cases.cases.iter().enumerate() {
        let source = Path::new(RECORDINGS).join(format!("{}.jud", case.name(index)));
        let mut recording = parse_recording(&std::fs::read_to_string(source).unwrap()).unwrap();
        let questions = case.request(&rubric).unwrap();
        recording.request_hash = Some(request_hash(&case.state, &questions));
        recording.fingerprint = None;
        write_recording(&dir, &recording).unwrap();
    }
    assert!(dir.join("receipt.json").exists());
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&dir), &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let tuning = yaml(&stdout(&out))["tuning"].clone();
    assert_eq!(tuning["server"], json!("https://api.typesafe.ai"));
    assert_eq!(tuning["model"], json!("jev-1.13.0"));
}

#[test]
fn replay_is_required_and_jud_replay_stands_in_for_it() {
    let out = jud(&["tune", TRIAGE, TRIAGE_CASES], "", &[]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("--replay"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty());

    let by_flag = tune_triage(&[]);
    let by_env = jud(
        &["tune", TRIAGE, TRIAGE_CASES],
        "",
        &[("JUD_REPLAY", RECORDINGS)],
    );
    assert_eq!(code(&by_env), 0, "{}", stderr(&by_env));
    let (flag, env) = (yaml(&stdout(&by_flag)), yaml(&stdout(&by_env)));
    assert_eq!(flag["policy"], env["policy"]);
    assert_eq!(flag["tuning"]["cases"], env["tuning"]["cases"]);
}

#[test]
fn tune_never_calls_a_backend() {
    // No key, no base URL, and a base URL nothing listens on: a call would
    // fail, and a tune that made one would exit 1.
    let out = jud(
        &["tune", TRIAGE, TRIAGE_CASES, "--replay", RECORDINGS],
        "",
        &[
            ("TYPESAFE_BASE_URL", "http://127.0.0.1:9"),
            ("TYPESAFE_API_KEY", "k"),
        ],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
}
