---
title: Glossary
description: The terms the documentation uses, each defined once, with the page that owns it.
status: current
last_reviewed: 2026-10-07
tags: [judgment, glossary, reference]
---

# Glossary

**Backend**
: A source of answers behind the `SystemOne` trait: the `Client`, a `Fake`, a `Recorder`, a `Replay`. Every backend verifies its response against the questions before returning it. [How judgment works](../concepts/how-judgment-works.md).

**Band**
: One row of a gate's `bands`: a confidence bar (`at_least`) and the verdict an answer at or above it receives. Bands are listed highest first; below the last, the answer is deferred. [Policy](jud-format.md#policy).

**Case**
: One labelled state in a Cases document: the state the questions are asked about, the expected answers for the questions the labeller was sure of, and optionally the options supplied for the request. [Rubrics, cases and recordings](../concepts/rubrics-cases-recordings.md), [`Cases`](jud-format.md#cases).

**Choice**
: The primitive that picks one option out of a closed set. The answer is the chosen option, a probability per option, and a confidence. [System One](../concepts/system-one.md).

**Confidence**
: For a Choice or a Score, how far the distribution leans toward its answer, in `[0, 1]`. It is not a probability of being right, and the crate's `Confidence` type cannot be compared with a `Probability` at a threshold. A Noul has no confidence. [System One](../concepts/system-one.md).

**Fake**
: The backend that answers from a scripted table, refuses a question it has no answer for or an answer the question could not produce, and remembers every call. [Record, replay and test](../guides/record-replay-and-test.md).

**Fingerprint**
: `sha256:` over the RFC 8785 canonical JSON of a value inside a document's `spec`: a rubric's questions, a cases document's cases, or a request (state and questions). The same content has one fingerprint in every implementation. [Fingerprints](jud-format.md#fingerprints).

**Gate**
: A rubric's policy entry for one question: the bar at which its answer becomes an action, with a fallback, bands or a level to reach. [Policy](jud-format.md#policy).

**Lower**
: To build the request for one state from a rubric: the questions whose `when` holds, with the parts their `part_when` keeps, and the supplied options in place. `Rubric::lower` in the crate, `jud lower` on the command line. [Declarations](jud-format.md#declarations).

**Noul**
: TypeSafe's yes-or-no primitive. The answer is the probability of yes. [System One](../concepts/system-one.md).

**Policy**
: The map of gates in a rubric, keyed by question id. Never sent to the model. [Policy](jud-format.md#policy).

**Policy fingerprint**
: The fingerprint of a rubric's `spec.policy` map alone, so a moved bar is as visible as a changed question. [Rubric fingerprint](jud-format.md#fingerprint).

**Recording**
: One model response to one request, kept with the request's fingerprint, the server, and the time, so a run can be replayed without a call. [`Recording`](jud-format.md#recording).

**Replay**
: The backend that answers from a directory of recordings, found by request fingerprint or request hash, verified before they are returned; a request nobody recorded is an error. [Record, replay and test](../guides/record-replay-and-test.md).

**Rubric**
: The decision as a document: the questions a model is asked about a state, in wire shape, and the policy that reads the answers. [`Rubric`](jud-format.md#rubric).

**Score**
: The primitive that places a state on ordered levels. The answer is a weighted position, a probability per level, and a confidence; the nearest level is what a gate reads. [System One](../concepts/system-one.md).

**State**
: The JSON value the questions are asked about: the `state` field of a request, a case's `state`, what `jud` reads on stdin.

**System One**
: TypeSafe's family of decision models and the wire they speak (`POST /v1/systemone`): typed questions in, calibrated probabilities out, no generated text. Also the crate's `SystemOne` trait. [System One](../concepts/system-one.md).

**Tuning**
: The provenance block of a rubric's policy: which cases, which model, which server and when the gates were tuned on. Also the act of sweeping a bar over graded recordings. [Tune thresholds](../guides/tune-thresholds.md).

**Verdict**
: What the policy makes of one answer: yes or no, an option, a level, or deferred, with the numbers behind it. Derived when a response is read, never stored. [Policy](jud-format.md#policy).
