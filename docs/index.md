---
title: judgment documentation
description: Map of the judgment crate's documentation, what each page answers, and where the crate's documentation ends and an application's begins.
status: current
last_reviewed: 2026-10-03
tags: [judgment, index]
---

# judgment documentation

The [README](../README.md) says what the crate is for, what it guarantees and how to depend on it, and maps TypeSafe's four patterns onto its types. These pages say how it works, what has been verified against real servers, and why it is shaped the way it is. The rustdoc (`cargo doc -p judgment --open`) is the reference for every type, error and default.

## By question

| You want to know | Read |
|---|---|
| How a question's handle ties it to its answer, how a response is checked, how the retry loop decides | [How judgment works](design.md) |
| Which crate types carry speculative fan-out, confidence-gated routing, composite scoring and intent routing | [README, Patterns](../README.md#patterns) and the examples under `examples/` |
| What the hosted TypeSafe API does with what the crate sends, and with what it refuses to send | [Against the hosted TypeSafe API](verification/hosted-typesafe.md) |
| Whether the crate works against a second implementation of the wire, what the benchmark measured, and what each `laya-serve` release changed on the wire | [Against Laya typed-decisions](verification/laya-typed-decisions.md) |
| What the other Rust clients and the official SDKs do, and which of it the crate adopted | [System One client libraries](research/system-one-client-libraries.md) |
| Which servers and models speak the wire, how close the open ones are to Jev on the Decision Index, and what each one's limits mean for a consumer | [Compatible servers and models](research/compatible-servers-and-models.md) |
| Why an answer is read through a typed handle rather than a string key | [Decision 0003](decisions/0003-typed-handles-between-questions-and-answers.md) |
| How a version is cut from the commits and published to crates.io, and what to set up once | [Releasing](releasing.md) |
| Why releases are cut with cocogitto from Conventional Commits and published by CI from a tag | [Decision 0013](decisions/0013-releases-cut-with-cocogitto-and-published-from-ci.md) |
| What changed in each release | [CHANGELOG](../CHANGELOG.md) |
| Everything, as an agent or a model reads it | [llms.txt](llms.txt), the index; [llms-full.txt](llms-full.txt), every page in one file |

## Verification

A mock encodes what the client author believed about the wire; only a real server can contradict that belief. Two records say what real servers did:

- [Against the hosted TypeSafe API](verification/hosted-typesafe.md): the live tests (`tests/live.rs`) and about fifty probes past the builder's limits, against `jev-1.13.0`.
- [Against Laya typed-decisions](verification/laya-typed-decisions.md): the same tests against an open-weights server, `laya-serve` 0.3.20 and then 0.3.24, and the 400-case benchmark replayed through the crate.

## What is not here

The crate began as the client layer of an alert-triage application and moved to this repository with its history on 2026-10-03. How any one application uses the crate, its questions, its thresholds, its evaluation harness and its choice of model provider belong to that application's own documentation, and this set does not name the application it came from. A page belongs here when it would still be true, and still be needed, if no particular application existed.

## Conventions

- Every page starts with YAML frontmatter: `title`, `description`, `status`, `last_reviewed`, `tags`.
- `llms.txt` and `llms-full.txt` are generated from [`mkdocs.yml`](../mkdocs.yml) and the frontmatter by `scripts/gen-llms-txt.sh`; never edit them. A new page goes in the nav.
- Links inside this documentation are relative, so they work on GitHub, in a TechDocs build and in a packaged crate. A link to another repository's page is an absolute URL, because it leaves the crate.
- Paths in the crate's sources and docs are relative to the crate: `docs/design.md`, `tests/live.rs`.
