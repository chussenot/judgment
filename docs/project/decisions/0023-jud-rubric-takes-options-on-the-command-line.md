---
title: 0023 jud RUBRIC takes per-request options on the command line
description: Why the options of a Choice marked options_from request reach jud RUBRIC through --options and --options-file rather than through the state on stdin or a path into it, which 0019 left for later and which tool choice, the main use of a per-request Choice, needs.
status: accepted
date: 2026-10-09
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-09
tags: [judgment, jud, cli, decision, options]
---

# 0023 jud RUBRIC takes per-request options on the command line

## Context and problem statement

A Choice marked `options_from: request` is asked over options that change from one request to the next: the tools an agent may call this turn, the desks staffed now, the tickets a customer has open. The format has carried it since [0016](0016-jud-takes-minor-versions.md), the library lowers it (`Rubric::lower` with `Supplied`), and since [0021](0021-record-eval-and-tune-from-the-command-line.md) `jud record` and `jud eval` ask it over each case's own `options`. [0019](0019-a-command-line-for-the-format.md) left the run itself for later: `jud RUBRIC`, the command a host runs per request, refused such a rubric and said so.

That left the one caller the declaration exists for without a way to use it. Tool choice is a Choice over a shortlist a retriever produced for this request, ranked, often with a `none` option kept static in the rubric; an application that runs `jud` per request had to drop to the crate to ask it.

## Decision drivers

- The state stays the thing being judged. The options are what the model chooses among, not facts about the world; putting them in the state would put them where the questions point the model and where a recording's state fingerprint would count them twice.
- The order is the caller's. A shortlist arrives ranked, and the model reads options in the order sent; nothing on the way may sort them.
- One shape for supplied options everywhere: a case's `options`, `jud lower --options` and a run take the same object.
- What cannot be asked is refused before any call, as every other refusal of the command is.

## Considered options

1. **`--options JSON` and `--options-file PATH` on `jud RUBRIC`**, the object a case's `options` holds: question id, then option key to description.
2. An envelope on stdin, `{"state": ..., "options": ...}`, instead of the bare state.
3. A path into the state per question (`options_from: {path: tools}`), read when the rubric lowers.
4. Leave it to the crate, as 0019 did.

## Decision outcome

Option 1, because it adds to the command without changing what stdin is, and uses the shape the format already defines for a case.

The options are parsed before stdin is read and before the backend is opened, so a malformed argument is status 2 with no key needed and no call made. They are lowered through `Rubric::lower`, which keeps the order given (`Supplied` is an `IndexMap`) and puts them before the static options, and which refuses an unknown question id, a question that does not take options, a key the question already offers, and fewer than 2 or more than 255 options in all. When lowering fails and a question that takes options got none, the message names each such question and the two flags. The two flags exclude each other; the file form is for a shortlist too long for a command line.

### Consequences

- A host runs `jud rubric.jud --options '…' < state.json` per request, with the retriever's ranking intact on the wire.
- The options are part of the request, so a recording made with them answers only the same options under `--replay`, as a case's recording already does.
- `jud RUBRIC`'s statuses keep their meaning: what used to be refused with status 2 for want of options is now asked, and what still cannot be asked is still status 2. The change is additive and ships in a minor release.
- 0019's "options supplied for a Choice that takes them from the request are left for later" is settled here; 0019 stands otherwise. 0021's "cases carry their options where stdin cannot" stays true of stdin; the command line now carries them too. The [index](README.md) records both in its status notes.

## Pros and cons of the options

### 1. Flags on the command line

- Good, because stdin keeps one meaning, the state, for every rubric.
- Good, because the object is the one a case and `jud lower` already take.
- Bad, because a long shortlist on a command line is unwieldy and can meet the system's argument limit; `--options-file` is the answer.

### 2. An envelope on stdin

- Good, because one pipe carries everything a request needs.
- Bad, because stdin would mean two things depending on the rubric, and every pipeline that pipes a bare state would have to know which.

### 3. A path into the state

- Good, because the options would travel with the state with no second argument.
- Bad, because the options would then be part of the state the questions point at, and the format would need a new declaration and a new `apiVersion` ([0018](0018-jud-1-3-takes-the-manifest-envelope.md)). 0016 left deriving options from a state path out until a second application needs it; this decision does not need it, and does not rule it out.

### 4. Leave it to the crate

- Good, because nothing changes.
- Bad, because the main use of a per-request Choice stays out of reach of the command meant to run rubrics.

## More information

- [The jud command line](../../reference/cli.md), `jud [OPTIONS] [RUBRIC]`: the flags, the refusals and an example.
- [The .jud format](../../reference/jud-format.md): `options_from` and lowering.
- `src/bin/jud/run.rs` reads and applies the flags; `src/bin/jud/tools.rs` holds the parser `jud lower` shares; `tests/jud_cli.rs` holds the tests (the order on the wire, the file form, each refusal).
