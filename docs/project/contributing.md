---
title: Contributing
description: How the repository is worked on; the quality gates and the hooks, tracking work in beads, Conventional Commits, how the documentation is organised (Diátaxis) and checked, what is generated, how the container image is built locally, how a cloud session is set up, and how a decision is recorded.
status: current
last_reviewed: 2026-10-16
tags: [judgment, contributing, tooling, documentation, diataxis]
---

# Contributing

**You will** have a checkout that passes every gate the CI runs, and know where a change goes. **Prerequisites:** [mise](https://mise.jdx.dev), which installs the pinned toolchain and tools.

```sh
mise install          # the toolchain, cog, prek, bd and the rest
mise run setup        # the git hooks
mise run check        # every quality gate, in CI order
mise tasks            # the rest
```

## The gates

`mise run check` runs, in CI's order: `cargo fmt --check`, `cargo clippy --all-targets --all-features` with warnings denied, a build and check of every target without the `http` feature (`check:minimal`), `cargo test --all-features` (unit, wiremock, example, doc and documentation tests, no network), `cargo doc` with broken links as errors, and the documentation checks below. The pre-commit hooks (`prek`, configured in `.pre-commit-config.yaml`) run the fast file-level checks and verify the commit message; the pre-push hook runs the tests.

Four more gates guard what a user of the crate depends on. Each is a CI job and a `mise run` task of the same name, and none is in `mise run check`, because the first two read the network (the baseline release, the advisory database):

| Task | Checks | A failure means |
|---|---|---|
| `semver` | The public API against the latest crates.io release, every feature (`cargo semver-checks`) | A change needs a major release, which 0.x spells as a minor one. The version in `Cargo.toml` says what is allowed: under a patch bump only additions pass. An addition never fails it |
| `deny` | Advisories, licences, duplicate versions and sources over the whole graph (`deny.toml`, `cargo deny`) | A dependency has a known vulnerability or is yanked, a direct dependency is unmaintained, a dependency brings a licence the list does not name, appears twice (two `opentelemetry` versions fail, the rest warn), or comes from outside crates.io |
| `msrv` | `cargo +1.91 check --all-features --locked` | The code or a locked dependency needs a newer Rust than `rust-version` promises |
| `hack` | Each feature on its own, and every combination of them with `http` and `pacer` as one group (`cargo hack check --each-feature`, `--feature-powerset --group-features http,pacer`) | A feature only builds when another is on, or in some combination |

The tools come from crates.io at the versions `mise.toml` pins (`mise install`), and `ci.yml` installs the same versions, cached by version, so the workflow still uses no third-party action. A licence goes on the allow list in `deny.toml` with a comment saying which crate needs it.

`tests/properties.rs` holds the crate's invariants as properties over generated inputs ([proptest](https://docs.rs/proptest)): a `Probability` and a `Confidence` hold exactly `[0, 1]`, the canonical fingerprint ignores key order and moves with any leaf, `Response::verify` accepts what fits and names each way a response does not, and `Questions` holds its limits at their edges. It runs 256 cases per property with the rest of `cargo test` and needs no feature; `PROPTEST_CASES=10000 cargo test --test properties --release` runs more, as worth doing after touching what a property covers. A failure is shrunk to a minimal input and saved to `proptest-regressions/properties.txt`, which is replayed first on every later run: commit that file with the fix. A property that filters its inputs with `prop_assume!` aborts once a large run rejects too many, so generate the input the property needs instead.

`tests/snapshots.rs` holds what the crate sends and reports as [insta](https://insta.rs) snapshots under `tests/snapshots/`: the request body each example rubric lowers to, and the report `jud eval --json --replay` gives for each recorded example. A change to either is a contract change, so it appears as a diff to review: `cargo insta review` accepts it, `mise run snapshots` checks that none is pending, and CI sets `INSTA_UPDATE=no`, so a changed or new snapshot fails there and is never written. Keys are compared sorted, because a parsed `Value` follows serde_json's `preserve_order`, which the `mcp` feature turns on; the order a model sees is asserted in `tests/jud_loop.rs` and `tests/jud_mcp.rs`, where it is the point.

`benches/verify.rs` is the crate's benchmark ([divan](https://docs.rs/divan)): `mise run bench` runs it and [Performance](../reference/performance.md) holds the numbers and the machine. CI only builds it (`cargo bench --no-run`; `mise run bench:build`). Keep divan's default features off: they turn on clap's `wrap_help` through feature unification and the `jud` binary's help changes wrap in every test build.

Mutation testing ([cargo-mutants](https://mutants.rs), `.cargo/mutants.toml`) asks whether the tests notice a changed line. `.github/workflows/mutants.yml` mutates the diff of a pull request that touches `src/answer.rs`, `src/http.rs` or `src/eval/` (`mise run mutants` does the same locally against `origin/main`) and reports the mutants the tests let through without blocking the merge, and a weekly run over the whole crate files one issue listing the survivors. A survivor is a test to strengthen or a line that does nothing; `#[cfg(kani)]` proofs are excluded because no `cargo test` can see them. A run copies the tree and builds it per mutant, so it needs disk (several GB) and minutes per hundred mutants.

Tests never call a real API: wiremock for the client, `Fake` and recordings for everything above it. The exceptions are the ignored live tests in `tests/live.rs` (`mise run live:typesafe`, `live:ollama`, `live:laya`, `live:clef`, `live:pplx`), whose results are recorded under [verification](verification/method.md), and `tests/openapi_drift.rs`, which refreshes the vendored OpenAPI document.

## Tracking work

Issues live in [beads](https://github.com/gastownhall/beads): `bd ready` lists what is open, `bd create` files one, `bd update <id> --claim` takes it. Create or claim an issue before code. The documentation audit of 2026-10-07 filed `judgment-rxl` (whether `jud` should read `TYPESAFE_MODEL` as the examples do).

The beads travel between machines through git, as the committed export `.beads/issues.jsonl`, and the hooks keep it in step on both sides. After every write bd rewrites the file and stages it (`export.auto` in `.beads/config.yaml`), and its pre-commit hook flushes it, so a commit carries the issues it touched: commit the staged file with the work, or alone as `chore(beads): ...`, and push it before a session ends. Before a commit, and after a merge or a checkout, the prek hook imports the file (`scripts/bd-sync.sh import`): a row older than the database's is skipped and nothing is deleted, so pulling never regresses local work, and a merge resolved by hand is imported before beads' flush exports the database over it. Two branches that both add or rewrite lines merge as the union of both (`.gitattributes`), the import takes the newer row of any duplicate, and the next write exports the file deduplicated. The file is never edited by hand; `mise run bd:sync -- status` says whether it is current, `mise run bd:import` and `mise run bd:export` do by hand what the hooks do. The Dolt remote (`refs/dolt/data`, `bd sync`, `bd dolt push`) is not used: the export is the one channel, reviewable in a pull request, and it works from a cloud container as well as a laptop. Switching to the Dolt remote would be a decision record: it has to be seeded from one database and every other one cloned from it (`bd bootstrap`), or their histories diverge.

## Commits and releases

Commits are [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/); `cog verify` runs on every message and CI checks a pull request's commits. The `CHANGELOG` is [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) under [Semantic Versioning](https://semver.org/spec/v2.0.0.html): a change to what the crate sends or accepts on the wire, to the `.jud` format, or to what the `jud` command's exit statuses, JSON keys, file writes or configuration mean is listed as breaking, and an addition to any of them is listed under Added ([Stability](../reference/stability.md)). A release is `cog bump --auto` on `main` and never a hand-edited version ([Releasing](releasing.md)).

## The documentation

The pages under `docs/` follow [Diátaxis](https://diataxis.fr): four kinds of page, never mixed on one.

| Kind | Where | Voice |
|---|---|---|
| Tutorials | `docs/start/` | imperative, second person; opens with what you will do and the prerequisites, closes with next steps |
| How-to guides | `docs/guides/` | the same, one task per page |
| Reference | `docs/reference/` | declarative, no narrative; facts, tables, generated where possible |
| Explanation | `docs/concepts/` | the why; links to the reference for every term |

`docs/project/` holds what is about the repository rather than the crate: this page, the internals, releasing, the verification records, the research notes and the decision records. Each fact has one page; every other page links there. Terms are defined once, in the [glossary](../reference/glossary.md). A standard is linked at its first mention on a page, and only where the code relies on it.

Every page carries frontmatter (`title`, `description`, `status`, `last_reviewed`, `tags`); `scripts/check-frontmatter.sh docs` checks it. Every page is in the `mkdocs.yml` nav, which is also what Backstage TechDocs builds (`catalog-info.yaml`) and what `scripts/gen-llms-txt.sh` reads to write `docs/llms.txt` and `docs/llms-full.txt`. Links between pages are relative; a link to another repository is an absolute URL on its default branch. Nothing names the application the crate was extracted from.

Generated, never edited by hand:

| File | Generated by | Checked by |
|---|---|---|
| `docs/reference/cli.md` | `scripts/gen-cli-reference.sh`, from the binary's `--help` | the same script with `--check`, in `mise run docs:check` |
| `plugins/jud/skills/jud/references/format.md` | `scripts/gen-plugin-format.sh`, from `docs/reference/jud-format.md` | the same, `--check` |
| `docs/llms.txt`, `docs/llms-full.txt` | `scripts/gen-llms-txt.sh` | the same, `--check` |
| `docs/demo.cast` | `scripts/record_demo.sh` (`mise run demo`), from the example recordings | a pre-bump hook re-records it |
| `examples/jud/images/page-*.png`, `examples/jud/stamps-cases.jud` | `scripts/gen-document-pages.py`, from one seed | nothing: the committed pages are the ones `examples/recordings/stamps/` was recorded on, and a redrawn page is a request nobody recorded (`tests/jud.rs` fails) |

Code on the pages is held to the code: a Rust block in a tutorial or the README is an `examples/` file between `README:BEGIN` and `README:END` markers, compared verbatim by `tests/docs_examples.rs`, and a Rust block in a guide is compiled as a documentation test (`src/lib.rs` includes the page under `#[cfg(doctest)]`); a whole `.jud` document or configuration file on a page is a file under `examples/`, named in an HTML comment before the fence (`file: PATH` between `<!--` and `-->`) and compared verbatim (a `.jud` one is also read by `jud check`, `mise run jud:check`); a command transcript is held only when it is marked with an HTML comment (`transcript: LABEL` between `<!--` and `-->`), which the same test reproduces from the binary, offline; a step that needs a key or a server stays plain text. `mise run docs:check` runs the frontmatter, generated-file and link checks (`lychee --offline`); `mise run docs:links` also checks the external links, which a weekly workflow does in CI.

## Decision records

A record under `docs/project/decisions/` says why the API or the format is shaped as it is. Records are numbered in one sequence and a number is never reused or renumbered; the [index](decisions/README.md) says which numbers are taken elsewhere. A record is not edited after it is accepted; a later record supersedes it. A new record goes in the nav and the index.

## The container image

`ghcr.io/chussenot/jud` is built by the release workflow from the release tarballs, never from source ([The container image](../reference/container-image.md)). For a change to the `Dockerfile`:

```sh
mise run image                                   # this checkout's own cargo build, tagged ghcr.io/chussenot/jud:dev
scripts/image-context.sh --release v0.10.4       # a release's tarballs, verified against its SHA256SUMS
scripts/image-context.sh --dist dist/            # a directory of those tarballs, which is what the workflow does
docker buildx build --load -t ghcr.io/chussenot/jud:local .
```

The context holds binaries and nothing else (`.dockerignore` lets only `Dockerfile`, `bin/` and `image/` through), and `scripts/image-context.sh` fills it. A release image is only ever pushed by the workflow.

## The wire

Before touching questions, answers, the client or the backends, read the live TypeSafe documentation (https://docs.typesafe.ai/llms.txt). The OpenAPI document is vendored at `tests/fixtures/typesafe-openapi.json`, checked by `tests/contract.rs`, and refreshed only through `tests/openapi_drift.rs`. A change to what the crate sends or accepts is a contract change: the test, the `CHANGELOG` and the README's guarantees move together, and the recordings under `examples/recordings/` must still replay.

## Cloud sessions

A Claude Code cloud session starts from a fresh container, and its sandbox cannot list versions through the backends `mise.toml` names (api.github.com answers only for this repository). `.claude/hooks/session-start.sh`, registered in `.claude/settings.json`, makes the container the checkout above describes before the session's first command, and only there (`CLAUDE_CODE_REMOTE`): it installs mise from its release tarball, sets `MISE_ENV=cloud` so `mise.cloud.toml` supplies the same tools at the same versions from pinned release URLs (rust stays rustup's, from `rust-toolchain.toml`; Ollama is skipped), runs `mise install` and `mise run setup`, bootstraps the beads database from `.beads/issues.jsonl` and imports the file the way the post-merge hook does, installs the two Claude Code plugins (below), installs `beads-mcp` (the MCP server for beads, a separate Python package) with `uv tool install` at the version `bd` is pinned to, and warms the build of every test target through mbx. The container is snapshotted after the hook, so later sessions pay only for what changed. `JUDGMENT_SESSION_SKIP_BUILD=1` skips the warm build. A version bump in `mise.toml` is a bump in `mise.cloud.toml` too; `beads-mcp` follows it, since the hook reads the version from `bd --version`. The hook installs the server and does not register it with Claude Code: `claude mcp add beads -- beads-mcp` does, at the scope you choose, and a missing `uv` or a failed install is logged and skipped.

The plugins are the three `.claude/settings.json` enables: `jud` from this checkout, the marketplace being the checkout itself as a directory source, so the plugin loads in place and an edit under `plugins/jud/` is live at the next session start or `/reload-plugins`; `typesafe` from TypeSafe's marketplace (`typesafe-ai/skills`), whose `typesafe-ai` skill `CLAUDE.md` asks for before the wire is touched; and [`ponytail`](https://github.com/DietrichGebert/ponytail), the less-code discipline `CLAUDE.md` holds every change to, whose hooks run at session start, at a subagent's start and on every prompt. Ponytail is pinned to a tag (`ref: v5.2.0` in the settings file, `PONYTAIL_REF` in the script, moved together), since a plugin that shapes how code is written is a release to review, not one to receive unseen; the other two follow their marketplaces. A laptop's Claude Code installs all three from the settings file once the workspace is trusted; a cloud session never does ([Plugins](https://code.claude.com/docs/en/plugins/loading)), so `scripts/claude-plugins.sh install` runs them in through the `claude` command at user scope, idempotently, and the snapshot keeps them. The same script is `mise run plugins:install` on a laptop, and `mise run plugins:update` refreshes the marketplaces and moves `jud` and `typesafe` to their latest versions and `ponytail` to its pin; a session that is already open sees an update after `/reload-plugins`. A shortcut taken on purpose is marked `ponytail:` in a comment, this repository's word for Ponytail's default `shortcut:`, and `/ponytail-debt ponytail:` lists them.

## Secrets

`.env` is gitignored and loaded by mise; a key never goes anywhere else, and the hooks refuse to commit one.
