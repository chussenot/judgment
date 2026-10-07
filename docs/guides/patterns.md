---
title: Patterns
description: TypeSafe's four patterns, speculative fan-out, confidence-gated routing, composite scoring and intent routing, mapped onto the crate's types, with one runnable example each that replays committed recordings and three things the recordings teach about thresholds.
status: current
last_reviewed: 2026-10-07
tags: [judgment, patterns, examples, thresholds, how-to]
---

# Patterns

**You will** know which type carries each of the shapes a System One call takes inside a larger program, and run an example of each with no key. **Prerequisites:** a checkout of the repository and a Rust toolchain; [How judgment works](../concepts/how-judgment-works.md).

TypeSafe documents four [patterns](https://docs.typesafe.ai/patterns). Each has a runnable example in this repository, written to the documentation page's own scenario and thresholds, in a domain of its own. The question each example answers is not "does the call work" but "which type carries this shape, and what does the recorded answer teach about the threshold".

| Pattern | The shape | What carries it in this crate | Run |
|---|---|---|---|
| [Speculative fan-out](https://docs.typesafe.ai/patterns/fan-out) | Every question the decision tree might need goes in one request; the branch that is taken reads its answers and the others go unread. Questions are answered in parallel, so the extra ones cost input tokens, not latency | One `Questions` with a typed `Handle` per question; each branch reads only its handles | `cargo run --example fan_out` |
| [Confidence-gated routing](https://docs.typesafe.ai/patterns/confidence-routing) | The answer says what, the confidence says whether to act: a floor sends uncertainty to a person, and each action sets its own bar by what a wrong one would cost | `Choice::confidence`, a `Confidence` that cannot be thresholded as a `Probability`; `confidence_from_probabilities` for the formula behind it | `cargo run --example confidence_routing` |
| [Composite scoring](https://docs.typesafe.ai/patterns/composite-scoring) | Several atomic Scores, normalised and combined with weights the code owns; a new weighting needs no new inference | `Score::value` per dimension; `Recorder` and `Replay`, so the weights change over recorded answers | `cargo run --example composite_scoring -- --weights 0.5,0.1,0.3,0.1` |
| [Intent routing](https://docs.typesafe.ai/patterns/intent-routing) | A cheap classifier in front of expensive handlers, so code, a specialist model or a person each get only what needs them; a second question gates the escalation | A `Choice` and a `Score` in one request, a `Confidence` read off each | `cargo run --example intent_routing` |

Each example replays its committed recordings (`examples/recordings/<name>/`, `jev-1.13.0`'s answers of 2026-10-03) by default, so it runs with no key and no network and prints every answer next to the decision it led to. `-- --live` sends the same requests to the hosted API (`TYPESAFE_API_KEY`; `TYPESAFE_BASE_URL` for another server, `TYPESAFE_MODEL` for another model), and `-- --record` does that and rewrites the recordings. Each example ends in a test over its recordings that `cargo test` runs: the request hash covers the questions, so a question changed without re-recording fails the gate rather than the next reader.

Two more examples run the `.jud` loop and the typed-decisions benchmark: `jud_calibration` ([Tune thresholds](tune-thresholds.md)) and `typed_decisions` ([Against Laya typed-decisions](../project/verification/laya-typed-decisions.md)).

## What the recordings teach

Three things the pattern pages say that the examples make concrete.

- **Thresholds are starting points, not constants.** The hosted model's probabilities are not deterministic, and the spread grows with ambiguity: identical requests moved by up to 0.05 on a clear-cut input and by 0.19 in probability (0.28 in confidence) on an ambiguous one, the decision holding every time ([hosted API record](../project/verification/hosted-typesafe.md)). A recording is one draw, and a threshold needs its margin most where the input is least clear.
- **A Noul has no confidence.** It is thresholded on its probability, where a Choice or a Score has both. The routing example gates on `Confidence`, the fan-out example on `Probability`, and the types keep the two from being compared.
- **A Score's confidence falls fast when probability splits between neighbouring levels.** A 60/40 split on three levels is a confidence of 0.40, so the intent example's second gate, a 0.5 floor on the complexity's confidence, sends mild complaints to a person on `jev-1.13.0`. That floor, or the number of levels, is the first thing to tune.

## Other shapes

The same primitives take other shapes, each with a [cookbook](https://docs.typesafe.ai/cookbooks): select a value or a span from candidates found in code rather than generate it; rerank retrieved passages with one question per pair; verify a claim against its evidence and escalate what fails; turn scores into features for a classical model. None has an example here yet. They are the same `Questions`, handles and backends arranged differently.

## Next

- [Record, replay and test](record-replay-and-test.md): how the examples' recordings are made and kept.
