---
title: Record, replay and test
description: How to test a decision with no key and no network, with the Fake backend that refuses an answer the question could not produce, the Recorder and Replay pair keyed by the request's content, and jud --replay for a rubric on the command line.
status: current
last_reviewed: 2026-10-07
tags: [judgment, testing, fake, replay, recordings, how-to]
---

# Record, replay and test

**You will** test a decision against scripted answers, record a real model's answers once and replay them in every later run, in Rust and on the command line. **Prerequisites:** [Your first decision in Rust](../start/first-decision-rust.md) or [from the command line](../start/first-decision-cli.md). Why a recording is what it is: [Rubrics, cases and recordings](../concepts/rubrics-cases-recordings.md).

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

A state nobody recorded is a backend failure (status 1, `no recording`); a directory that does not exist is a usage error (status 2). [Run in CI](run-in-ci.md) turns this into a job.

## Write a recording by hand

Only as a test fixture, where the answer is scripted and the point is the shape. The fingerprint is the one the request has, which `jud check` prints when it binds the recording to its case, and the response is written in the wire's own shape so that verification passes for the reason a real one would. [`Recording`](../reference/jud-format.md#recording) has the fields; [`examples/jud/spec-recording.jud`](../../examples/jud/spec-recording.jud) is one.

## Grade the recordings

A directory of recordings beside a cases document is the input of the tuning loop: each recording is graded against its case's labels into judgments, and the bars are read off the judgments. [Tune thresholds](tune-thresholds.md).

## What this does not replace

A mock encodes what the client author believed about the wire. Only a real server can contradict that belief, which is what the ignored live tests and the verification records are for ([How the crate is checked](../project/verification/method.md)).

## Next

- [Tune thresholds](tune-thresholds.md).
- [Run in CI](run-in-ci.md).
