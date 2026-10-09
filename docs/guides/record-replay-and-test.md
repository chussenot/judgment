---
title: Record, replay and test
description: How to test a decision with no key and no network, with the Fake backend that refuses an answer the question could not produce, the Recorder and Replay pair keyed by the request's content, jud --replay for a rubric on the command line, and jud record to answer a rubric's labelled cases once and keep the answers.
status: current
last_reviewed: 2026-10-09
tags: [judgment, testing, fake, replay, recordings, record, how-to]
---

# Record, replay and test

**You will** test a decision against scripted answers, record a real model's answers once and replay them in every later run, in Rust and on the command line, with `jud record` for a rubric's labelled cases. **Prerequisites:** [Your first decision in Rust](../start/first-decision-rust.md) or [from the command line](../start/first-decision-cli.md). Why a recording is what it is: [Rubrics, cases and recordings](../concepts/rubrics-cases-recordings.md).

A decision made on a probability is hard to test against a live model: the call costs money, the answer moves between runs, and a test that passes against the real server says nothing about the branch the model did not take. The crate separates the three things a test needs: a backend that answers what the test scripts, a recording of what the real model once said, and a way to grade those recordings against labels. All three sit behind the same `SystemOne` trait as the client, so the code under test does not change.

## Script answers with `Fake`

`Fake` answers from a table, refuses a question it has no answer for, and remembers every call, so a test checks the decision and what was asked. It verifies its response as the client does: a scripted option the question does not offer, or an answer of the wrong primitive, fails the call instead of passing a test the real client would fail.

```rust
use judgment::{Fake, Questions, SystemOne, options};

options! {
    enum Desk {
        Billing = "billing" => "Invoices, payments, refunds",
        Technical = "technical" => "Errors, outages",
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> judgment::Result<()> {
    let mut questions = Questions::new();
    let desk = questions.choice::<Desk>("desk", "Which desk should take `message`?")?;
    let urgent = questions.noul("is_urgent", "Does `message` convey urgency?", None)?;

    let backend = Fake::new()
        .choice("desk", [("billing", 0.9), ("technical", 0.1)], 0.8)?
        .noul("is_urgent", 0.2)?;

    let state = serde_json::json!({ "message": "Invoice 42 is wrong." });
    let response = backend.answer(&state, "any-model", &questions).await?;
    assert_eq!(response.get(&desk)?.chosen, Desk::Billing);
    assert!(!response.get(&urgent)?.is_yes(0.5));
    assert_eq!(backend.calls()[0].question_ids, ["desk", "is_urgent"]);
    Ok(())
}
```

A scripted Score is only its probabilities: its legend is the levels of the question it answers, echoed as a server echoes them. A scripted answer the question could not produce is a compile error when the option is an enum variant, and a runtime refusal when it is a dynamic key.

## Record once, replay forever

A `Recorder` wraps any backend and writes a recording for every call it forwards; a `Replay` over the same directory answers the same requests later, with no key and no network.

```rust no_run
use std::path::Path;

use judgment::{Client, Questions, Recorder, Replay, SystemOne};

#[tokio::main(flavor = "current_thread")]
async fn main() -> judgment::Result<()> {
    let mut questions = Questions::new();
    let urgent = questions.noul("is_urgent", "Does `message` convey urgency?", None)?;
    let state = serde_json::json!({ "message": "The site is down." });

    // Once, with a key: the client's answer is written under recordings/.
    let recorder = Recorder::new(Client::from_env()?, "recordings");
    let live = recorder.answer(&state, "jev-latest", &questions).await?;

    // Every later run: the same request finds the same answer, no network.
    let replay = Replay::open(Path::new("recordings"))?;
    let again = replay.answer(&state, "jev-latest", &questions).await?;
    assert_eq!(live.get(&urgent)?.yes, again.get(&urgent)?.yes);
    Ok(())
}
```

A recording is found by two keys, in this order: the request's `sha256:` fingerprint, [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) canonical JSON of the state and the questions, which any tool computes the same way ([Fingerprints](../reference/jud-format.md#fingerprints)); then the crate's own 16-hex-digit `request_hash`. Both cover the questions, so a question changed without re-recording misses the recording and fails the test rather than grading old answers under new questions. The model is not in either key: two models answering the same request share a fingerprint, and a comparison keeps their recordings in two directories.

`Replay` verifies each recording against the questions as the client would, so a recording made under one set of options cannot answer a question that offers another. A request nobody recorded is `Error::NoRecording`, never a guess: the test that reaches for a missing recording has found a request the suite never saw.

The crate writes its own `.json` form; with the `jud` feature it writes and reads the `.jud` form too (`jud::recording_to_yaml`, `jud::parse_recording`), and `Replay` reads both from one directory. The pattern examples under `examples/` each keep their recordings under `examples/recordings/<name>/`, replay them by default, run live with `--live` and re-record with `--record` ([Patterns](patterns.md)).

## Replay on the command line

`jud --replay DIR rubric.jud`, or `JUD_REPLAY=DIR`, answers a rubric from the recordings under a directory and consults no key, no configuration file and no network:

```sh
cat event.json | jud --replay examples/recordings/jud_calibration examples/jud/triage.jud
```

A state nobody recorded is a backend failure (status 1, `no recording`); a directory that does not exist is a usage error (status 2). `jud eval` takes the same flag and variable, and `jud tune` requires one of them. [Run in CI](run-in-ci.md) turns this into a job.

## Record a rubric's cases from the shell

`jud record` answers every case of a Cases document once and keeps each answer as a `.jud` recording. It is the command that spends calls to keep the answers: a plain run and `jud eval` without `--replay` ask a backend too, and keep nothing. It needs a backend and a key, which a server that ignores the bearer takes as any non-blank word ([Configure a backend](configure-a-backend.md)). It always asks the configured backend. It has no `--replay` and never replays from `JUD_REPLAY`: a non-empty value is ignored, and an empty one is refused as everywhere else ([Replay](../reference/configuration.md#replay)). Every flag is in [The jud command line](../reference/cli.md#jud-record).

`jud record`, `jud eval` and `jud tune` arrived after 0.10.4; pin a release that lists them in [the changelog](../../CHANGELOG.md).

Before spending anything, `--dry-run` says what the run would do: how many requests, which the directory already answers, which backend and model would be asked, whether a key is set, and how long it would take at the pace of the recordings already there. It asks nothing, writes nothing and needs no key:

```sh
jud record examples/jud/screening.jud examples/jud/screening-cases.jud --out recordings/screening --dry-run
```

<!-- transcript: jud record examples/jud/screening.jud examples/jud/screening-cases.jud --out recordings/screening --dry-run -->
```text
4 requests: 0 already recorded in recordings/screening, 4 to ask (0 replacing a stale recording)
to ask: charged-twice, cancelled-last-week, how-does-billing-work, furious-outage
backend https://api.typesafe.ai, model jev-latest (from default)
API key: missing; the run would stop before the first call (a local server that ignores it takes any word)
time: unknown until the first request answers (no recording in the directory to read it from)
```

The repository's `refund-screening` documents have no recordings, so recording them needs a key:

```sh
export TYPESAFE_API_KEY=...
jud record examples/jud/screening.jud examples/jud/screening-cases.jud --out recordings/screening
```

The cases are asked one at a time, in order. The first line names the model and the server before anything is asked, so a run aimed at the wrong server can be stopped before it has paid for every request ([What `jud record` prints](../reference/cli.md#what-it-prints)). Each line after it goes to stderr as its case is recorded, then a tally; stdout stays empty. The times are the calls' own, in milliseconds; those shown came from a local test server, which `TYPESAFE_BASE_URL` pointed at.

```text
asking jev-latest at http://127.0.0.1:8000 for up to 4 requests (base URL from environment)
recorded charged-twice (1/4, 2 ms)
recorded cancelled-last-week (2/4, 1 ms)
recorded how-does-billing-work (3/4, 1 ms)
recorded furious-outage (4/4, 1 ms)
recorded 4, kept 0 in recordings/screening
```

Run it again and nothing is asked, though the first line still counts every request the cases plan, before the directory is read:

```text
asking jev-latest at http://127.0.0.1:8000 for up to 4 requests (base URL from environment)
kept charged-twice
kept cancelled-last-week
kept how-does-billing-work
kept furious-outage
recorded 0, kept 4 in recordings/screening
```

What the command does:

- **It writes one file per case.** `DIR/CASE.jud` is a `Recording` document: the response as received, the request's fingerprint, the rubric's name, the server and the time ([Recording](../reference/jud-format.md#recording)). `DIR` is created if it is not there. A file is written whole or not at all, and a recording is on disk the moment its answer has been checked against its questions.
- **It records what a case asks.** A case's state and its own `options` are lowered through the rubric, as `jud lower --cases` shows. A Choice with `options_from: request` is recorded too.
- **It records a conversation turn by turn** when a label uses `from_turn`. Each turn is its own request, with the label resolved at that turn, and its file is `CASE-turn-N.jud`, from `escalates-turn-0.jud` for a case named `escalates`. [Conversations](../reference/jud-format.md#conversations) has the rule.
- **It resumes.** A request already recorded in `DIR` is kept, found by its fingerprint under whatever file name, so an interrupted run continues where it stopped. A new case costs one call, and so does a recording you deleted. A case whose state changed, or a rubric whose questions changed, has another fingerprint and is asked again.
- **It asks the model the configuration names.** That is the `model` field of the configuration file, `jev-latest` when there is none. No flag and no environment variable names it, and `jud config` prints the model a run would use and where it came from ([Configure a backend](configure-a-backend.md)). To record another model, set `model` first, in the file or in another one that `XDG_CONFIG_HOME` points the run at, such as `model: tev1:0.8b` for a local model.
- **`--refresh` asks every case again.** Each answer is written over the file named after its case. Use it once the model has changed, since a request's fingerprint leaves the model out and a recording of the old model would be kept ([Tune thresholds](tune-thresholds.md#when-the-model-moves)). Recording into a new directory keeps the old answers beside the new. A file under another name is left alone, and the run warns when two files record one request, since a replay reads only one.
- **A failed call stops the run** with status 1. The message names the case it stopped at, how many were recorded and kept before it, and the directory. What was written stays, and the next run resumes.
- **It strips credentials.** The server written into a recording is the base URL without its user information, query and fragment, so a recording can be committed.

A refusal is status 2 and comes before any call, so nothing is written, not even `DIR`. The message says what to change. Two are fixed in `DIR` rather than in the documents:

- **A stale recording under another name**, a file that records this request and no longer fits its questions. Delete or move it, then record again. Under `--refresh` it is not refused: the new recording is written, the stale file stays beside it and the run warns that two files record one request, so delete it. A stale recording in the case's own file is replaced, and the run says so.
- **A file the reader refuses**, such as a `.jud` that is not a recording. Fix or move it, or pass `--refresh`, which does not read `DIR`.

[The jud command line](../reference/cli.md#refusals-before-any-call) lists every refusal, among them a case without an `id`, a recording that would land on the rubric or the cases, and a missing key.

Check what was written with the rubric and the cases. Each recording is verified against its request and its fingerprint compared:

```sh
jud check examples/jud/screening.jud examples/jud/screening-cases.jud recordings/screening/*.jud
```

The recordings then answer `jud --replay`, `jud eval` and `jud tune` ([Tune thresholds](tune-thresholds.md)).

## Write a recording by hand

Only as a test fixture, where the answer is scripted and the point is the shape. The fingerprint is the one the request has, which `jud check` prints when it binds the recording to its case, and the response is written in the wire's own shape so that verification passes for the reason a real one would. [`Recording`](../reference/jud-format.md#recording) has the fields; [`examples/jud/spec-recording.jud`](../../examples/jud/spec-recording.jud) is one.

## Grade the recordings

A directory of recordings beside a cases document is the input of the tuning loop. `jud eval RUBRIC CASES --replay DIR` grades each recording against its case's labels, and `jud tune` reads the bars off the same answers. In Rust the steps are `jud::grade` and `eval::tuning`. [Tune thresholds](tune-thresholds.md).

## What this does not replace

A mock encodes what the client author believed about the wire. Only a real server can contradict that belief, which is what the ignored live tests and the verification records are for ([How the crate is checked](../project/verification/method.md)).

## Next

- [Tune thresholds](tune-thresholds.md).
- [Run in CI](run-in-ci.md).
