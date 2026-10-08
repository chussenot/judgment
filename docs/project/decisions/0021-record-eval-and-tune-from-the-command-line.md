---
title: 0021 Record, evaluate and tune from the command line
description: Proposed; three jud subcommands close the loop from a rubric and its labelled cases to a tuned policy without Rust. jud record answers every case once and writes the recordings, jud eval grades answers against the labels and gates CI with a new exit status 3, and jud tune proposes bars from recordings only, printing them by default; why three commands rather than one, what each writes, and what the choice costs.
status: proposed
date: 2026-10-08
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-08
tags: [decisions, jud, cli, evaluation, tuning, recordings, ci]
---

# 0021 Record, evaluate and tune from the command line

**Status: proposed.** Nothing in the binary moves until this record is accepted.

## Context and problem statement

A decision written as a `.jud` rubric goes through one loop ([Rubrics, cases and recordings](../../concepts/rubrics-cases-recordings.md)): write the questions and a guessed policy, label cases, answer each case, grade the answers against the labels, read each bar off a table, write the bars back with their provenance, and keep the answers so the next change is checked without a call. [Decision 0019](0019-a-command-line-for-the-format.md) gave the format a command line that covers the first half. `jud check` reads and binds the documents, `jud lower` shows what a state is asked, `jud RUBRIC` evaluates one state, and `--replay` answers from recordings.

The second half has no command. Answering every case means a shell loop over the cases file with `yq`. Comparing the verdicts with the labels is done by eye. Recording the answers, grading them and sweeping a bar need Rust: the steps exist in the crate (`Recorder`, `jud::recording_to_yaml`, `jud::grade`, `eval::QuestionMetrics`, `eval::tuning`) and are wired together only in `examples/jud_calibration.rs`. So the people the command line was built for, who write rubrics and pipe JSON, stop exactly where a guessed threshold should become a measured one. A pipeline has no way to fail when a model upgrade or a rubric edit breaks the labels.

The question is what the command line should add so that the loop runs from a shell, and how to cut it.

## Decision drivers

- **The paid step is separate.** Answering the cases costs model calls. Grading and tuning cost nothing once the answers exist, and are repeated far more often (another target accuracy, another rubric edit).
- **CI never calls a model and never rewrites a file.** A regression check runs over committed recordings, with no key, and only reads.
- **A tuned bar is reviewed before it lands.** The format exists so that a number in a policy says what it rests on. A bar written into a file without a person seeing the table behind it is the guess with better formatting.
- **No new logic.** Everything the commands do is already in the library and exercised by the calibration example; the commands are glue, and their results equal the example's on the same documents.
- **The command's contract stays legible.** The exit status, the output and what writes a file are the command's contract ([Stability](../../reference/stability.md)); additions are named, and nothing existing changes meaning.

## Considered options

1. **One `jud tune RUBRIC CASES`** that answers every case, grades, sweeps and rewrites the rubric.
2. **Three subcommands, `jud record`, `jud eval` and `jud tune`**, one per step, sharing the recordings directory.
3. **`jud eval` alone**, with a `--record DIR` flag that writes what it answers, and tuning left to Rust.
4. **Nothing in the binary**: document the shell loop and the calibration example as the way.

## Proposed outcome

Option 2.

### `jud record RUBRIC CASES --out DIR`

Lowers every case's request (its state and its own `options`, so a Choice with `options_from: request` is recorded too) and asks the configured backend, as `jud RUBRIC` does. Each response is verified against its request, then written as `DIR/<case>.jud` through `jud::recording_to_yaml`, with the request fingerprint, the rubric's name, the server and the time. A conversation case whose labels use `from_turn` is recorded turn by turn, as `<case>-turn-<n>.jud`, the shape the calibration recordings already have.

A recording already in `DIR` whose fingerprint matches the request is kept and not asked again, so an interrupted run resumes and a new case costs one call; `--refresh` asks every case again. Cases are asked one at a time. It is the only subcommand that spends calls. Status 1 when a call fails (the recordings already written stay), 2 for a usage error.

### `jud eval RUBRIC CASES [--replay DIR]`

Answers every case (from `DIR` with `--replay` or `JUD_REPLAY`, from the backend otherwise), grades each answer against the case's `expect` with `jud::grade`, and reports per question:

- what the model got right: labelled, correct, accuracy with its 95 % Wilson interval, Brier score, calibration error, mean confidence when right and when wrong;
- what the current policy does with those answers: the share of cases each gate acts on rather than defers, and the accuracy among them;
- every miss, by case, with the expected and the given answer.

Text by default, for a person; `--json` writes the same report as one JSON object on stdout for `jq`. A gate is a flag: `--min-accuracy 0.9` for every question, or `--min-accuracy desk=0.95` for one, repeatable. When a gate is not met the command exits with **status 3**, a new status meaning "the evaluation ran and the labels were not met", distinct from 1 (a call failed, or no recording for a case under `--replay`) and 2 (wrong before any call). Without a gate it exits 0 whatever the accuracy.

### `jud tune RUBRIC CASES --replay DIR`

Reads recordings only; there is no live mode, so a tuning run never spends a call. For each gate it prints the sweep and a proposal:

- a Noul's `threshold`: the best F1 (`eval::tuning::best_threshold`);
- a Choice's or a Score's `confidence`: the lowest bar that keeps `--target-accuracy` (default 0.95) over at least `--min-covered` cases (default 3) (`lowest_bar`);
- a Score's `level_at_least`, when the gate has one: the best F1 over the levels (`best_level`).

A gate with `bands` gets its table printed and no proposal in this first version; `fallback` and `strict` are kept as written. Every proposal carries the number of labelled cases and the accuracy interval, and a warning when the interval is wider than 0.2, so seven cases never print a precise-looking bar.

Output: the tables on stderr, and on stdout the proposed `policy` and `tuning` blocks as YAML, ready to paste. The `tuning` block names the cases by fingerprint, the model and server the recordings came from, and the time. `--out PATH` instead writes the whole rubric with the proposal applied, through `Rubric::to_yaml`, to a path other than the input. The input is never rewritten in place, because `to_yaml` serialises the document again and drops its YAML comments.

### Consequences

- The loop runs from a shell: `jud record` once with a key, then `jud eval --replay` and `jud tune --replay` as often as needed, with no key.
- CI gets a regression check: `jud eval rubric.jud cases.jud --replay recordings/ --min-accuracy 0.9`, which fails with status 3 when an edit breaks the labels, and with status 1 when a case was added and not recorded.
- Rubrics with `options_from: request` become testable from the shell, because cases carry their options where stdin cannot.
- The command now writes files: `record` writes recordings and `tune --out` writes a rubric. [The jud command line](../../reference/cli.md) says today that nothing writes a file; that sentence becomes "only `record` and `tune --out` write, and never over their input". `check`, `lower`, `eval` and a run stay read-only.
- Exit status 3 joins the command's contract in [Stability](../../reference/stability.md). The existing statuses keep their meaning, so the change is additive and ships in a minor release.
- [Decision 0019](0019-a-command-line-for-the-format.md) stands; this record extends the command it defined. A status note in the [index](README.md) records that.
- The calibration example stays, as the Rust reference for the same loop; its output and `jud tune`'s are the same on the example documents.
- Not decided here: patching the `policy` and `tuning` keys in place while keeping comments, tuning `bands`, asking cases concurrently, and a `jud init` scaffold. Each can be a later record once the three commands are used.

### Confirmation

When applied: `tests/jud_cli.rs` runs each subcommand against wiremock and against `examples/recordings/jud_calibration/`, with no key and no network. `jud eval` over the triage documents reports the accuracies `examples/jud_calibration.rs` prints, and exits 3 with a gate above them. `jud tune` proposes the bars the example writes (`threshold: 0.55` and `confidence: 0.45` for the triage rubric). `jud record` against wiremock writes recordings that `jud check` verifies with matching fingerprints, and asks nothing on a second run. `docs/reference/cli.md` is regenerated from the new help, and the guides [Tune thresholds](../../guides/tune-thresholds.md) and [Run in CI](../../guides/run-in-ci.md) move from the example and the `yq` loop to the commands.

## Pros and cons of the options

### 1. One `jud tune`

- Good, because it is one command to learn and one line in a tutorial.
- Bad, because every attempt at another target accuracy spends the calls again, or needs flags to skip the paid half.
- Bad, because CI needs the grading without the calls and without the write, which this command would have to switch off by flags.
- Bad, because it rewrites the rubric as the end of a run nobody looked at.

### 2. Three subcommands

- Good, because the paid step, the check and the proposal are each one command with one effect, and each composes in a script.
- Good, because CI uses `eval` alone, read-only, keyless.
- Good, because the recordings directory is the hand-off between them and is already the format's third kind.
- Bad, because there are three commands to document and test instead of one.
- Bad, because a first-time user runs two commands (`record`, then `tune`) before seeing a bar.

### 3. `jud eval --record`

- Good, because one command answers, records and grades in one pass.
- Bad, because the CI command would sometimes write files, depending on a flag, which is the property CI must not have.
- Bad, because tuning, the step people ask for by name, stays in Rust.

### 4. Nothing in the binary

- Good, because there is no new contract to keep.
- Bad, because the loop the format exists for stays out of reach of the people writing rubrics, and pipelines keep no regression check.

## More information

- [Decision 0019](0019-a-command-line-for-the-format.md): the command this record extends.
- [Rubrics, cases and recordings](../../concepts/rubrics-cases-recordings.md): the loop and the metrics.
- [Tune thresholds](../../guides/tune-thresholds.md) and `examples/jud_calibration.rs`: the same loop in Rust today.
- `src/bin/jud/`: where the subcommands land; `src/backend.rs` (`Recorder`, `Replay`), `src/jud/` (`grade`, `recording_to_yaml`, `Case::per_turn`), `src/eval/` (`QuestionMetrics`, `tuning`): what they call.
