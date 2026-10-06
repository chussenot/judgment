# judgment

Typed, calibrated judgments from TypeSafe System One models (Jev) and
compatible backends (Laya), as a Rust crate: typed questions and answers, the
HTTP client with the official SDKs' retries, record and replay, evaluation.
The README says why; `docs/` says how; the rustdoc is the reference.

## Start here

- `mise install` once, then `mise run setup` (git hooks) and `mise run check`
  (every quality gate, CI order). `mise tasks` lists the rest.
- Track work in beads (section below): `bd ready` for what is open, create or
  claim an issue before code. Git's hooks live in `.beads/hooks` (`bd init`
  set `core.hooksPath`); `scripts/setup-hooks.sh` puts the prek block before
  the beads section in each, so `prek install` is never run directly.
- `docs/index.md` maps the documentation; `docs/decisions/` holds the records
  that govern the API; `CHANGELOG.md` is Keep a Changelog, and 0.x means a
  minor release may break.

## Rules that are easy to get wrong

- Load the `typesafe:typesafe-ai` skill before touching questions, answers,
  the client or the backends. The live docs are the contract; start at
  https://docs.typesafe.ai/llms.txt. The OpenAPI document
  (https://api.typesafe.ai/openapi.json) is vendored at
  `tests/fixtures/typesafe-openapi.json`, checked by `tests/contract.rs`,
  exposed as `judgment::contract::OPENAPI_DOCUMENT` under the `openapi`
  feature, and refreshed only through `tests/openapi_drift.rs`; never
  hand-edit it.
- The crate is generic. Nothing about any one application (alerts, an
  incident tool, a catalog, an application's metrics) goes in. Token usage
  and failed attempts are reported through `judgment::Observer`; the crate
  emits `tracing` spans and no metrics of its own. The `http` feature gates
  the client; questions, answers, the other backends, recordings and metrics
  build without it (`mise run check:minimal`).
- Every backend holds a response against the questions it was sent
  (`Response::verify`): an answer for every question, of its primitive, a
  Choice naming only offered options, a Score whose legend parses to the
  levels sent. An off-list option is an error, never a default. Deterministic
  logic stays in the caller; the model answers narrow, atomic questions.
- Errors are grouped by what fixes them (`Error`). A 2xx the client cannot
  use is `decode` or `unfit` to the observer and is never retried; every HTTP
  call goes through `http::send_with_retries`.
- Tests never call a real API: wiremock for the client, `Fake` and recordings
  for everything else. The exceptions are `tests/live.rs`, all `#[ignore]`,
  run by hand against `JUDGMENT_LIVE_BASE_URL` (`mise run live:typesafe` with
  `TYPESAFE_API_KEY` in `.env`, `mise run live:clef` with `CLOUDFLARE_ACCOUNT_ID`
  and `CLOUDFLARE_API_TOKEN` in `.env` through `tools/systemone/serve.py`,
  `mise run live:laya` and `mise run live:ollama` against a local server;
  `docs/verification/` records what real servers did), and
  `tests/openapi_drift.rs`, ignored, network only, no key.
- A change to what the crate sends or accepts on the wire is a contract
  change: `tests/contract.rs`, the CHANGELOG and the README's guarantees move
  together, and the recordings under `examples/*/recordings/` must still
  replay (`cargo test` runs them).
- The `.jud` format (`docs/jud.md`, `src/jud/`, feature `jud`) is a
  specification other tools implement: a change to what a document may hold
  or how it is read moves `docs/jud.md`, `schemas/jud/`, `tests/jud.rs` and
  the documents under `examples/jud/` together. A purely additive change
  takes the next minor version (`jud: 1.1`, decision 0016), a change of
  meaning the next major; a document must declare the version of the
  features it uses, and a writer declares the lowest that reads it, so a
  `jud: 1` rubric keeps its fingerprint and is still written as `jud: 1`. The
  questions in a rubric are the wire's shape, never a translation of it, and
  fingerprints are RFC 8785 canonical JSON (`src/eval/canonical.rs`), checked
  against the known vector on the page. `Questions` and a Choice's options
  keep insertion order: it is the order the model sees.
- Documentation: every page under `docs/` carries frontmatter (`title`,
  `description`, `status`, `last_reviewed`, `tags`); the README does not
  (crates.io renders it). `docs/llms.txt` and `docs/llms-full.txt` are
  generated from `mkdocs.yml`, `docs/llms-intro.txt` and that frontmatter:
  never edit them; run `mise run docs:llms` after adding a page (add it to
  the nav too) or changing a title or description. Paths in sources and pages
  are relative to the crate. A link to another repository's documentation is
  an absolute URL on its default branch. Nothing about the application the
  crate was extracted from is named anywhere in the repository.
- Decision records are numbered in one sequence and a number is never reused
  or renumbered: 0003 kept its number when it moved here, the numbers up to
  0012 and 0015 are taken outside this repository and left unused, and a
  new record takes the number after the highest in use in either place
  (`docs/decisions/README.md` says which are skipped).
- Commits are Conventional Commits (`cog verify` runs on every commit
  message through prek; CI checks a pull request's commits). A release is
  `cog bump --auto` on `main` and never a hand-edited version: `cog.toml`
  and `scripts/release-bump.sh` say what a bump touches, and
  `docs/releasing.md` is the runbook. `.github/workflows/release.yml`
  publishes to crates.io from a pushed `v*` tag, so the Bash guard denies
  `cargo publish` (the dry run is allowed) and pushing a tag; both are the
  release owner's decision.
- Secrets live in `.env` (gitignored, loaded by mise). Never commit one.

## Harness

- Subagents in `.claude/agents/`: `contract-reviewer` (the wire against the
  live TypeSafe documentation and the vendored OpenAPI document),
  `docs-writer` and `docs-auditor` (this documentation set, the why as well
  as the how), `test-writer` (tests in this crate's wiremock, Fake and
  recording style), `refactor-scout` (dead code and duplication, ranked),
  `pr-shepherd` (open the PR, read its CI checks). Reviewers report, writers
  edit; run the reviewers on a diff before opening a pull request.
- Hooks in `.claude/hooks/`: Rust files are formatted after every edit;
  `docs/llms.txt` is regenerated after a page, the README, `llms-intro.txt`
  or `mkdocs.yml` is written; a Bash guard denies pushes to `main`,
  committing `.env`, `cargo publish` and `bd edit` (it opens an editor and
  hangs). `bd prime` runs at session start.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:1105d646 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->
