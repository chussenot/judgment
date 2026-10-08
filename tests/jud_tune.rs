//! `jud tune` (decision 0021), run as a subprocess: no key and no network.
//! The recordings are the committed ones under
//! `examples/recordings/jud_calibration/`, replayed; a test that needs other
//! documents (a gate with bands, a label flipped, a second model) writes them
//! to a scratch directory, never over the examples.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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

/// The last component of a directory, as text.
fn dir_name(dir: &Path) -> String {
    dir.file_name().unwrap().to_string_lossy().into_owned()
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
        err.contains(
            "jud: actionable: propose threshold 0.55 (now 0.55): best F1 on 7 labelled cases"
        ),
        "{err}"
    );
    assert!(err.contains("jud: desk: propose confidence 0.45 (now 0.45): lowest bar at 95% accuracy; covers 6 of 7 labelled cases"), "{err}");
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
        // A hard link is another name for the same bytes, which resolving a
        // path does not see: writing through it rewrites the input.
        let hard = dir.join("hard.jud");
        std::fs::hard_link(&rubric, &hard).unwrap();
        spellings.push((path_str(&hard).to_owned(), "rubric"));
        let hard_cases = dir.join("hard-cases.jud");
        std::fs::hard_link(&cases, &hard_cases).unwrap();
        spellings.push((path_str(&hard_cases).to_owned(), "cases"));
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
    // Nothing was written through any name: the inputs are the examples still.
    assert_eq!(
        std::fs::read(&rubric).unwrap(),
        std::fs::read(TRIAGE).unwrap()
    );
    assert_eq!(
        std::fs::read(&cases).unwrap(),
        std::fs::read(TRIAGE_CASES).unwrap()
    );
}

/// `--out` is not in the recordings directory either: `--replay` reads every
/// `.jud` and `.json` there as a recording, so a rubric put beside them
/// makes the whole directory unreadable, and one put over a recording
/// destroys an answer that was paid for.
#[test]
fn out_is_never_a_recording_nor_a_file_beside_them() {
    let recordings = copy_dir(RECORDINGS);
    let dir = path_str(&recordings).to_owned();
    let name = dir_name(&recordings);
    let before = snapshot(&recordings);
    let receipt = std::fs::read(recordings.join("receipt.jud")).unwrap();
    for spelling in [
        // A new file in the directory, and a recording by name.
        format!("{dir}/tuned.jud"),
        format!("{dir}/receipt.jud"),
        // Both extensions the replay reads.
        format!("{dir}/tuned.json"),
        // The same directory spelled another way.
        format!("{dir}/./tuned.jud"),
        format!("{dir}/../{name}/tuned.jud"),
    ] {
        let out = tune(TRIAGE, TRIAGE_CASES, &dir, &["--out", &spelling]);
        assert_eq!(code(&out), 2, "{spelling}: {}", stderr(&out));
        let err = stderr(&out);
        assert!(
            err.contains("refusing to write into the recordings directory"),
            "{spelling}: {err}"
        );
        assert!(stdout(&out).is_empty(), "{spelling}: {}", stdout(&out));
        assert_eq!(snapshot(&recordings), before, "{spelling} was written");
    }
    assert_eq!(
        std::fs::read(recordings.join("receipt.jud")).unwrap(),
        receipt,
        "a recording is byte for byte what it was"
    );
    // The directory still replays.
    let eval = jud(&["eval", TRIAGE, TRIAGE_CASES, "--replay", &dir], "", &[]);
    assert_eq!(code(&eval), 0, "{}", stderr(&eval));
    // And a file elsewhere is fine: only the directory is off limits.
    let elsewhere = scratch("tune").join("tuned.jud");
    let out = tune(TRIAGE, TRIAGE_CASES, &dir, &["--out", path_str(&elsewhere)]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(elsewhere.exists());
}

/// The same refusal through a link: a symlink or a hard link outside the
/// directory is still the recording it points at, and a symlinked directory
/// is still the directory.
#[cfg(unix)]
#[test]
fn out_through_a_link_into_the_recordings_is_refused() {
    let recordings = copy_dir(RECORDINGS);
    let dir = path_str(&recordings).to_owned();
    let elsewhere = scratch("tune");
    let victim = recordings.join("receipt.jud");
    let soft = elsewhere.join("soft.jud");
    std::os::unix::fs::symlink(&victim, &soft).unwrap();
    let hard = elsewhere.join("hard.jud");
    std::fs::hard_link(&victim, &hard).unwrap();
    let dir_link = elsewhere.join("dir-link");
    std::os::unix::fs::symlink(&recordings, &dir_link).unwrap();
    // A link to a file that is not there yet: writing through it creates the
    // file in the directory, absolute or relative to the link.
    let dangling = elsewhere.join("dangling.jud");
    std::os::unix::fs::symlink(recordings.join("new.jud"), &dangling).unwrap();
    let relative = elsewhere.join("relative.jud");
    std::os::unix::fs::symlink(format!("../{}/new.jud", dir_name(&recordings)), &relative).unwrap();
    let before = snapshot(&recordings);
    let bytes = std::fs::read(&victim).unwrap();
    for out_path in [soft, hard, dir_link.join("new.jud"), dangling, relative] {
        let out = tune(TRIAGE, TRIAGE_CASES, &dir, &["--out", path_str(&out_path)]);
        assert_eq!(code(&out), 2, "{}: {}", out_path.display(), stderr(&out));
        assert!(
            stderr(&out).contains("refusing to write into the recordings directory"),
            "{}: {}",
            out_path.display(),
            stderr(&out)
        );
        assert!(stdout(&out).is_empty());
        assert_eq!(snapshot(&recordings), before, "{}", out_path.display());
        assert_eq!(std::fs::read(&victim).unwrap(), bytes);
    }
    let eval = jud(&["eval", TRIAGE, TRIAGE_CASES, "--replay", &dir], "", &[]);
    assert_eq!(code(&eval), 0, "{}", stderr(&eval));
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
    // A bar of 0 is no bar, and the proposal says so beside the number.
    let err = stderr(&out);
    assert!(
        err.contains("jud: desk: propose confidence 0.00 (now 0.90): lowest bar at 80% accuracy"),
        "{err}"
    );
    assert!(
        err.contains("jud: desk: the proposed bar is 0, so the gate would defer nothing"),
        "{err}"
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
    // The odd one out is named, to record again, and a long list is cut.
    assert!(err.contains("jev-1.12.0 answered receipt"), "{err}");
    assert!(
        err.contains("jev-1.13.0 answered refund-angry, thanks, login-loop, close-account, how-to-export and 1 more"),
        "{err}"
    );
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
    assert!(
        stdout(&out).starts_with("policy:\n"),
        "a proposal, not nothing: {}",
        stdout(&out)
    );
}

// ---- what the gate has now ---------------------------------------------------

#[test]
fn every_proposal_says_what_the_gate_has_now() {
    // The committed gates: the sweep agrees with two, and says the third
    // (tone, a bar of 0.3 that defers nothing here) is not what it proposes.
    let out = tune_triage(&[]);
    let err = stderr(&out);
    assert!(
        err.contains("jud: actionable: propose threshold 0.55 (now 0.55): best F1"),
        "{err}"
    );
    assert!(
        err.contains("jud: desk: propose confidence 0.45 (now 0.45): lowest bar"),
        "{err}"
    );
    assert!(
        err.contains("jud: tone: propose confidence 0.00 (now 0.30): lowest bar"),
        "{err}"
    );
    // The explanation of a bar of 0 stays beside it.
    assert!(
        err.contains("jud: tone: the proposed bar is 0, so the gate would defer nothing"),
        "{err}"
    );

    // Guesses the sweep moves: each says where it came from.
    let (_, rubric, cases) = workspace(GUESSES);
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    let err = stderr(&out);
    assert!(
        err.contains("jud: actionable: propose threshold 0.55 (now 0.90): best F1"),
        "{err}"
    );
    assert!(
        err.contains("jud: desk: propose confidence 0.45 (now 0.90): lowest bar"),
        "{err}"
    );

    // A gate without the field says so, rather than a default it does not have.
    let (_, rubric, cases) = workspace(
        "    actionable:
      note: no bar yet
    desk:
      fallback: none_of_these
    tone:
      level_at_least: 2
",
    );
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("jud: actionable: propose threshold 0.55 (now unset): best F1"),
        "{err}"
    );
    assert!(
        err.contains("jud: desk: propose confidence 0.45 (now unset): lowest bar"),
        "{err}"
    );
    assert!(
        err.contains("jud: tone: propose confidence 0.00 (now unset): lowest bar"),
        "{err}"
    );
    // A level is shown as the gate wrote it: an index, then a text.
    assert!(
        err.contains("jud: tone: propose level_at_least annoyed (now 2): level 1;"),
        "{err}"
    );
    let (_, rubric, cases) = workspace("    tone:\n      level_at_least: angry\n");
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert!(
        stderr(&out).contains("jud: tone: propose level_at_least annoyed (now angry): level 1;"),
        "{}",
        stderr(&out)
    );

    // A bar written with more digits than the grid is shown whole.
    let (_, rubric, cases) = workspace("    desk:\n      confidence: 0.333\n");
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert!(
        stderr(&out).contains("jud: desk: propose confidence 0.45 (now 0.333): lowest bar"),
        "{}",
        stderr(&out)
    );
    // And nothing the stdout carries changed for it.
    assert!(!stdout(&out).contains("now"), "{}", stdout(&out));
}

// ---- strict gates -------------------------------------------------------------

/// Every gate strict, the bars guesses the sweep moves.
const STRICT: &str = "    actionable:
      threshold: 0.9
      strict: true
    desk:
      confidence: 0.9
      fallback: none_of_these
      strict: true
    tone:
      confidence: 0.3
      strict: true
";

/// What a recording says its `question` answer was, over every recording
/// that answers it: the `field` of that answer (`noul` for a Noul, else
/// `confidence`).
fn recorded_values(dir: &str, question: &str, field: &str) -> Vec<f64> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            let doc = yaml(&text);
            doc["spec"]["response"]["answers"][question][field].as_f64()
        })
        .collect()
}

#[test]
fn a_strict_gate_says_so_and_counts_what_sits_exactly_at_its_bar() {
    // The committed recordings: the sweep proposes 0.55, 0.45 and 0, and no
    // recorded answer is exactly at any of them. Said, not assumed.
    let at = |values: &[f64], bar: f64| values.iter().filter(|v| (**v - bar).abs() < 1e-9).count();
    assert_eq!(
        at(&recorded_values(RECORDINGS, "actionable", "noul"), 0.55),
        0
    );
    assert_eq!(
        at(&recorded_values(RECORDINGS, "desk", "confidence"), 0.45),
        0
    );
    assert_eq!(
        at(&recorded_values(RECORDINGS, "tone", "confidence"), 0.0),
        0
    );
    let (_, rubric, cases) = workspace(STRICT);
    let out = tune(path_str(&rubric), path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("propose threshold 0.55 (now 0.90)"), "{err}");
    assert!(err.contains("propose confidence 0.45 (now 0.90)"), "{err}");
    for question in ["actionable", "desk", "tone"] {
        assert!(
            err.contains(&format!(
                "jud: {question}: the gate is strict (acts above the bar)"
            )),
            "{question}: {err}"
        );
    }
    assert!(!err.contains("sit exactly"), "{err}");
    assert!(!err.contains("sits exactly"), "{err}");

    // The proposal itself is the sweep's as ever: the gate stays strict.
    let proposal = yaml(&stdout(&out));
    assert_eq!(proposal["policy"]["desk"]["confidence"], json!(0.45));
    assert_eq!(proposal["policy"]["desk"]["strict"], json!(true));
}

#[test]
fn a_strict_gate_names_the_answers_at_the_bar_it_would_defer() {
    // A tie made on purpose in a scratch copy: two right desk answers at
    // 0.45, the bar the sweep proposes, and one yes at exactly 0.55.
    let recordings = copy_dir(RECORDINGS);
    let edit = |name: &str, from: &str, to: &str| {
        let path = recordings.join(name);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{name} has no `{from}`");
        std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
    };
    edit("how-to-export.jud", "confidence: 0.47", "confidence: 0.45");
    edit("invoice-vat.jud", "confidence: 0.81", "confidence: 0.45");
    edit("close-account.jud", "noul: 0.9\n", "noul: 0.55\n");
    let (_, rubric, cases) = workspace(STRICT);
    let out = tune(
        path_str(&rubric),
        path_str(&cases),
        path_str(&recordings),
        &[],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("jud: desk: 2 answers sit exactly at the proposed bar 0.45 and a strict gate defers them"),
        "{err}"
    );
    // A yes at the threshold is not deferred, it is a no.
    assert!(
        err.contains("jud: actionable: 1 answer sits exactly at the proposed bar 0.55 and a strict gate reads it as no"),
        "{err}"
    );
    // The generic note is replaced where the count is said, kept where there
    // is nothing at the bar (tone proposes 0).
    assert_eq!(err.matches("the gate is strict").count(), 1, "{err}");
    assert!(err.contains("jud: tone: the gate is strict"), "{err}");
    let proposal = yaml(&stdout(&out));
    assert_eq!(proposal["policy"]["desk"]["confidence"], json!(0.45));
    assert_eq!(proposal["policy"]["actionable"]["threshold"], json!(0.55));
}

#[test]
fn a_strict_gate_without_a_proposal_says_nothing_about_strictness() {
    // `--min-covered 7`: desk and tone have no bar to propose, so the gates
    // stay as written and there is nothing at a bar to warn about.
    let (_, rubric, cases) = workspace(STRICT);
    let out = tune(
        path_str(&rubric),
        path_str(&cases),
        RECORDINGS,
        &["--min-covered", "7"],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("jud: desk: no bar reaches"), "{err}");
    assert_eq!(err.matches("the gate is strict").count(), 1, "{err}");
    assert!(err.contains("jud: actionable: the gate is strict"), "{err}");
}

// ---- a Score's two readings -----------------------------------------------------

#[test]
fn a_score_bar_says_the_table_reads_the_most_probable_level() {
    let out = tune_triage(&[]);
    let err = stderr(&out);
    let note = "jud: tone: the table reads the most probable level as the answer; the policy reads the level nearest the weighted score, which can differ";
    assert_eq!(err.matches(note).count(), 1, "{err}");
    // It follows the proposal it qualifies, and only a Score's.
    assert!(
        err.find(note).unwrap() > err.find("jud: tone: propose confidence").unwrap(),
        "{err}"
    );
    assert!(!err.contains("jud: desk: the table reads"), "{err}");
    assert!(!err.contains("jud: actionable: the table reads"), "{err}");

    // No bar proposed, nothing to qualify.
    let out = tune_triage(&["--min-covered", "7"]);
    assert!(
        !stderr(&out).contains("the table reads the most probable level"),
        "{}",
        stderr(&out)
    );
}

// ---- two files, one request ----------------------------------------------------

/// The proposal as data without the moment it was made, which differs.
fn proposal_without_time(out: &Output) -> Value {
    let mut proposal = yaml(&stdout(out));
    proposal["tuning"]
        .as_object_mut()
        .unwrap()
        .remove("tuned_at");
    proposal
}

#[test]
fn two_recordings_of_one_request_are_warned_about_and_change_nothing() {
    let clean = tune_triage(&[]);
    assert!(
        !stderr(&clean).contains("record the same request"),
        "{}",
        stderr(&clean)
    );

    let recordings = copy_dir(RECORDINGS);
    std::fs::copy(
        recordings.join("receipt.jud"),
        recordings.join("receipt-again.jud"),
    )
    .unwrap();
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert_eq!(err.matches("record the same request").count(), 1, "{err}");
    let line = err
        .lines()
        .find(|l| l.contains("record the same request"))
        .unwrap();
    assert!(line.starts_with("jud: warning: 2 files under "), "{line}");
    assert!(
        line.contains("receipt.jud") && line.contains("receipt-again.jud"),
        "both files are named: {line}"
    );
    // The warning is a warning: the proposal is the one without the twin.
    assert_eq!(proposal_without_time(&out), proposal_without_time(&clean));
}

// ---- a reader that has gone ----------------------------------------------------------

/// `jud tune` on the triage documents whose stdout or stderr reader goes
/// away at once, as in `jud tune ... 2>&1 | head -1`.
///
/// Best effort: a child still starting has not written yet, and a child that
/// wrote first ends with the same status, so this cannot fail for being
/// early; it fails when a closed reader changes what the command does.
fn tune_with_closed(stdout_closed: bool, stderr_closed: bool, extra: &[&str]) -> Output {
    let mut args = vec!["tune", TRIAGE, TRIAGE_CASES, "--replay", RECORDINGS];
    args.extend_from_slice(extra);
    let mut child = Command::new(env!("CARGO_BIN_EXE_jud"))
        .current_dir(support::ROOT)
        .args(&args)
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("TYPESAFE_BASE_URL")
        .env_remove("JUD_REPLAY")
        .env("XDG_CONFIG_HOME", support::config_home())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if stderr_closed {
        drop(child.stderr.take());
    }
    if stdout_closed {
        drop(child.stdout.take());
    }
    child.wait_with_output().unwrap()
}

#[test]
fn a_stderr_nobody_reads_does_not_stop_the_proposal() {
    // Every table and note is a write to a closed pipe; `eprintln!` panics on
    // that (status 101). The proposal is what the run is for.
    let out = tune_with_closed(false, true, &[]);
    assert_eq!(code(&out), 0, "a closed stderr is not a failure");
    let proposal = yaml(&stdout(&out));
    assert_eq!(proposal["policy"]["actionable"]["threshold"], json!(0.55));
    assert_eq!(proposal["policy"]["desk"]["confidence"], json!(0.45));

    // With --out the file is written the same.
    let tuned = scratch("tune").join("tuned.jud");
    let out = tune_with_closed(false, true, &["--out", path_str(&tuned)]);
    assert_eq!(code(&out), 0);
    assert!(stdout(&out).is_empty());
    let check = jud(&["check", path_str(&tuned), TRIAGE_CASES], "", &[]);
    assert_eq!(code(&check), 0, "{}{}", stdout(&check), stderr(&check));
}

#[test]
fn a_stdout_nobody_reads_does_not_change_the_exit_status() {
    // `jud tune ... | head -c1`: the proposal was complete, the pipe ended.
    let out = tune_with_closed(true, false, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("jud: actionable: propose threshold 0.55"),
        "{err}"
    );
    assert!(!err.contains("panicked"), "{err}");
    // Neither reader at all.
    let out = tune_with_closed(true, true, &[]);
    assert_eq!(code(&out), 0);
}

// ---- the help ------------------------------------------------------------------------

#[test]
fn the_help_says_where_the_blocks_go_and_what_out_loses() {
    let out = jud(&["tune", "--help"], "", &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    // The help wraps at the terminal's width: compare the words.
    let help = stdout(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for wanted in [
        "column 0",
        "paste them under `spec:`, indented two spaces",
        "its comments and layout are lost",
        "`version` is written as text",
        "Never the rubric, the cases or a file in DIR",
    ] {
        assert!(help.contains(wanted), "`{wanted}` is not in: {help}");
    }
}

// ---- digits ------------------------------------------------------------------------------

/// Nineteen cases: the six desk answers the model got right, three times
/// each under other ids, and the one it got wrong (the receipt, at 0.40).
/// 18 of 19 is 0.947, which two decimals print as the 0.95 the target asks.
fn cases_one_miss_in_nineteen() -> String {
    let mut cases = yaml(&std::fs::read_to_string(TRIAGE_CASES).unwrap());
    let originals = cases["spec"]["cases"].as_array().unwrap().clone();
    let mut chosen = Vec::new();
    for case in originals {
        let id = case["id"].as_str().unwrap().to_owned();
        let times = if id == "receipt" { 1 } else { 3 };
        for k in 0..times {
            let mut case = case.clone();
            if id != "receipt" {
                case["id"] = json!(format!("{id}-{k}"));
            }
            chosen.push(case);
        }
    }
    cases["spec"]["cases"] = Value::Array(chosen);
    serde_saphyr::to_string(&cases).unwrap()
}

#[test]
fn a_row_below_the_target_never_prints_as_the_target() {
    let dir = scratch("tune");
    let cases = write(&dir, "cases.jud", &cases_one_miss_in_nineteen());
    let out = tune(TRIAGE, path_str(&cases), RECORDINGS, &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("jud: desk (choice): 19 labelled cases"),
        "{err}"
    );
    let rows = table(&err, "desk (choice)");
    // 18 of 19 right with the miss in: under 95%, and printed as such.
    assert_eq!(rows[0][1..], ["19", "1.00", "18", "0.947"], "{err}");
    let proposed = rows
        .iter()
        .find(|row| row.contains(&"<-".to_owned()))
        .unwrap();
    // The first bar the miss is under, with every row of the column in the
    // same digits.
    assert_eq!(proposed[0], "0.45", "{err}");
    assert_eq!(proposed[4], "1.000", "{err}");
    assert!(
        err.contains("jud: desk: propose confidence 0.45 (now 0.45): lowest bar at 95% accuracy; covers 18 of 19 labelled cases"),
        "{err}"
    );

    // Where two decimals say it right, they are kept: the committed table.
    let out = tune_triage(&[]);
    let rows = table(&stderr(&out), "desk (choice)");
    assert_eq!(rows[0][4], "0.86", "{}", stderr(&out));
}

// ---- what a recording says is not trusted ---------------------------------------------------

#[test]
fn a_credential_in_a_recordings_server_is_not_copied_into_the_tuning_block() {
    // Recordings an older run wrote with the base URL as it was configured.
    let recordings = copy_dir(RECORDINGS);
    for entry in std::fs::read_dir(&recordings).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            text.replace(
                "server: https://api.typesafe.ai",
                "server: https://user:s3cret@api.typesafe.ai/?key=abc#frag",
            ),
        )
        .unwrap();
    }
    // One of them with another secret: the same server, still.
    let receipt = recordings.join("receipt.jud");
    let text = std::fs::read_to_string(&receipt).unwrap();
    std::fs::write(&receipt, text.replace("user:s3cret@", "other:hunter2@")).unwrap();

    let tuned = scratch("tune").join("tuned.jud");
    for extra in [&[][..], &["--out", path_str(&tuned)][..]] {
        let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), extra);
        assert_eq!(code(&out), 0, "{}", stderr(&out));
        for secret in ["s3cret", "hunter2", "key=abc", "frag", "user:"] {
            assert!(!stdout(&out).contains(secret), "{secret}: {}", stdout(&out));
            assert!(!stderr(&out).contains(secret), "{secret}: {}", stderr(&out));
        }
    }
    let written = std::fs::read_to_string(&tuned).unwrap();
    assert!(
        !written.contains("s3cret") && !written.contains("hunter2"),
        "{written}"
    );
    // The host and the path stay: they say which server answered.
    assert_eq!(
        yaml(&written)["spec"]["tuning"]["server"],
        json!("https://api.typesafe.ai/")
    );
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(
        yaml(&stdout(&out))["tuning"]["server"],
        json!("https://api.typesafe.ai/")
    );
}

#[cfg(unix)]
#[test]
fn a_link_in_the_recordings_directory_does_not_vote_for_a_server() {
    // The replay reads regular files only, so a link answers nothing and
    // names no server.
    let recordings = copy_dir(RECORDINGS);
    let other = scratch("tune").join("elsewhere.jud");
    let text = std::fs::read_to_string(recordings.join("receipt.jud")).unwrap();
    std::fs::write(
        &other,
        text.replace(
            "server: https://api.typesafe.ai",
            "server: https://other.example",
        ),
    )
    .unwrap();
    std::os::unix::fs::symlink(&other, recordings.join("zz-link.jud")).unwrap();
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(
        yaml(&stdout(&out))["tuning"]["server"],
        json!("https://api.typesafe.ai")
    );
}

#[test]
fn a_models_control_characters_are_shown_as_text_never_obeyed() {
    // A model a recording (or a server) named with an escape sequence and a
    // line break: shown as characters, so it cannot recolour the terminal,
    // retitle it or start a line of its own that a CI log reads as a command.
    let hostile = "jev\\x1b[31mRED\\x1b]0;pwned\\x07\\n::error::spoofed";
    let recordings = copy_dir(RECORDINGS);
    let receipt = recordings.join("receipt.jud");
    let text = std::fs::read_to_string(&receipt).unwrap();
    std::fs::write(
        &receipt,
        text.replacen("model: jev-1.13.0", &format!("model: \"{hostile}\""), 1),
    )
    .unwrap();
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("more than one model"), "{err}");
    assert!(
        err.contains("jev\\u{1b}[31mRED\\u{1b}]0;pwned\\u{7}\\n::error::spoofed"),
        "{err}"
    );
    assert!(
        !err.contains('\u{1b}') && !err.contains('\u{7}'),
        "a control character reached the terminal: {err:?}"
    );
    assert!(
        !err.lines().any(|l| l.starts_with("::error::")),
        "a line of its own: {err}"
    );

    // Every recording by that model: tuned, and the YAML escapes it itself.
    let recordings = copy_dir(RECORDINGS);
    for entry in std::fs::read_dir(&recordings).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            text.replacen("model: jev-1.13.0", &format!("model: \"{hostile}\""), 1),
        )
        .unwrap();
    }
    let out = tune(TRIAGE, TRIAGE_CASES, path_str(&recordings), &[]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let (text, err) = (stdout(&out), stderr(&out));
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{7}'),
        "{text:?}"
    );
    assert!(!err.contains('\u{1b}') && !err.contains('\u{7}'), "{err:?}");
    assert_eq!(
        yaml(&text)["tuning"]["model"],
        json!("jev\u{1b}[31mRED\u{1b}]0;pwned\u{7}\n::error::spoofed")
    );
}
