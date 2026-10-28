---
title: Decisions
description: The architecture decision records that govern the judgment crate's design, how they are numbered, how a record is written, and what has moved since each was accepted.
status: current
last_reviewed: 2026-10-25
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
| [0019](0019-a-command-line-for-the-format.md) | A command line for the format, released as a binary | accepted |
| [0020](0020-how-a-v1-minor-is-spelled.md) | How a v1 minor of the .jud format is spelled | accepted |
| [0021](0021-record-eval-and-tune-from-the-command-line.md) | Record, evaluate and tune from the command line | accepted |
| [0022](0022-jud-reports-what-the-tuning-loop-counted-by-hand.md) | jud reports what the tuning loop counted by hand | accepted |
| [0023](0023-jud-rubric-takes-options-on-the-command-line.md) | jud RUBRIC takes per-request options on the command line | accepted |
| [0024](0024-images-are-part-of-the-request.md) | Images are part of the request, not a call option | accepted |
| [0025](0025-a-document-names-an-image-by-path.md) | A document names an image by a path relative to itself | accepted |
| [0026](0026-a-box-is-graded-by-intersection-over-union.md) | A Box is graded by intersection over union | accepted |
| [0027](0027-long-context-is-a-switch-on-the-call.md) | Long context is a switch on the call, not a request field or a default | accepted |
| [0028](0028-multipart-is-a-feature-and-a-switch.md) | Multipart upload is a feature and a switch on the call, off by default | accepted |
| [0029](0029-request-compression-is-a-feature-and-a-switch.md) | Request compression is a feature and a switch, off by default | accepted |
| [0030](0030-an-absent-usage-is-recorded-as-zeros.md) | An absent usage is recorded as zeros, not kept absent | accepted |

## Status notes

Records are not edited after acceptance; a fact that has moved since is noted here.

- 0003 was written before the crate existed and moved here with its number on 2026-10-03. Its "More information" section is the one part edited since: the page it pointed to was split, and the half that belongs to an application was dropped when this documentation stopped naming applications.

- 0016 says a 1.1 reader reads every `jud: 1` document as a 1 reader does. [0017](0017-jud-1-2-refuses-what-a-reviewer-cannot-see.md) makes one narrow exception: a reader refuses, in every version, a merge key, a tag the core schema does not define and an id that is not a name, none of which a conforming document ever carried.

- 0014 describes the envelope as `jud: 1` and `kind`; since [0018](0018-jud-1-3-takes-the-manifest-envelope.md) the envelope is a manifest's and the version is `apiVersion`. 0014's reasons for one format with content fingerprints stand.
- 0014 says "a later version that adds a field takes the next number". [0016](0016-jud-takes-minor-versions.md) read that as the next minor number for a purely additive change (1.1), and kept the next major number for a change of meaning.

- 0016 and 0017 are superseded by [0018](0018-jud-1-3-takes-the-manifest-envelope.md) on 2026-10-06: the envelope is a manifest's, a reader reads exactly one `apiVersion` and any change of the format takes a new one, so the compatibility rule of 0016 and the exception 0017 made to it are both withdrawn. What the two records added to the format (declarations, bands and level gates, names, the refusal of what a reviewer cannot see, the policy fingerprint) stands as rules of the format, and 0018 is the one page that maps the earlier keys onto the new envelope.

- 0018 says any change of the format, additive or not, takes a new `apiVersion` that a reader of the earlier one refuses. [Stability](../../reference/stability.md), on 2026-10-06, promises that a `v1` minor only adds and that readers accept every `v1` document for at least twelve months after a later minor is published; a reader of a later minor therefore reads earlier `v1` documents, and only a change of meaning takes a new major. [0020](0020-how-a-v1-minor-is-spelled.md), accepted on 2026-10-15, spells the minor as `metadata.minor` under `apiVersion: jud/v1`; 0018's one-`apiVersion` rule stands for the string, and its rule that any change takes a new string is narrowed to a change of meaning. The spelling `jud/v1.3` that 0018 introduced is read as minor 3.

- 0021 extends the command [0019](0019-a-command-line-for-the-format.md) defined: three subcommands (`record`, `eval`, `tune`), exit status 3 for `jud eval` alone, and two places that write files (`jud record --out` and `jud tune --out`). 0019 stands: its statuses 0, 1 and 2 keep their meaning, and `jud RUBRIC` still evaluates one state from stdin. 0021 was accepted on 2026-10-08, when the subcommands were implemented; its "As implemented" section records what the implementation settled, and where it differs from the proposal the implementation is the contract.
- 0022 adds to 0021's commands without changing them: per-outcome counts, a majority and three signals in `jud eval`, `jud record --dry-run`, and a fourth subcommand, `jud split`, a third place that writes files.
- 0019 says options supplied for a Choice that takes them from the request "are left for later". [0023](0023-jud-rubric-takes-options-on-the-command-line.md) settles it: `jud RUBRIC` takes them through `--options` and `--options-file`. 0019 stands otherwise.
- 0021 says cases carry their options "where stdin cannot". Stdin still cannot; since [0023](0023-jud-rubric-takes-options-on-the-command-line.md) the command line can.
- 0027's consequences say the `jud` command has no flag or setting for long context yet. It has both since the release after 0027: `long_context: true` in its configuration file and `--long-context` on `jud RUBRIC`, `jud record` and `jud eval`, the flag winning over the file, both passed to `ClientBuilder::long_context` and both nothing under a replay ([Configuration](../../reference/configuration.md#the-jud-command)). The rest of 0027 stands: the switch is still the caller's, never the crate's, and a recording is keyed as without it.
- 0020's "As applied" section says every committed document was rewritten to `minor: 3`. Minor 4 ([0025](0025-a-document-names-an-image-by-path.md), [0026](0026-a-box-is-graded-by-intersection-over-union.md)) shipped in the same release, 0.14.0: the writer writes `minor: 4`, the documents that use nothing minor 4 added stay at `minor: 3`, and the twelve-month clock for a minor 3 document runs from 0.14.0.

## Writing a record

One record per decision, as `NNNN-short-title.md` with the number after the highest in the table, skipping one taken outside this repository (0015 is). The frontmatter is the one every page carries (`title`, `description`, `status`, `last_reviewed`, `tags`) plus `date`, `decision-makers`, `consulted` and `informed`; `status` is `proposed`, `accepted`, `deprecated` or `superseded by NNNN`. The body follows 0003: context and problem statement, decision drivers, the options considered, the outcome with its consequences and how it is confirmed in code or tests, the pros and cons of each option, and a "More information" section naming the pages and code that implement it. State what was rejected and why, and what the choice costs, not only what was chosen.

Add the record to the table above and to the nav in `mkdocs.yml` (under `Project` → `Decisions`), then run `mise run docs:llms`. A record is not edited after acceptance: a fact that moved is a status note here, and a changed decision is a new record that supersedes the old one.
