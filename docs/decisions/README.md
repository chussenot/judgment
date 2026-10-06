---
title: Decisions
description: The architecture decision records that govern the judgment crate's design, how they are numbered, how a record is written, and what has moved since each was accepted.
status: current
last_reviewed: 2026-10-06
tags: [judgment, decisions, adr, madr]
---

# Decisions

A record captures one architecturally significant decision: the problem, the options weighed, the choice and what it costs, in [MADR](https://adr.github.io/madr/) form. The records here govern the crate's own design and the way it is released.

A number is an identifier, never reused or renumbered, so `decision 0003` means the same record wherever it is cited. The crate's first record was written before this repository existed and kept its number when it moved here; the numbers up to 0012 were taken by records about the application the crate was extracted from, which govern nothing in the crate and are not reproduced, so they stay unused. Records written in this repository start at 0013. The sequence is shared with records written outside it, so a number taken there (0015) is skipped here too.

| Id | Title | Status |
|---|---|---|
| [0003](0003-typed-handles-between-questions-and-answers.md) | Typed handles between questions and answers | accepted |
| [0013](0013-releases-cut-with-cocogitto-and-published-from-ci.md) | Releases cut with cocogitto and published from CI | accepted |
| [0014](0014-a-file-format-for-rubrics-cases-and-recordings.md) | A file format for rubrics, cases and recordings | accepted |
| [0016](0016-jud-takes-minor-versions.md) | The .jud format takes minor versions, and 1.1 makes the request depend on the state | superseded by 0018 |
| [0017](0017-jud-1-2-refuses-what-a-reviewer-cannot-see.md) | The .jud format 1.2 refuses what a reviewer cannot see, and names the policy | superseded by 0018 |
| [0018](0018-jud-1-3-takes-the-manifest-envelope.md) | The .jud format 1.3 takes the manifest envelope and reads one apiVersion | accepted |

## Status notes

Records are not edited after acceptance; a fact that has moved since is noted here.

- 0003 was written before the crate existed and moved here with its number on 2026-10-03. Its "More information" section is the one part edited since: the page it pointed to was split, and the half that belongs to an application was dropped when this documentation stopped naming applications.

- 0016 says a 1.1 reader reads every `jud: 1` document as a 1 reader does. [0017](0017-jud-1-2-refuses-what-a-reviewer-cannot-see.md) makes one narrow exception: a reader refuses, in every version, a merge key, a tag the core schema does not define and an id that is not a name, none of which a conforming document ever carried.

- 0014 describes the envelope as `jud: 1` and `kind`; since [0018](0018-jud-1-3-takes-the-manifest-envelope.md) the envelope is a manifest's and the version is `apiVersion`. 0014's reasons for one format with content fingerprints stand.
- 0014 says "a later version that adds a field takes the next number". [0016](0016-jud-takes-minor-versions.md) read that as the next minor number for a purely additive change (1.1), and kept the next major number for a change of meaning.

- 0016 and 0017 are superseded by [0018](0018-jud-1-3-takes-the-manifest-envelope.md) on 2026-10-06: the envelope is a manifest's, a reader reads exactly one `apiVersion` and any change of the format takes a new one, so the compatibility rule of 0016 and the exception 0017 made to it are both withdrawn. What the two records added to the format (declarations, bands and level gates, names, the refusal of what a reviewer cannot see, the policy fingerprint) stands as rules of the format, and 0018 is the one page that maps the earlier keys onto the new envelope.

## Writing a record

One record per decision, as `NNNN-short-title.md` with the number after the highest in the table, skipping one taken outside this repository (0015 is). The frontmatter is the one every page carries (`title`, `description`, `status`, `last_reviewed`, `tags`) plus `date`, `decision-makers`, `consulted` and `informed`; `status` is `proposed`, `accepted`, `deprecated` or `superseded by NNNN`. The body follows 0003: context and problem statement, decision drivers, the options considered, the outcome with its consequences and how it is confirmed in code or tests, the pros and cons of each option, and a "More information" section naming the pages and code that implement it. State what was rejected and why, and what the choice costs, not only what was chosen.

Add the record to the table above, to the nav in `mkdocs.yml` and to the table in `docs/index.md`, then run `mise run docs:llms`. A record is not edited after acceptance: a fact that moved is a status note here, and a changed decision is a new record that supersedes the old one.
