---
title: 0014 A file format for rubrics, cases and recordings
description: The questions a request sends, the policy that reads the answers, the labelled cases the policy is tuned on and the recordings of what a model answered get one YAML format, .jud, with the wire's own shapes, content fingerprints from RFC 8785 canonical JSON and strict reading, instead of staying in code, configuration and notebooks that drift apart.
status: accepted
date: 2026-10-04
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-04
tags: [decisions, jud, format, rubric, cases, recordings, yaml, fingerprint]
---

# 0014 A file format for rubrics, cases and recordings

## Context and problem statement

The crate builds a request in code and verifies the response against it; what happens around the request it has left to the application. In practice the application keeps four things in four places: the questions in Rust, the thresholds in a configuration file or a constant, the labelled examples the thresholds were tuned on in a notebook or a test fixture, and the model's past answers in a recordings directory keyed by a hash only this crate computes. When the model version moves, or a question is reworded, or a label is corrected, nothing ties the four together: the threshold does not say which cases or which model it rests on, the cases do not say which questions they label, and a second tool (a labelling UI, a CI gate, another language's client) cannot read any of it. A comparison with the other Rust clients ([Compared with the other Rust clients](../research/client-comparison.md)) and with a REPL built on the same wire showed the same need met in incompatible ways: one keeps a policy bar beside each question in its own file format, another keeps recordings as cassettes keyed by its own hash, none can read the others'. How should the questions, the policy, the cases and the recordings be kept so that they stay consistent with each other and readable by more than one tool?

## Decision drivers

- A rubric is read and reviewed by people, so the format must be writable by hand, with comments and multi-line text
- The questions must reach the wire unchanged, so the format must not invent a second vocabulary for them
- A threshold must say what it rests on: which labels, which model, which server, when
- An identity must survive a copy between tools and repositories, so it must be computed from content by a published rule, not assigned
- Strict reading: a misspelt field, an off-list label or a gate on the wrong primitive must fail, not pass silently, the same rule the crate applies to answers
- The crate stays generic: nothing in the format may belong to one application
- The crate's existing recordings and `Replay` must keep working

## Considered options

1. One YAML format with three kinds (`rubric`, `cases`, `recording`), wire-shaped questions, a policy map beside them, RFC 8785 fingerprints and a strict reader, with JSON Schemas as the formal statement
2. Adopt the REPL's existing file format and extend it
3. Keep everything in code and configuration; add a `Questions` serialiser and document a convention
4. JSON only, with the same shapes

## Decision outcome

Chosen option: 1. The format is `.jud`, specified in [The .jud format](../jud.md): an envelope of `jud: 1` and `kind`; a rubric whose `questions` are the wire's own objects in the order the model sees them, a `policy` of gates per question id (a Noul's `threshold`, a Choice's or a Score's `confidence` and `fallback`) and a `tuning` block naming the cases by fingerprint, the model, the server and the time; a cases document of labelled states, with a conversation as an array state and `{from_turn: n}` for the turn a Noul becomes true; and a recording of one response with the request's fingerprint. Fingerprints are `sha256:` over RFC 8785 canonical JSON, computed over the questions alone for a rubric, the cases array for a cases document, and `{questions, state}` for a request, model excluded. The reader uses the YAML 1.2 core schema (only `true` and `false` are booleans), refuses unknown fields and every cross-check failure by path, and lowers a rubric through the request builder so a rubric that reads is a request that sends. Three JSON Schemas under `schemas/jud/` state the shapes; the format version is in the envelope and a later version takes the next number.

### Consequences

- Good: the four artefacts name each other by content, so a tuned threshold carries its evidence and an edited case set is visibly a different one
- Good: a rubric is the request, so there is no mapping to maintain and nothing to learn beyond the API's documentation
- Good: another implementation needs only the page, the schemas and the RFC; the known vector lets it check its fingerprints
- Good: the crate's recordings gain `fingerprint`, `rubric`, `server` and `recorded_at`, and `Replay` reads `.jud` recordings by fingerprint beside its own `.json` ones, so a recording made by another tool replays here
- Bad: a dependency on a YAML parser, behind the `jud` feature so a client that builds questions in code does not pay for it
- Bad: strict reading means a new optional field is a new format version, not a quiet addition; accepted so a document is never half-read
- Bad: `Recording` gained fields, so a struct literal in a consumer no longer compiles without `..Recording::new(…)`; a breaking change under the crate's 0.x rule, listed in the CHANGELOG

### Confirmation

`src/jud/` implements the reader, the writer, the gates and the grading; `tests/jud.rs` validates every example document and every document the crate writes against the schemas, pins that the reader and the schemas refuse the same documents, and that `yes` is a string; `src/eval/canonical.rs` carries the RFC 8785 tests and the known vector; `examples/jud_calibration.rs` runs the whole loop over `examples/jud/` and its test asserts the committed gates are the ones the loop derives. `tests/contract.rs` checks the committed `.jud` recordings against the OpenAPI response schema as it checks the `.json` ones.

## Pros and cons of the options

### 1. One YAML format with three kinds

- Good: YAML carries comments, which a reviewed rubric needs, and JSON is valid YAML, so a tool that writes JSON needs nothing more
- Good: three kinds with one envelope keep each file small and single-purpose, and a reader dispatches on `kind`
- Good: fingerprints by a published canonicalisation make identity portable
- Bad: YAML's own history (1.1 booleans, octal) has to be excluded explicitly; the reading rules do so
- Bad: a parser dependency

### 2. Adopt the REPL's format

- Good: an existing user base and files
- Bad: its question shape is its own, not the wire's, so every field needs a mapping and the API's documentation does not describe the file
- Bad: no content identity; a policy bar says nothing about what it was tuned on
- Bad: no cases or recordings kind, so two of the four artefacts still have no home

### 3. Code and configuration, with a convention

- Good: no new dependency, nothing to specify
- Bad: a convention is not readable by a second tool, and nothing ties a threshold to its cases or a case set to its questions
- Bad: the questions still cannot be reviewed without reading Rust

### 4. JSON only

- Good: no YAML parser, no schema subtleties
- Bad: no comments and no multi-line strings, and a rubric is written and reviewed by people; the deciding driver
- Bad: nothing gained that YAML's JSON compatibility does not already give

## More information

[The .jud format](../jud.md) is the specification; the schemas are under `schemas/jud/`; the example documents and the round-trip example are under `examples/jud/` and `examples/jud_calibration.rs`. The weakness the comparison exposed on the way, questions and options sent in alphabetical order, was fixed in the same change: `Questions` and a Choice's `criteria` now keep insertion order, which is what the format's order rule relies on.
