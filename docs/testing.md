---
title: Testing without the model
description: How an application tests its decisions with no key and no network, with the Fake backend that refuses an answer the question does not fit, the Recorder and Replay pair keyed by a content hash, and the eval module that grades recordings against labels.
status: current
last_reviewed: 2026-10-04
tags: [judgment, testing, fake, replay, evaluation]
---

# Testing without the model

A decision made on a probability is hard to test against a live model: the call costs money, the answer moves between runs, and a test that passes against the real server tells you nothing about the branch the model did not take. The crate separates the three things a test needs: a backend that answers what the test scripts, a recording of what the real model once said, and a way to grade those recordings against labels. All three sit behind the same `SystemOne` trait as the client, so the code under test does not change.

## A scripted backend

`Fake` answers from a table, refuses a question it has no answer for, and remembers every call, so a test checks the decision and what was asked. It verifies its response like the client does, so a scripted option the question does not offer, or an answer of the wrong primitive, fails the call (and is not remembered) instead of passing a test the real client would fail. A scripted Score is only its probabilities: its legend is the levels of the question it answers, echoed as a server echoes them.

```rust
let backend = Fake::new()
    .choice("department", [("billing", 0.9), ("technical", 0.1)], 0.8)?
    .noul("is_urgent", 0.2)?;
let response = backend.answer(&state, "any-model", &questions).await?;
assert_eq!(backend.calls()[0].question_ids, ["department", "is_urgent"]);
```

The fake is where the type system pays off in tests: a scripted answer that the question could not produce is a compile error when the option is an enum variant, and a runtime refusal when it is a dynamic key.

## Record once, replay forever

A `Recorder` writes what a real backend answered; a `Replay` over the same directory answers the same requests later, with no key and no network:

```rust
let recorder = Recorder::new(Client::from_env()?, "recordings");
let live = recorder.answer(&state, "jev-latest", &questions).await?;
let replay = Replay::open(Path::new("recordings"))?;
let again = replay.answer(&state, "jev-latest", &questions).await?;
assert_eq!(live, again);
```

A recording is keyed by a 64-bit hash of the canonical, key-sorted JSON of the state and the questions, so a question changed without re-recording misses the recording and fails the test rather than grading old answers under new questions; `eval::fingerprint` exposes the same hash. The hash is the same whichever client serialised the request, so two clients that order the questions differently share a recording. Each recording also carries the request's `sha256:` fingerprint ([the .jud format](jud.md), RFC 8785 canonical JSON), the key another tool computes the same way, and the time it was made; with the `jud` feature, `Replay` reads `.jud` recordings by that fingerprint beside its own `.json` ones. Replay verifies each recording against the questions as the client would, so a recording made under one set of options cannot answer a question that offers another.

The committed examples work this way: each pattern example under `examples/` replays the recordings beside it by default, runs live with `--live`, re-records with `--record`, and carries a test over its recordings that `cargo test` runs ([Patterns](patterns.md)).

## Grading recordings

`eval` turns recordings and labels into numbers: one `Judgment` per answer and label, then per question the accuracy with a 95% Wilson interval, the Brier score, the calibration error and the mean confidence when right and when wrong. The interval is there because an accuracy on three labelled cases and one on three hundred read the same without it. The Brier score is the multi-class sum, which the rustdoc states because other tools use a different form and the numbers are not comparable.

What the module deliberately leaves to the application: the thresholds. A probability becomes an action in code the application owns and tunes; the crate measures whether the probabilities deserve the trust the thresholds place in them, and `eval::tuning` sweeps a threshold or a confidence bar over the judgments so the number is read off a table rather than guessed. A change of model is a change of policy that needs a replay before it ships, which is the reason the recordings, the hash and the metrics are one module. [The .jud format](jud.md) is where the questions, the thresholds, the labelled cases and the recordings live as files that name each other by content, and `examples/jud_calibration.rs` runs the loop from cases to a tuned policy.

## What this does not replace

A mock encodes what the client author believed about the wire. Only a real server can contradict that belief, which is what the ignored live tests and the verification records are for ([How the crate is checked](verification/method.md)).
