---
title: Your first decision from the command line
description: From nothing to a verdict with the jud command and no account, by replaying a recorded answer; then the same command against the hosted API or a local model.
status: current
last_reviewed: 2026-10-08
tags: [judgment, jud, cli, tutorial, getting-started, replay]
---

# Your first decision from the command line

**You will** read a rubric with the crate's own reader, evaluate a JSON state against it from a recorded answer with no key and no network, see what the rubric asked, and point the same command at a real model. **Prerequisites:** the `jud` command ([Install](install.md)); `git`; no account until the last step. No Rust is needed.

## 1. Get the example files

The repository holds a rubric, the labelled cases its policy was tuned on, and recorded answers to those cases. The answers are scripted and labelled `jev-1.13.0`: they let the command run with no key, and they are not what that model says. Step 5 asks a real model.

```sh
git clone https://github.com/chussenot/judgment && cd judgment
jud --version
```

## 2. Read the rubric

`examples/jud/triage.jud` sorts a support message: three questions and a policy. `jud check` reads it as the crate does and prints what it found:

```sh
jud check examples/jud/triage.jud examples/jud/triage-cases.jud
```

<!-- transcript: jud check examples/jud/triage.jud examples/jud/triage-cases.jud -->
```text
rubric    examples/jud/triage.jud: name inbox-triage, jud/v1.3, 3 questions, 3 gates (tuned)
          questions sha256:fda347c2cba6deaa8ad784360c46a8289adbc0284b373c3d9162ce11f3e7ac8a
          policy    sha256:1585094f59711426a98044a00ded2ec747010b5369e941f33a2732a50262e99f
          tuned on  sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6
cases     examples/jud/triage-cases.jud: name inbox-triage-cases, jud/v1.3, 7 cases, bound to examples/jud/triage.jud
          cases     sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6
2 documents, 0 refused
```

Open the file. Under `spec.questions` are the questions the model is asked, in the shape the wire sends them; under `spec.policy` the bar at which each answer becomes an action; under `spec.tuning` which cases, which model and which server the bars were tuned on. `tuned on` is the fingerprint of the cases document, which the `cases sha256:` line below shows is the same `sha256:` as the cases file beside it: the policy rests on exactly those labels ([Rubrics, cases and recordings](../concepts/rubrics-cases-recordings.md)).

## 3. Evaluate a state from a recording

A run takes the state on stdin. `--replay DIR` answers from the recordings under a directory instead of a server, so this step needs no key and no network:

```sh
echo '{"message": "This is the third time I'"'"'m writing. I was charged twice last month and nobody has refunded me. Fix it today or I cancel."}' \
  | jud --replay examples/recordings/jud_calibration examples/jud/triage.jud
```

<!-- transcript: replay refund-angry -->
```json
{
  "actionable": {
    "verdict": "yes",
    "probability": 0.97
  },
  "desk": {
    "verdict": "option",
    "key": "billing",
    "confidence": 0.87
  },
  "tone": {
    "verdict": "level",
    "index": 2,
    "label": "angry",
    "value": 1.86,
    "confidence": 0.84
  }
}
```

One verdict per question, keyed by question id, in the rubric's order. The Noul gives `yes` with its probability, the Choice the option with its confidence, the Score the nearest level with its index, label, weighted value and confidence. The recording was found by the fingerprint of the state and the questions, verified against the questions as a server's response would be, and read through the policy; `jq '.desk.key'` reads what a script needs.

Try a state nobody recorded:

```sh
echo '{"message": "hello"}' | jud --replay examples/recordings/jud_calibration examples/jud/triage.jud
```

```text
jud: no recording under examples/recordings/jud_calibration answers this state and rubric: no recording for request 4748f6f4e9a9827d; record it first
```

Status 1: a replay refuses what it lacks rather than guessing, which is what makes it a test ([Record, replay and test](../guides/record-replay-and-test.md)).

## 4. See what the rubric asked

```sh
jud lower examples/jud/triage.jud --state '{"message": "hello"}'
```

prints the request the rubric lowers to for that state: the questions as the wire carries them, `type`, `instructions`, `criteria`. The policy is not in it. The model never sees the thresholds, which is why they can move without an answer changing.

## 5. Ask a real model

The same command without `--replay` asks the configured backend. With a key from [TypeSafe](https://docs.typesafe.ai), one call:

```sh
export TYPESAFE_API_KEY=...
jud config                                            # "api_key": "environment", the hosted API, jev-latest
echo '{"message": "My payouts have been failing for 3 days."}' | jud examples/jud/triage.jud
```

Without a key the run is refused before any call, with status 2 and a message naming the variable and the configuration file. Without an account, an open-weight model on your own machine answers the same command through a configuration file: [Configure a backend](../guides/configure-a-backend.md) sets up `tev1` on Ollama in three commands.

## Next

- [Write a rubric](../guides/write-a-rubric.md) for a decision of your own.
- [Tune thresholds](../guides/tune-thresholds.md): grade the recordings you replayed against the labels with `jud eval`, and see how the bars were chosen with `jud tune`.
- [The jud command line](../reference/cli.md): every subcommand, flag and exit status.
- [Run in CI](../guides/run-in-ci.md).
