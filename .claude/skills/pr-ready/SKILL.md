---
name: pr-ready
description: Take this repository's current branch from "the change is done" to an open draft pull request, with the reviewers the diff calls for having reported and the gate passed first. Use when a change is committed and ready for review, or when asked to open a pull request here.
---

# From a finished change to a pull request

CLAUDE.md says to run the reviewers on a diff before a pull request; this
is the order that gets there in one round. It ends by handing the branch
to the `pr-shepherd` agent, which owns the pull request's shape and reads
its checks. Nothing here merges, approves, pushes to `main` or rewrites
history.

## 1. The reviewers this diff needs

List the files: `git diff --name-only origin/main...HEAD`. Then, in
parallel, one agent per row that applies:

| The diff touches | Run | Why |
|---|---|---|
| `src/{client,question,answer,backend,http,error}.rs`, `tests/contract.rs`, `tests/live.rs`, `tests/fixtures/typesafe-openapi.json` | `contract-reviewer` | the wire against the live TypeSafe docs and the vendored OpenAPI document |
| `README.md`, `docs/**`, a rustdoc module page | `docs-auditor` | claims that no longer hold, generated files that drifted |
| `src/**` with no test file in the diff | `test-writer`, first | a check without its negative case is not done |
| `src/jud/**`, `schemas/jud/**` | `contract-reviewer`, asked about the format | a document's shape is a contract too (decisions 0018, 0020) |

Reviewers report and do not edit. Verify each finding in the code before
acting on it: fix the ones that hold and commit; a finding that does not
hold gets one line in the pull request's **Decisions to review**. A row
skipped because its files did not change needs no mention.

A wire or `.jud` format change also moves `CHANGELOG.md` (Unreleased) and
the README's guarantees; check both are in the diff.

## 2. The gate, before the push

`mise run check` runs CI's local gates in CI's order, but not all of CI:
the jobs that need the network or a second toolchain stay outside it. When
the diff touches `Cargo.toml` or `Cargo.lock`, run them too, because that is
the change they exist for:

```sh
mise run deny msrv hack semver
```

For a small change confined to one area, this subset catches what CI would,
and the pre-push hook runs the tests:

```sh
cargo fmt --all --check
RUSTFLAGS=-D warnings cargo clippy --all-targets --all-features
mise run docs:check
```

A generated file is regenerated with its script, never edited:
`scripts/gen-llms-txt.sh`, `scripts/gen-cli-reference.sh`,
`scripts/gen-plugin-format.sh`, `scripts/gen-plugin-rust.sh`.

## 3. Push, then hand off

- Conventional Commits, one concern per commit (`cog verify` runs on each).
- The pre-push hook runs `cargo test --all-features` for minutes: push in
  the background, with its whole output in the scratchpad. The first error
  line explains the failure, and a tail can hide it.
- `.beads/issues.jsonl` travels with the work: the hooks staged it; a
  commit that leaves it modified has not finished.
- Hand the pushed branch to `pr-shepherd`, which opens a **draft** with the
  body it defines (Summary, Decisions to review, Test plan). Give it the
  reviewers' findings and what the gate ran, so the Test plan is true.
- Subscribe to the pull request's activity and end the turn: checks and
  reviews wake the session. The `steward` skill covers what comes next.
