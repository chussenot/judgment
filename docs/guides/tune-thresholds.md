---
title: Tune thresholds
description: How to grade a model's recorded answers against a rubric's labelled cases with jud eval, hold them to an accuracy bar that fails CI, and replace the guessed bars of the policy with bars proposed by jud tune from a table, then write them back with their provenance.
status: current
last_reviewed: 2026-10-08
tags: [judgment, jud, tuning, calibration, metrics, eval, how-to]
---

# Tune thresholds

**You will** grade a model's recorded answers against labelled cases with `jud eval`, hold them to an accuracy bar, and replace the guessed bars of a rubric's policy with bars read off a table by `jud tune`, recording what they rest on. **Prerequisites:** the `jud` command ([Install](../start/install.md)), in a release that has `jud eval` and `jud tune`, which arrived after 0.10.4 ([the changelog](../../CHANGELOG.md) lists them); a rubric and its cases ([Write a rubric](write-a-rubric.md), [Label cases](label-cases.md)); a checkout of the repository, whose triage documents and recordings this page uses. Nothing below needs a key or a network. For a rubric of your own you also need recordings of a model's answers to every case, which `jud record` makes once, with a key ([Record, replay and test](record-replay-and-test.md#record-a-rubrics-cases-from-the-shell)). The loop and its metrics are explained in [Rubrics, cases and recordings](../concepts/rubrics-cases-recordings.md).

`jud eval` and `jud tune` read the answers from a directory of recordings, named with `--replay DIR` or `JUD_REPLAY`. `jud tune` requires it. Without it `jud eval` asks the configured backend, which costs calls and keeps nothing. Neither writes a file, except `jud tune --out`. Run every command from the root of the checkout. The recordings under `examples/recordings/jud_calibration` are scripted answers that carry the model name `jev-1.13.0`, not a model's own, so the numbers below show the loop and not how that model performs.

## 1. Grade the recorded answers

`jud eval` answers every case from the recordings, grades each answer against the case's `expect`, and prints one block per question:

```sh
jud eval examples/jud/triage.jud examples/jud/triage-cases.jud \
  --replay examples/recordings/jud_calibration
```

<!-- transcript: jud eval examples/jud/triage.jud examples/jud/triage-cases.jud --replay examples/recordings/jud_calibration -->
```text
rubric inbox-triage: questions sha256:fda347c2cba6deaa8ad784360c46a8289adbc0284b373c3d9162ce11f3e7ac8a, policy sha256:1585094f59711426a98044a00ded2ec747010b5369e941f33a2732a50262e99f
cases inbox-triage-cases: sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6, 7 cases
7 requests, model jev-1.13.0

actionable
  labelled 7, correct 6, accuracy 0.86 (95% interval 0.49 to 0.97)
  brier 0.090, calibration error 0.144, confidence when right 0.92, when wrong 0.52
  gate: acts on 7 of 7, defers 0, accuracy when acted 1.00

desk
  labelled 7, correct 6, accuracy 0.86 (95% interval 0.49 to 0.97)
  brier 0.155, calibration error 0.151, confidence when right 0.76, when wrong 0.40
  gate: acts on 6 of 7, defers 1, accuracy when acted 1.00

tone
  labelled 6, correct 6, accuracy 1.00 (95% interval 0.61 to 1.00)
  brier 0.059, calibration error 0.202, confidence when right 0.80, when wrong -
  gate: acts on 7 of 7, defers 0, accuracy when acted 1.00

model misses (2)
  actionable  receipt: expected no, predicted yes, confidence 0.52
  desk        receipt: expected none_of_these, predicted billing, confidence 0.40
```

The report is on stdout and nothing else is. Read a block from the top.

- **Accuracy** is the share of labelled cases the model got right, with its 95 % interval. Six of seven is 0.86, and the interval runs from 0.49 to 0.97. On seven cases that width is the honest number.
- **Brier** is 0 for a perfect answer and grows with confident wrong ones. **Calibration error** is the gap between how confident the model was and how often it was right; it needs far more than seven cases to mean anything.
- **Confidence when right and when wrong** says whether confidence separates the two. For `actionable` it does, 0.92 against 0.52. If the two are close, no bar can sort the answers.
- **The gate line** is the policy's, not the model's. It says how many answers the gate acted on rather than deferred, and how often the policy's own verdict was right among those.
- **Model misses** lists each answer the model got wrong, by case.

The model's accuracy and the policy's can differ. The `receipt` case is a miss for `actionable`: the recorded answer is 0.52, which is a yes, and the label says no. The gate's threshold is 0.55, so the policy says no and is right, and `accuracy when acted` is 1.00. The model's own reading decides a miss: a Noul at 0.5, a Choice by its pick, a Score by its most probable level.

## 2. Hold the labels with a gate

`--min-accuracy` makes the command fail when a question's accuracy falls short. A bare number holds every labelled question; `desk=0.95` holds one; the flag repeats. It holds the model's accuracy, the first number of a block, and never the policy's.

```sh
jud eval examples/jud/triage.jud examples/jud/triage-cases.jud \
  --replay examples/recordings/jud_calibration --min-accuracy 0.9
```

The report is the same, with a section at the end:

<!-- transcript: jud eval examples/jud/triage.jud examples/jud/triage-cases.jud --replay examples/recordings/jud_calibration --min-accuracy 0.9 (end of stdout) -->
```text
--min-accuracy
  NOT MET  actionable 0.86 < 0.9
  NOT MET  desk 0.86 < 0.9
  met      tone 1.00 >= 0.9
```

The report is printed in full. The message goes to stderr and the exit status is 3:

<!-- transcript: jud eval examples/jud/triage.jud examples/jud/triage-cases.jud --replay examples/recordings/jud_calibration --min-accuracy 0.9 (stderr) -->
```text
jud: 2 question(s) below --min-accuracy: actionable 0.86 < 0.9, desk 0.86 < 0.9
```

Status 3 means the evaluation ran and the labels were not met. It is the only command that exits 3. Status 1 means a call failed, or a case has no recording under `--replay`. Status 2 means the invocation was wrong before any call, and a `--min-accuracy` that names a question the rubric does not ask is one of those. Without a gate flag `jud eval` exits 0 whatever the accuracy. A bar that names a question nobody labelled is not met, and a bare bar holds only the labelled questions and fails when none is labelled. [The exit status table](../reference/cli.md#exit-status) has every status, and [Run in CI](run-in-ci.md) puts this in a job.

## 3. Propose the bars

`jud tune` sweeps each gate over the same answers and proposes a bar for it. It reads recordings only and never calls a backend.

- **A Noul's `threshold`** is the one with the best F1, the lowest on a tie.
- **A Choice's or Score's `confidence`** is the lowest bar that keeps `--target-accuracy` (0.95 by default) over at least `--min-covered` cases (3 by default). The lowest bar acts on the most cases that still meet the target.
- **A Score's `level_at_least`**, when the gate has one, is the level with the best F1.

The tables and every note go to stderr. The proposal goes to stdout. Send stderr to a file, since the tables are long:

```sh
jud tune examples/jud/triage.jud examples/jud/triage-cases.jud \
  --replay examples/recordings/jud_calibration 2> tune.log
```

<!-- transcript: jud tune examples/jud/triage.jud examples/jud/triage-cases.jud --replay examples/recordings/jud_calibration (stdout) -->
```yaml
policy:
  actionable:
    threshold: 0.55
    note: best F1 on 7 labelled cases; ties go to the lower threshold
  desk:
    confidence: 0.45
    fallback: none_of_these
    note: lowest bar at 95% accuracy; covers 6 of 7 labelled cases
  tone:
    confidence: 0.0
    note: lowest bar at 95% accuracy; covers 6 of 6 labelled cases
tuning:
  cases: sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6
  model: jev-1.13.0
  server: https://api.typesafe.ai
  tuned_at: "2026-10-08T09:40:05Z"
  labelled:
    actionable: 7
    desk: 7
    tone: 6
```

The blocks start at column 0. `tuned_at` is the time of your run. Read the decision of each gate in `tune.log`, which also holds the tables:

```sh
grep '^jud:' tune.log
```

<!-- transcript: jud tune examples/jud/triage.jud examples/jud/triage-cases.jud --replay examples/recordings/jud_calibration (stderr notes) -->
```text
jud: tuning inbox-triage on 7 answers to 7 cases (sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6) from the recordings under examples/recordings/jud_calibration
jud: actionable (noul): 7 labelled cases, accuracy 0.86 (95% interval 0.49-0.97)
jud: actionable: propose threshold 0.55 (now 0.55): best F1 on 7 labelled cases; ties go to the lower threshold
jud: warning: actionable: 7 labelled cases, accuracy 0.86 (95% interval 0.49-0.97); a bar read off so few cases is a guess with a number on it
jud: desk (choice): 7 labelled cases, accuracy 0.86 (95% interval 0.49-0.97)
jud: desk: propose confidence 0.45 (now 0.45): lowest bar at 95% accuracy; covers 6 of 7 labelled cases
jud: warning: desk: 7 labelled cases, accuracy 0.86 (95% interval 0.49-0.97); a bar read off so few cases is a guess with a number on it
jud: tone (score): 6 labelled cases, accuracy 1.00 (95% interval 0.61-1.00)
jud: tone: propose confidence 0.00 (now 0.30): lowest bar at 95% accuracy; covers 6 of 6 labelled cases
jud: tone: the proposed bar is 0, so the gate would defer nothing: the labelled answers are already at least 95% right with no bar
jud: tone: the table reads the most probable level as the answer; the policy reads the level nearest the weighted score, which can differ
jud: warning: tone: 6 labelled cases, accuracy 1.00 (95% interval 0.61-1.00); a bar read off so few cases is a guess with a number on it
```

Read these lines before you keep anything.

- **`(now X)`** is the gate's current value. `actionable` stays at 0.55 and `desk` at 0.45: the cases say what the committed rubric already says. `tone` would move from 0.30 to 0.00.
- **A proposed bar of 0** defers nothing. All six labelled `tone` answers are right, so the table has no reason to defer one, and the note says so. That is a result to weigh, not to paste.
- **The warning** appears when the accuracy interval is wider than 0.2. Seven cases trigger it whatever the accuracy. A bar read off so few cases is a guess with a number on it; add cases before you trust it.
- **A Score's note**, printed when a confidence bar is proposed for it, says the table reads the most probable level as the answer, where the policy reads the level nearest the weighted score. They agree on a one-peaked answer and part on a spread one.
- **A strict gate** acts above its bar, and the table counts an answer at the bar as acting. The note counts the answers sitting exactly at the proposed bar when there are any, since the gate reads those the other way, and says it can happen when there are none.
- **A gate with `bands`** gets its table and no proposal, and stays as written. It is also absent from the `labelled` count, which lists only the questions that were tuned and the cases each was read from.

`jud tune` refuses with status 2 when the rubric has no gate, when `--target-accuracy` is outside 0 to 1, or when `--min-covered` is 0. It refuses recordings of more than one model, since a bar is tuned per model, and names which cases each model answered. A question with no gate, or that no case labels, is skipped with a note. When no gate gets a proposal, stdout is empty and a note says so. `jud tune` warns when two files record one request, because a replay answers from only one of them.

## 4. Write the proposal back

Keep the proposal in one of two ways.

**Paste it.** The two blocks are the rubric's `policy` and `tuning`. Indent them two spaces, and replace the rubric's own `policy` and `tuning` under `spec:` with them. Every other comment in the file stays. The `sed` does the indenting:

```sh
jud tune examples/jud/triage.jud examples/jud/triage-cases.jud \
  --replay examples/recordings/jud_calibration 2> tune.log | sed 's/^/  /' > proposal.yaml
```

**Write to another file.** `--out PATH` writes the whole rubric with the proposal applied and prints nothing on stdout. A file already at `PATH` is replaced:

```sh
jud tune examples/jud/triage.jud examples/jud/triage-cases.jud \
  --replay examples/recordings/jud_calibration --out triage-tuned.jud
```

The rubric is serialised again, so its comments and layout are lost and `version` is written as text. `--out` never names the rubric, the cases or a file in the recordings directory: that is a status 2 refusal. Without `--out`, `jud tune` writes nothing.

Check the file with its cases:

```sh
jud check triage-tuned.jud examples/jud/triage-cases.jud
```

<!-- transcript: jud tune --out triage-tuned.jud, then jud check triage-tuned.jud examples/jud/triage-cases.jud -->
```text
rubric    triage-tuned.jud: name inbox-triage, jud/v1.3, 3 questions, 3 gates (tuned)
          questions sha256:fda347c2cba6deaa8ad784360c46a8289adbc0284b373c3d9162ce11f3e7ac8a
          policy    sha256:d9e705af6ee1d94e4219fd4a8c26710d18e01d160262f3e83d8ab8c0032eefc7
          tuned on  sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6
cases     examples/jud/triage-cases.jud: name inbox-triage-cases, jud/v1.3, 7 cases, bound to triage-tuned.jud
          cases     sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6
2 documents, 0 refused
```

The questions' fingerprint is unchanged. The policy fingerprint moved, which is the point. `tuned on` is the fingerprint of the cases the bars rest on, and `tuning` also names the model, the server and the time. The next person to open the file sees what the numbers rest on.

## When the model moves

A bar is tuned per model version, and a confidence is the model's own formula: a threshold tuned on `jev-1.13.0` says nothing about `tev1:0.8b`. Record the cases again against the new model, which `jud record` reads from the `model` field of the configuration file: no flag and no environment variable names it ([Configure a backend](configure-a-backend.md), [the file](../reference/configuration.md#the-file)). Read how it grades with `jud eval`, and run `jud tune` for the new bars. Pass `--refresh` to `jud record`, or record into a new directory: a request's fingerprint leaves the model out, so otherwise the old model's answers are kept. `tuning.model` names which model answered.

When the cases change, the cost depends on what changed. A new case, or one whose state changed, has no recording, so `jud eval` and `jud tune` exit 1 naming it; `jud record` asks only that one. A changed label needs no new recording. A changed question has a new request fingerprint, so every case needs a recording again.

## The same loop in Rust

`examples/jud_calibration.rs` runs the loop in code, with `judgment::eval::tuning`, `judgment::jud::grade` and `eval::QuestionMetrics`:

```sh
cargo run --features jud --example jud_calibration
```

It proposes the same bars for `actionable` and `desk`. It tunes two of the three gates, where `jud tune` sweeps every gate of the rubric. `cargo test --all-features` runs its test over the recordings.

## Next

- [Run in CI](run-in-ci.md): hold the labels on every change with `jud eval --min-accuracy`.
- [Record, replay and test](record-replay-and-test.md): record the answers again when the model or the cases move.
- [Label cases](label-cases.md): more cases make a bar worth trusting.
- [The jud command line](../reference/cli.md#jud-eval): every flag of `jud eval` and `jud tune`, and the JSON report.
