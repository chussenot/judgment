---
title: System One
description: What a System One model answers, why a calibrated probability suits a decision better than generated text, and the three primitives (Noul, Choice, Score) with what each returns.
status: current
last_reviewed: 2026-10-07
tags: [judgment, typesafe, system-one, concepts, calibration]
---

# System One

A generative model asked to classify something answers in prose, or in JSON it was told to produce. A `confidence` field in that JSON is generated text, not a measured probability, and a parse failure becomes a failure of the decision it was meant to inform.

A [System One](https://docs.typesafe.ai) model does not generate text. It evaluates a `state` (any JSON) against typed questions and returns calibrated answers: a probability of yes, one option out of a defined set with the full distribution and a confidence, or a position on ordered levels. Calibrated means the probabilities can be measured against outcomes and held to account: among the answers given at 0.8, about eight in ten are right. That lets code own the workflow. The model supplies a number; the threshold that turns the number into an action is written, tested and tuned by the application.

TypeSafe's hosted models are the Jev family (`jev-latest`); the wire they speak, `POST /v1/systemone`, is spoken by other servers and open-weight models too ([Compatible servers and models](../project/research/compatible-servers-and-models.md)). This crate is a client to the wire, not to one server. It is not affiliated with TypeSafe AI.

## The three primitives

| Primitive | Asks | Answers with | A gate reads |
|---|---|---|---|
| **Noul** | A yes-or-no question | The probability of yes | a threshold on the probability |
| **Choice** | One option among those offered | The chosen option, a probability per option, a confidence | a bar on the confidence, a fallback below it |
| **Score** | A position on a scale of levels | A weighted position, a probability per level, a confidence | the nearest level, a bar on the confidence, a level to reach |

A question is narrow and atomic: one thing, answered with one number. "Which desk, and is it urgent, and should we refund?" is three questions. The model never sees the question's id, only its instructions and criteria, and never sees the threshold, which is why a bar can move without an answer changing.

Confidence is not the probability of being right. For a Choice it is how far the distribution leans toward the chosen option; for a Score, how far it leans toward one level. A 60/40 split between two neighbouring levels is a confidence of 0.40 even when the weighted position is exact. A Noul has no confidence: it is thresholded on its probability. The crate keeps the two apart as types, `Probability` and `Confidence`, so a threshold meant for one is never applied to the other ([How judgment works](how-judgment-works.md)).

## What the model is not asked

Deterministic facts stay in code. Whether an account is on an enterprise plan, whether a ticket is older than a week, whether a refund is above the limit: the application already knows, and asking a model costs tokens to get a probability about a certainty. The model answers what the application cannot compute, and the application decides. The one test the `.jud` format makes on a state, whether a path is present, exists so a question is left out when there is nothing to ask it about ([Rubrics, cases and recordings](rubrics-cases-recordings.md)).

## Why calibration has to be measured

A threshold is a claim about the model's probabilities: above this number, the model is right often enough to act on. The claim is only as good as the examples it was checked on, with the model version that answered them, and it expires when the model moves. That is why the crate keeps recordings of what a model answered, grades them against labelled cases, and reports accuracy with its interval, the Brier score and the calibration error ([Tune thresholds](../guides/tune-thresholds.md)). A probability the application trusts is one it has measured.

## Next

- [How judgment works](how-judgment-works.md): how the crate ties an answer to its question and what it checks before an answer is read.
- [Your first decision from the command line](../start/first-decision-cli.md) or [in Rust](../start/first-decision-rust.md).
