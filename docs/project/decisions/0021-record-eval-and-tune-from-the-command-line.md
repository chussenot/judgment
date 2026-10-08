---
title: 0021 Record, evaluate and tune from the command line
description: Accepted and implemented; three jud subcommands close the loop from a rubric and its labelled cases to a tuned policy without Rust. jud record answers every case once and writes the recordings, jud eval grades answers against the labels and gates CI with a new exit status 3, and jud tune proposes bars from recordings only, printing them by default; why three commands rather than one, what each writes, what the implementation settled, and what the choice costs.
status: accepted
date: 2026-10-08
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-08
tags: [decisions, jud, cli, evaluation, tuning, recordings, ci]
---

# 0021 Record, evaluate and tune from the command line

**Status: accepted.** Implemented in the `jud` binary on 2026-10-08. The decision below is as it was proposed, with the sentences the implementation made false corrected. [As implemented](#as-implemented) lists what the implementation settled that the proposal left open or did differently; where the two differ, the implementation is the contract.

## Context and problem statement

A decision written as a `.jud` rubric goes through one loop ([Rubrics, cases and recordings](../../concepts/rubrics-cases-recordings.md)): write the questions and a guessed policy, label cases, answer each case, grade the answers against the labels, read each bar off a table, write the bars back with their provenance, and keep the answers so the next change is checked without a call. [Decision 0019](0019-a-command-line-for-the-format.md) gave the format a command line that covers the first half. `jud check` reads and binds the documents, `jud lower` shows what a state is asked, `jud RUBRIC` evaluates one state, and `--replay` answers from recordings.

The second half has no command. Answering every case means a shell loop over the cases file with `yq`. Comparing the verdicts with the labels is done by eye. Recording the answers, grading them and sweeping a bar need Rust: the steps exist in the crate (`Recorder`, `jud::recording_to_yaml`, `jud::grade`, `eval::QuestionMetrics`, `eval::tuning`) and are wired together only in `examples/jud_calibration.rs`. So the people the command line was built for, who write rubrics and pipe JSON, stop exactly where a guessed threshold should become a measured one. A pipeline has no way to fail when a model upgrade or a rubric edit breaks the labels.

The question is what the command line should add so that the loop runs from a shell, and how to cut it.

## Decision drivers

- **The paid step is separate.** Answering the cases costs model calls. Grading and tuning cost nothing once the answers exist, and are repeated far more often (another target accuracy, another rubric edit).
- **CI never calls a model and never rewrites a file.** A regression check runs over committed recordings, with no key, and only reads.
- **A tuned bar is reviewed before it lands.** The format exists so that a number in a policy says what it rests on. A bar written into a file without a person seeing the table behind it is the guess with better formatting.
- **No new logic.** Everything the commands do is already in the library and exercised by the calibration example; the commands are glue, and for the gates the example tunes their bars equal its on the same documents.
- **The command's contract stays legible.** The exit status, the output and what writes a file are the command's contract ([Stability](../../reference/stability.md)); additions are named, and nothing existing changes meaning.

## Considered options

1. **One `jud tune RUBRIC CASES`** that answers every case, grades, sweeps and rewrites the rubric.
2. **Three subcommands, `jud record`, `jud eval` and `jud tune`**, one per step, sharing the recordings directory.
3. **`jud eval` alone**, with a `--record DIR` flag that writes what it answers, and tuning left to Rust.
4. **Nothing in the binary**: document the shell loop and the calibration example as the way.

## Decision outcome

Option 2.

### `jud record RUBRIC CASES --out DIR`

Lowers every case's request (its state and its own `options`, so a Choice with `options_from: request` is recorded too) and asks the configured backend, as `jud RUBRIC` does. Each response is verified against its request, then written as `DIR/<case>.jud` through `jud::recording_to_yaml`, with the request fingerprint, the rubric's name, the server and the time. A conversation case whose labels use `from_turn` is recorded turn by turn, as `<case>-turn-<n>.jud`, the shape the calibration recordings already have.

A recording already in `DIR` whose fingerprint matches the request is kept and not asked again, so an interrupted run resumes and a new case costs one call; `--refresh` asks every case again. Cases are asked one at a time. It is the only subcommand whose job is to spend calls and keep the answers; `jud eval` without a replay also calls the backend, and keeps nothing. Status 1 when a call fails (the recordings already written stay), 2 for a usage error.

### `jud eval RUBRIC CASES [--replay DIR]`

Answers every case (from `DIR` with `--replay` or `JUD_REPLAY`, from the backend otherwise), grades each answer against the case's `expect` with `jud::grade`, and reports per question:

- what the model got right: labelled, correct, accuracy with its 95 % Wilson interval, Brier score, calibration error, mean confidence when right and when wrong;
- what the current policy does with those answers: the share of cases each gate acts on rather than defers, and the accuracy among them;
- every miss, by case, with the expected and the given answer.

Text by default, for a person; `--json` writes the same report as one JSON object on stdout for `jq`. A gate is a flag: `--min-accuracy 0.9` for every labelled question, or `--min-accuracy desk=0.95` for one, repeatable. When a gate is not met the command exits with **status 3**, a new status meaning "the evaluation ran and the labels were not met", distinct from 1 (a call failed, or no recording for a case under `--replay`) and 2 (wrong before any call). Without a gate it exits 0 whatever the accuracy.

### `jud tune RUBRIC CASES --replay DIR`

Reads recordings only; there is no live mode, so a tuning run never spends a call. For each gate it prints the sweep and a proposal:

- a Noul's `threshold`: the best F1 (`eval::tuning::best_threshold`);
- a Choice's or a Score's `confidence`: the lowest bar that keeps `--target-accuracy` (default 0.95) over at least `--min-covered` cases (default 3) (`lowest_bar`);
- a Score's `level_at_least`, when the gate has one: the best F1 over the levels (`best_level`).

A gate with `bands` gets its table printed and no proposal in this first version; `fallback` and `strict` are kept as written. Every question's sweep states the number of labelled cases and the accuracy interval on stderr, the `tuning` block records the number, and a warning follows when the interval is wider than 0.2, so seven cases never print a precise-looking bar.

Output: the tables on stderr, and on stdout the proposed `policy` and `tuning` blocks as YAML, ready to paste. The `tuning` block names the cases by fingerprint, the model and server the recordings came from, and the time. `--out PATH` instead writes the whole rubric with the proposal applied, through `Rubric::to_yaml`, to a path other than the input. The input is never rewritten in place, because `to_yaml` serialises the document again and drops its YAML comments.

### Consequences

- The loop runs from a shell: `jud record` once with a key, then `jud eval --replay` and `jud tune --replay` as often as needed, with no key.
- CI gets a regression check: `jud eval rubric.jud cases.jud --replay recordings/ --min-accuracy 0.9`, which fails with status 3 when the recorded answers fall below the bar, and with status 1 when a case was added or a question edited and the new request was not recorded.
- Rubrics with `options_from: request` become testable from the shell, because cases carry their options where stdin cannot.
- The command now writes files: `record` writes recordings and `tune --out` writes a rubric. [The jud command line](../../reference/cli.md) said that nothing writes a file; it now says that two commands write and nothing else does, and never over a rubric or a cases document they were given. `check`, `lower`, `eval` and a run stay read-only.
- Exit status 3 joins the command's contract in [Stability](../../reference/stability.md). The existing statuses keep their meaning, so the change is additive and ships in a minor release.
- [Decision 0019](0019-a-command-line-for-the-format.md) stands; this record extends the command it defined. A status note in the [index](README.md) records that.
- The calibration example stays, as the Rust reference for the same loop. Its bars and `jud tune`'s are the same for the two gates it tunes on the example documents; `jud tune` sweeps every gate that has labels, so it proposes a third, for `tone`, that the example does not ([As implemented](#as-implemented)).
- Not decided here: patching the `policy` and `tuning` keys in place while keeping comments, tuning `bands`, asking cases concurrently, and a `jud init` scaffold. Each can be a later record once the three commands are used.

### Confirmation

The tests are `tests/jud_record.rs`, `tests/jud_eval.rs`, `tests/jud_tune.rs` and `tests/jud_loop.rs`. Each runs the binary as a subprocess with no key that works and no network.

- `jud record` runs against wiremock: it asks every case once, writes recordings that `jud check` verifies with matching fingerprints, and asks nothing on a second run.
- `jud eval` and `jud tune` run over `examples/recordings/jud_calibration/`. `jud eval` over the triage documents reports the accuracies `examples/jud_calibration.rs` prints, and exits 3 with a bar above them. `jud tune` proposes the bars the example writes: `threshold: 0.55` and `confidence: 0.45` for the triage rubric.
- `tests/jud_loop.rs` runs the three one after the other, so what `jud record` writes is what `jud eval` and `jud tune` read, a Choice with `options_from: request` included.
- `docs/reference/cli.md` is generated from the binary's help, and the guides [Tune thresholds](../../guides/tune-thresholds.md) and [Run in CI](../../guides/run-in-ci.md) use the commands where they used the example and the `yq` loop.

## As implemented

The subcommands are `src/bin/jud/record.rs`, `eval.rs` and `tune.rs`, with `batch.rs`, `backend.rs`, `out.rs`, `fsutil.rs` and `recordings.rs` shared. This section records what the implementation settled that the proposal left open or did differently. [The jud command line](../../reference/cli.md) is the reference for the commands as they are.

### What the proposal left open

- **Two accuracies in `jud eval`.** `--min-accuracy` holds the model's accuracy per question, the figure the report prints as `accuracy`. The gate block's `accuracy_when_acted` grades the policy: among the answers the gate acted on, how often its own verdict is the label. A Noul is read at the gate's threshold and `strict` flag, a Choice by its option, a Score by the level nearest the weighted score. Moving a gate's threshold or confidence moves the second figure and never the first, so a threshold set too high does not fail the check: `threshold: 0.95` on the triage rubric leaves the status at 0 and drops `accuracy_when_acted` to 0.43. The "model misses" are the model's own readings. An edit to a question changes the request's fingerprint, so a replay finds no recording and the status is 1, not 3.
- **What a `--min-accuracy` bar holds.** A bar with no question is held by every labelled question. A bar on a question with no labelled case is not met, and so is a bare bar when no question is labelled, because a gate that passes on nothing measured is the failure it exists to catch. A bar that names a question the rubric does not ask is status 2. Without a bar flag the status is 0 whatever the accuracy.
- **`jud tune --out` replaces the blocks.** Without `--out`, `jud tune` writes nothing and prints the `policy` and `tuning` blocks on stdout at column 0, to be indented two spaces under `spec:`. With `--out`, stdout stays empty, the whole rubric is written to the path, and stderr says so. A file already at the path is replaced. The comments of the input are lost and `version` is written as text.
- **What is refused, before any call or any write.**
    - `jud record` refuses to write a recording over the rubric or the cases. The path may be spelled another way (`./`, a symlinked directory, a hard link), and `--refresh` makes no difference.
    - `jud record` refuses a stale twin: a recording of a request this run asks, in a file this run would not write, that no longer fits the questions. The file is not the run's to remove, and a new recording beside it would leave two for one request, so the run stops and names it, unless `--refresh` is given, which does not read the directory: the new recording is written, the stale file stays and the run warns that two files record one request. A stale recording in the case's own file is replaced and reported.
    - `jud record` also refuses a case without an id, two requests that share a file name, a request that asks nothing, a label that a turn of a conversation cannot carry, and a directory that holds a file the reader refuses (`--refresh` skips reading the directory).
    - `jud tune --out` refuses the rubric, the cases and anything in the recordings directory: a recording under another name, a link into the directory, a link whose write would create a file there.
    - `jud tune` refuses recordings from more than one model and a rubric with no gates.
- **`--replay`.** `jud eval` takes it, or `JUD_REPLAY`, optionally. `jud tune` requires one of them. `jud record` has neither and never reads `JUD_REPLAY`, so a variable set for a CI job does not turn a record into a replay. A CI check that must not spend a call passes `--replay`, since an `eval` without one asks the backend.

### Streams, calls and files

- **stdout and stderr.** stdout is the result and nothing else: `jud eval`'s report, `jud tune`'s blocks, nothing for `jud record`. Progress, the final tally, tables, notes, warnings and failures (`jud: MESSAGE`) go to stderr. A reader that closes a pipe early is not an error: the result was complete, and the exit status carries it. This also stops `jud RUBRIC`, `jud check`, `jud lower` and `jud config` from panicking on a closed stdout.
- **Credentials.** The server written into a recording and into a `tuning` block is `scheme://host/path`, without userinfo, query or fragment, and so is the one a failure names. `jud tune` strips a recording's server the same way before it writes it into the `tuning` block, whatever the recording holds.
- **One runtime per run.** The backend holds one async runtime for the life of the run, so the client's connection pool survives a loop of calls.
- **Planning before any call.** Every request is lowered and every conversation label checked before any call is made or any answer is taken from a recording, so a document problem is status 2 found before case 1 is paid for, never status 1 after case 4. A failed call stops `jud record` at that case and names how many were recorded and kept; what was written stays. Under a replay `jud eval` and `jud tune` try every case before they fail, and one message names every case with no recording.
- **Writes are atomic in `jud record`.** Each recording goes to a temporary file in the same directory, is flushed, and then takes its name, so a replay never reads half a file. Two cases that lower to one request share a recording: the second is kept.
- **Replay order.** `Replay::open` reads files in name order, so which of two files recording one request answers does not depend on the file system: the one that sorts last. A `.json` recording that does not parse is `Error::InvalidRecording`, naming the file, where it was `Error::Decode`. `jud record` and `jud tune` warn when two files record one request.

### What differs from the proposal

- **Every gate with labels is swept.** `jud tune` sweeps each gated question that has labelled cases, where the calibration example tunes two. On the triage documents it proposes `tone` `confidence: 0.0` (now 0.30) beside the `0.55` and `0.45` the example writes. A question with no gate or no labelled case is skipped, and stderr says so.
- **The `tuning` block records more.** `labelled` is a map from each tuned question to the number of cases its bar was read from, where the example writes one number. `server` is the one the recordings name, and is left out when none names one or they disagree. The accuracy interval is on stderr, not in the blocks.

### Known limits, shipped on purpose

- **A Score's table reads the most probable level; the policy reads the nearest.** The confidence table is the library's `gate_table`, which takes the most probable level as the answer. The policy acts on the level nearest the weighted score. They agree on an answer with one peak and part on a spread one. `jud tune` says so on stderr for every Score whose `confidence` bar it proposes.
- **A strict gate's sweep counts an answer at the bar as acting.** A strict gate acts above its bar. `jud tune` counts the answers sitting exactly at the proposed bar and says what a strict gate does with them when there are any, and otherwise says that one can sit there.
- **A lowest bar of 0.0 is proposed when every labelled answer is already right.** The gate would then defer nothing. `jud tune` proposes it and says so, as it does for `tone` on the triage documents.
- **`jud tune --out` is a plain write.** It truncates and writes the path, not a temporary file renamed over it.
- **A base URL with userinfo sends Basic auth.** With `https://user:pass@host` as the base URL the client sends those credentials as Basic auth instead of the API key. Every recording and message drops the userinfo, but the request still carries it.

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
- [Tune thresholds](../../guides/tune-thresholds.md) and `examples/jud_calibration.rs`: the same loop with the commands, and in Rust.
- `src/bin/jud/`: where the subcommands are; `src/backend.rs` (`Recorder`, `Replay`), `src/jud/` (`grade`, `recording_to_yaml`, `Case::per_turn`), `src/eval/` (`QuestionMetrics`, `tuning`): what they call.
