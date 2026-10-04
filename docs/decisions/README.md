---
title: Decisions
description: The architecture decision records that govern the judgment crate's design, how they are numbered, how a record is written, and what has moved since each was accepted.
status: current
last_reviewed: 2026-10-04
tags: [judgment, decisions, adr, madr]
---

# Decisions

A record captures one architecturally significant decision: the problem, the options weighed, the choice and what it costs, in [MADR](https://adr.github.io/madr/) form. The records here govern the crate's own design and the way it is released.

A number is an identifier, never reused or renumbered, so `decision 0003` means the same record wherever it is cited. The crate's first record was written before this repository existed and kept its number when it moved here; the numbers up to 0012 were taken by records about the application the crate was extracted from, which govern nothing in the crate and are not reproduced, so they stay unused. Records written in this repository start at 0013.

| Id | Title | Status |
|---|---|---|
| [0003](0003-typed-handles-between-questions-and-answers.md) | Typed handles between questions and answers | accepted |
| [0013](0013-releases-cut-with-cocogitto-and-published-from-ci.md) | Releases cut with cocogitto and published from CI | accepted |
| [0014](0014-a-file-format-for-rubrics-cases-and-recordings.md) | A file format for rubrics, cases and recordings | accepted |

## Status notes

Records are not edited after acceptance; a fact that has moved since is noted here.

- 0003 was written before the crate existed and moved here with its number on 2026-10-03. Its "More information" section is the one part edited since: the page it pointed to was split, and the half that belongs to an application was dropped when this documentation stopped naming applications.

## Writing a record

One record per decision, as `NNNN-short-title.md` with the number after the highest in the table. The frontmatter is the one every page carries (`title`, `description`, `status`, `last_reviewed`, `tags`) plus `date`, `decision-makers`, `consulted` and `informed`; `status` is `proposed`, `accepted`, `deprecated` or `superseded by NNNN`. The body follows 0003: context and problem statement, decision drivers, the options considered, the outcome with its consequences and how it is confirmed in code or tests, the pros and cons of each option, and a "More information" section naming the pages and code that implement it. State what was rejected and why, and what the choice costs, not only what was chosen.

Add the record to the table above, to the nav in `mkdocs.yml` and to the table in `docs/index.md`, then run `mise run docs:llms`. A record is not edited after acceptance: a fact that moved is a status note here, and a changed decision is a new record that supersedes the old one.
