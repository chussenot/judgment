# judgment

Typed, calibrated judgments from TypeSafe System One models (Jev) and
compatible backends (Laya), as a Rust crate: typed questions and answers, the
HTTP client with the official SDKs' retries, record and replay, evaluation.
The README says why; `docs/` says how; the rustdoc is the reference.

## Start here

- `mise install` once, then `mise run setup` (git hooks) and `mise run check`
  (every quality gate, CI order). `mise tasks` lists the rest.
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
  `TYPESAFE_API_KEY` in `.env`, `mise run live:laya` against a local server;
  `docs/verification/` records what real servers did), and
  `tests/openapi_drift.rs`, ignored, network only, no key.
- A change to what the crate sends or accepts on the wire is a contract
  change: `tests/contract.rs`, the CHANGELOG and the README's guarantees move
  together, and the recordings under `examples/*/recordings/` must still
  replay (`cargo test` runs them).
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
  0012 are left unused, and a new record takes the number after the highest
  in `docs/decisions/README.md`.
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
  committing `.env`, and `cargo publish`.
