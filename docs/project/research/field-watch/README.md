---
title: Field watch
description: The weekly field watch's reports and run prompts, one of each per run and named by the run's date, and how a run finds the one before it.
status: current
last_reviewed: 2026-11-21
tags: [research, field-watch]
---

# Field watch

Once a week an analyst reads what changed around the crate: TypeSafe's service and SDKs, the servers that speak `/v1/systemone`, the gateways and other vendors' decision APIs, the Rust crates in the field, and the papers. Each run leaves two files here.

| Directory | Holds | Written |
|---|---|---|
| `reports/` | What changed in the window, its impact on `judgment`, the tasks and the questions for the maintainer | every run |
| `prompts/` | The run prompt a coding agent executes to carry the tasks out | only when the run has at least one task |

## Naming

Every file is named `<YYYY-MM-DD>.md`, the run's date in UTC: `reports/2026-11-21.md`, with `prompts/2026-11-21.md` beside it when there is one. One run, one date; a second run on the same day replaces the first in its pull request rather than adding a suffix.

A run's window starts at the date of the newest file in `reports/` and ends on its own date, so the names are also the record of which weeks were read. A report is not edited after it merges: a correction goes in the next report, which says what it corrects.

## Where the work goes

A run prompt's tasks become pull requests named `claude/fw-<YYYYMMDD>-<n>`; what each became (merged, skipped, still open) is read back by the next run. The longer studies a run leads to live beside this directory, under `docs/project/research/`.
