---
title: 0016 The .jud format takes minor versions, and 1.1 makes the request depend on the state
description: The .jud format gains a minor version, 1.1, which only adds; it lets a rubric declare when a question or an instruction part is sent and that a Choice's options come with the request, lets a gate carry bands, a level threshold and strict comparison, and reserves top-level x- keys, so that a real application's questions and policy fit in the file.
status: accepted
date: 2026-10-04
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-04
tags: [decisions, jud, format, versioning, rubric, policy]
---

# 0016 The .jud format takes minor versions, and 1.1 makes the request depend on the state

## Context and problem statement

[Decision 0014](0014-a-file-format-for-rubrics-cases-and-recordings.md) shipped `jud: 1` with a strict reader: a field the format does not define is refused, so "a later version that adds a field takes the next number". The first application to move its questions into a rubric found three gaps (issues [#8](https://github.com/chussenot/judgment/issues/8), [#9](https://github.com/chussenot/judgment/issues/9), [#10](https://github.com/chussenot/judgment/issues/10)):

- Its request varies with the state. Two Choices are asked over options known only at request time, an instruction part is sent only when the state carries what it describes, and two questions are asked only when a list in the state is not empty. A `jud: 1` rubric cannot say so, so the application kept those rules in code and its rubric listed example options that are never sent.
- Its policy has two bars on one question (route automatically, ask to confirm, send to a person) and a threshold on a Score's level (escalate from the third level up). A `jud: 1` gate has one bar and no level threshold, so every threshold stayed in the application's own configuration, and the format's `tuning` provenance went unused.
- A shared instruction part written once as a YAML anchor had to live inside the first question that uses it, and a question could not be moved above that one.

How should the format grow, and what should the first addition hold?

## Decision drivers

- A document written today must mean the same thing tomorrow, and a reader of today must not half-read a document of tomorrow
- A rubric should state the whole request, so that a second tool (a case runner, a labelling UI) builds the same request from the file and the state
- Deterministic logic stays in code; the format must not grow an expression language
- The crate's own checks on a request (2 to 255 options, 2 to 10 levels) must still hold on what is sent
- A `jud: 1` rubric keeps its fingerprint, so committed evaluation runs stay current

## Considered options

1. Minor versions that only add; `jud: 1.1` with presence conditions, request-supplied options, bands, level thresholds, strict comparison and top-level `x-` keys
2. `jud: 2`, a new major version with the same additions
3. Accept unknown fields (warn and ignore) instead of versioning
4. Leave the format at 1 and keep these rules in each application

## Decision outcome

Chosen option: 1. The envelope's `jud` is `1` or `1.1`. A minor version only adds; a `1.1` reader reads every `jud: 1` document as a `1` reader does, a document that uses a 1.1 feature must say `jud: 1.1` (a reader refuses it otherwise, naming the feature), and a writer declares `1.1` only when it has to. Version 1.1 adds:

- A 1.1 field counts by its presence, whatever its value, and `null` is not a value of one, so a `jud: 1` document cannot carry a 1.1 key even emptied.
- On a question: `when`, a state path that must be present for the question to be asked; `part_when`, instruction part names to state paths; and on a Choice `options_from: request`, options supplied per request before the static ones, with fewer than two static options allowed. `Rubric::lower(state, supplied)` builds the request through the builder's checks. A state path is dot-separated keys, and "present" is the one test: not `null`, not an empty string, array or object. No negation, no comparison, no expression.
- On a case: `options`, the options supplied for its request, so a case is a complete request; binding refuses a label on a question the case's state does not ask.
- On a gate: `bands`, ordered confidence bars with named verdicts, generalising `confidence`; `level_at_least` on a Score, by level text or index; `strict`, `>` for `≥` at every bar. The verdicts carry the band and whether the level was reached.
- On every document: top-level `x-` keys, ignored by every reading and part of no fingerprint, kept on a round trip.
- In the crate: `eval::tuning::level_sweep` and `best_level` read a level threshold off a table, reading the nearest level as `apply` does.

### Consequences

- Good, because a rubric states its whole request: the file and a state (and a case's options) are enough to rebuild what was sent, in any implementation
- Good, because an application's two-bar and level-threshold policies fit in the rubric's `policy`, with their `tuning`
- Good, because a `jud: 1` document keeps its meaning, its fingerprint and how it is written: a question without declarations serialises exactly as before
- Bad, because the crate's API breaks under its 0.x rule: `Rubric::questions` holds `RubricQuestion`s instead of being a `Questions`, a request comes from `Rubric::lower` or `Case::request`, `Rubric::apply` takes the request it reads, and two verdicts gain fields
- Bad, because "present" is coarse: a condition on a value (`severity == critical`) needs the application to decide it and put the result in the state, which is the intended split but one more step
- Bad, because options supplied per request are not in the state, so a case must repeat them under `options`; deriving them from a state path (`options_from: {path, key, description}`) was left out until a second application needs it

### Confirmation

`src/jud/rubric.rs` tests (a declaration-free rubric's fingerprint is its lowered request's; lowering per state and supplied options; each 1.1 feature refused under `jud: 1`; declarations, bands and level thresholds checked where written; verdicts with bands, levels and strict bars; `x-` keys and anchors round trip without changing the fingerprint); `src/jud/cases.rs` tests (case options, labels on questions not asked); `tests/jud.rs` (the 1.1 example pair `examples/jud/routing.jud` and `routing-cases.jud` validates against the schemas and binds, and the schemas and the reader refuse the same 1.1 documents); `src/eval/tuning.rs` (the level sweep).

## Pros and cons of the options

### 1. Minor versions that only add

- Good, because every existing document and reader stays valid, and the version still tells a reader what it is about to read
- Good, because the additions are the smallest that fit a real application: presence, not expressions; supplied options, not a query language
- Bad, because a reader now carries a feature-to-version table, and every addition must be checked to be purely additive

### 2. A new major version

- Good, because it is what decision 0014 said a new field would take
- Bad, because nothing in 1 changes meaning; a major bump would make every `jud: 1` reader refuse documents that only add, and would force writers to choose between two incompatible formats for no gain

### 3. Accept unknown fields

- Good, because a reader would never refuse a newer document
- Bad, because a misspelt `treshold` would silently be ignored, and a document would be half-read: the failure strict reading exists to prevent

### 4. Leave the rules in each application

- Good, because the format stays small
- Bad, because the rubric then misstates the request (example options that are never sent) and two tools cannot rebuild the same request from it, which is what the format is for

## More information

[The .jud format](../jud.md) is the specification of both versions; the schemas under `schemas/jud/` state the 1.1 shapes. The numbers 0015 and below 0013 are taken by records outside this repository and stay unused here.
