---
title: Documentation audit before the Diátaxis reorganisation
description: Working file for the documentation rebuild. Inventory of every page, every claim checked against the code, the duplication map with each concept's canonical home, the old-to-new mapping, and the choices made. Deleted in the last commit of the pull request after its content moves into the description.
status: experiment
last_reviewed: 2026-10-07
tags: [judgment, docs, audit]
llms: false
---

# Documentation audit before the Diátaxis reorganisation

Working file. It exists on the branch only; the last commit deletes it after
its content moves into the pull request description.

Code is the source of truth. Where a page and the code disagree the page is
listed under *Claims* with what the code does; where the code looks wrong a
beads issue is named instead of a code change.

## 1. Inventory

Every page that documents the crate, the `jud` command or the `.jud` format,
with its Diátaxis type as written today. "Mixed" means a page holds two or
more types and is split in the new tree.

| Page | Lines | Type today | Readers |
|---|---|---|---|
| `README.md` | 405 | mixed: pitch, tutorial, reference tables, comparison, index | all |
| `docs/index.md` | 66 | index (a table of pages) | all |
| `docs/design.md` | 94 | explanation: typed handles, the response check, the retry loop | Rust dev |
| `docs/tour.md` | 55 | reference-ish: the modules, one paragraph each | Rust dev |
| `docs/implementation.md` | 213 | explanation of the rules the code applies, with tables of the cases | contributor |
| `docs/patterns.md` | 32 | how-to: TypeSafe's four patterns onto the examples | Rust dev |
| `docs/testing.md` | 67 | mixed: how-to (Fake, Recorder, Replay) + reference table of the properties tested | Rust dev, contributor |
| `docs/open-weights.md` | 126 | mixed: how-to (Ollama, Clef) + a comparison table (reference) + a verification record | CLI user, Rust dev |
| `docs/jud.md` | 406 | reference: the format specification | author, Rust dev |
| `docs/jud/rubric.md` | 188 | mixed: explanation (what a rubric is) + how-to (writing, tuning) | author |
| `docs/jud/cases.md` | 131 | mixed: explanation + how-to (labelling) | author, evaluator |
| `docs/jud/recording.md` | 102 | mixed: explanation + how-to (record, replay) | evaluator |
| `docs/cli.md` | 202 | mixed: tutorial, every install route, configuration reference, replay how-to, exit status, subcommand table, internals | CLI user |
| `docs/container.md` | 138 | mixed: how-to (mounts, keys, host Ollama, CI) + reference (what is inside, tags) + a verification note | CLI user |
| `docs/skill.md` | 103 | mixed: explanation of the plugin + how-to (install, the workflow) + an evaluation record | author |
| `docs/stability.md` | 54 | reference: what is stable, with a table | all |
| `docs/releasing.md` | 77 | how-to for the release owner | contributor |
| `docs/verification/method.md` | 62 | explanation of the verification method | contributor |
| `docs/verification/hosted-typesafe.md` | 91 | record of what the hosted API did | contributor |
| `docs/verification/laya-typed-decisions.md` | 195 | record of what Laya did | contributor |
| `docs/research/system-one-client-libraries.md` | 113 | research record, 2026-09-25 | contributor |
| `docs/research/client-comparison.md` | 140 | research record, 2026-10-04 | contributor, evaluator |
| `docs/research/compatible-servers-and-models.md` | 84 | research record, 2026-10-06 | CLI user, evaluator |
| `docs/research/system-one-on-google-cloud.md` | 124 | research record, 2026-10-06 | Rust dev |
| `docs/decisions/*.md` (0003, 0013, 0014, 0016–0020) + `README.md` | 755 | decision records, immutable | contributor |
| `CLAUDE.md` | 164 | agent instructions; contributor rules | contributor, agents |
| `plugins/jud/README.md` | 24 | plugin index | author |
| `plugins/jud/skills/jud/SKILL.md` | 263 | guide for an agent writing documents | agents |
| `plugins/jud/skills/jud/references/format.md` | 368 | a condensed copy of `docs/jud.md` | agents |
| `plugins/jud/commands/{rubric,cases,check}.md` | 109 | command prompts | agents |

Generated, never edited: `docs/llms.txt`, `docs/llms-full.txt` (from
`mkdocs.yml` and the frontmatter by `scripts/gen-llms-txt.sh`),
`docs/demo.cast` (by `scripts/record_demo.sh`).

### Every `docs/` path referenced from outside `docs/`

Each must be updated when the page moves. `grep -rn "docs/"` over `src`,
`tests`, `examples`, `scripts`, `.github`, `mkdocs.yml`, `catalog-info.yaml`,
`cog.toml`, `Cargo.toml`, `mise.toml`, `plugins`, `.claude-plugin`,
`README.md`, `CLAUDE.md`:

| Path | Referenced from |
|---|---|
| `docs/cli.md` | `Cargo.toml` (exclude comment), `README.md`, `mise.toml`, `scripts/homebrew-formula.sh`, `scripts/record_demo.sh`, `src/bin/jud/main.rs` (the `completion` help text) |
| `docs/container.md` | `README.md` |
| `docs/decisions/README.md`, `docs/decisions/` | `CLAUDE.md`, `README.md` |
| `docs/demo.cast` | `mise.toml`, `scripts/record_demo.sh` |
| `docs/design.md` | `README.md`, `src/answer.rs`, `src/http.rs`, `src/question.rs` |
| `docs/implementation.md` | `README.md` |
| `docs/index.md` | `README.md` |
| `docs/jud.md` | `CLAUDE.md`, `Cargo.toml`, `README.md`, `examples/jud/triage.jud`, `examples/jud_calibration.rs`, `examples/jud_quickstart.rs`, `plugins/jud/README.md`, `plugins/jud/skills/jud/SKILL.md`, `plugins/jud/skills/jud/references/format.md`, `src/bin/jud/tools.rs`, `src/eval/canonical.rs`, `src/eval/mod.rs`, `src/jud/{cases,mod,rubric}.rs`, `src/lib.rs`, `tests/contract.rs`, `tests/jud_cli.rs` |
| `docs/jud/recording.md` | `README.md`, `plugins/jud/skills/jud/SKILL.md` |
| `docs/jud/rubric.md`, `docs/jud/cases.md` | `plugins/jud/skills/jud/SKILL.md` |
| `docs/llms.txt`, `docs/llms-full.txt`, `docs/llms-intro.txt` | `CLAUDE.md`, `README.md`, `mise.toml`, `mkdocs.yml`, `scripts/gen-llms-txt.sh` |
| `docs/open-weights.md` | `README.md`, `examples/jud/config-tev1.yaml`, `mise.toml` |
| `docs/patterns.md` | `README.md`, `src/lib.rs` |
| `docs/releasing.md` | `.github/workflows/release.yml`, `CLAUDE.md`, `Cargo.toml`, `README.md`, `cog.toml`, `mise.toml` |
| `docs/research/client-comparison.md`, `docs/research/compatible-servers-and-models.md` | `README.md` |
| `docs/skill.md` | `CLAUDE.md`, `Cargo.toml`, `README.md`, `plugins/jud/.claude-plugin/plugin.json` (homepage), `plugins/jud/README.md` |
| `docs/stability.md`, `docs/tour.md` | `README.md` |
| `docs/testing.md` | `Cargo.toml`, `README.md`, `mise.toml`, `src/answer.rs`, `src/backend.rs`, `src/eval/{metrics,mod,tuning}.rs`, `src/http.rs` |
| `docs/verification/`, `docs/verification/method.md` | `CLAUDE.md`, `Cargo.toml`, `README.md`, `src/answer.rs` |
| `docs/verification/hosted-typesafe.md` | `examples/confidence_routing.rs`, `examples/fan_out.rs`, `src/answer.rs`, `src/client.rs`, `src/question.rs`, `tests/live.rs` |
| `docs/verification/laya-typed-decisions.md` | `README.md`, `examples/typed_decisions.rs`, `mise.toml`, `tests/live.rs` |

`CLAUDE.md` also names `docs/core-concepts/sync-concepts.md`: that is a path
in the beads repository's URL, not a page here.

## 2. Claims

Sources of truth: `src/bin/jud/*.rs` (the clap tree, captured with
`jud <cmd> --help` from a `--features cli` build of 0.10.4), `src/bin/jud/config.rs`,
`Cargo.toml` (`[features]`, `version`, `rust-version`), `src/client.rs`
(constants), `src/http.rs` (`RetryPolicy`), `src/error.rs`, `src/question.rs`
(limits), `src/jud/*`, `schemas/jud/*`, `src/eval/*`, `src/backend.rs`,
`tests/fixtures/typesafe-openapi.json`, `.github/workflows/*.yml`,
`Dockerfile`, `scripts/*`.

Status: **ok** holds; **stale** held for an earlier release; **wrong** never
held; **record** is a dated statement about a server or a measurement that
the code cannot confirm or refute and is kept as a record.

| Claim | Where | Source of truth | Status | Action |
|---|---|---|---|---|
| The `jud` module reads the format "versions 1 and 1.1" | `docs/tour.md` | `src/jud/mod.rs`: `API_VERSION = "jud/v1.3"`, the reader refuses any other by name | stale (1.3 since 0.8.0) | rewritten in `concepts/how-judgment-works.md` and `reference/crate.md` |
| A recording is keyed by "a 64-bit hash of the canonical JSON", which `eval::fingerprint` exposes | `docs/testing.md` | `src/backend.rs`: `Replay` looks up the SHA-256 request fingerprint first, then the FNV `request_hash`; `eval::fingerprint` is the SHA-256 | wrong on the function, stale on the key | `guides/record-replay-and-test.md` says both keys, in order |
| `jud check` prints `id inbox-triage` | `docs/skill.md` | the binary prints `name inbox-triage` (captured above) | stale (renamed with the 1.3 envelope) | transcript replaced by the binary's output, held by a test |
| The `jud` command exits 1 when a document is refused | `docs/skill.md` ("exit 1 if any is refused") | `jud check --help`: "status 2 when any document is refused"; `tests/jud_cli.rs` | wrong | `reference/cli.md` from the help text |
| `judgment = "0.9"` as the pin to copy | `docs/stability.md` | `Cargo.toml` 0.10.4; the README's pin is checked by `tests/readme.rs` | stale | pin written as `"0.10"` in `start/install.md` only; `reference/stability.md` says "the minor" without a number |
| `judgment 0.10.0` in the comparison table | `README.md` | `Cargo.toml` 0.10.4; the comparison was made on 2026-10-04 against 0.10.0 | record | the table leaves the README; the research page keeps its date and version |
| `@0.9.0`, `TAG=v0.9.0` as the install examples | `docs/cli.md` | latest tag v0.10.4 | stale | `start/install.md` uses `v0.10.4` and says it is an example |
| The `jud` timeout default is 30 s | `docs/cli.md` | `src/bin/jud/config.rs`: `DEFAULT_TIMEOUT_SECS = 30` | ok | `reference/configuration.md` |
| The crate's default timeout is 10 s | `docs/implementation.md`, rustdoc | `src/client.rs`: `DEFAULT_TIMEOUT = 10 s` | ok | both defaults documented, separately, with why they differ |
| Two retries, 500 ms doubling to 5 s, jitter 0.25, server wait up to 30 s, body cap 8 MiB | `docs/design.md`, `docs/tour.md`, `docs/implementation.md` | `RetryPolicy::default()` in `src/http.rs` | ok | `concepts/how-judgment-works.md` keeps the loop and the diagram; the numbers live in `reference/crate.md` |
| Retried statuses: 408, 429, every 5xx, plus transport errors | `docs/design.md` | `src/http.rs` | ok | kept |
| 255 options at most, 2 to 10 levels, non-empty unique ids | README, `docs/tour.md`, `docs/jud.md`, plugin | `src/question.rs`: `MAX_CHOICE_OPTIONS = 255`, `MAX_SCORE_LEVELS = 10` | ok | `reference/crate.md` and `reference/jud-format.md` |
| Config precedence: environment, then `~/.config/jud/config.yaml`, then defaults; `XDG_CONFIG_HOME` honoured | `docs/cli.md`, `docs/container.md` | `src/bin/jud/config.rs::path`, `resolve` | ok | `reference/configuration.md` with the diagram |
| No environment variable selects the model for `jud` | `docs/cli.md` ("the file adds the two values the crate has no variable for") | `config.rs`: `model` from the file only; the examples read `TYPESAFE_MODEL` | ok, and easy to misread | `reference/configuration.md` names `TYPESAFE_MODEL` as the examples' variable, not the command's |
| `jud config` prints `api_key` as `environment`, `config_file` or `missing`, never the key | `docs/cli.md` | `config.rs::Resolved` | ok | kept |
| `--replay DIR` or `JUD_REPLAY` | `docs/cli.md`, README | `jud --help` | ok | kept |
| Exit status 0, 1, 2 and their meaning | `docs/cli.md`, `docs/stability.md`, `docs/container.md` | `jud --help` | ok | one home, `reference/cli.md`; `reference/stability.md` links it |
| The no-key message | `docs/open-weights.md` | the binary: ``no API key: set TYPESAFE_API_KEY, or `api_key` in …/.config/jud/config.yaml`` | ok | kept in `start/first-decision-cli.md` |
| `jud completion` for bash, elvish, fish, powershell, zsh | `docs/cli.md` | `jud completion --help` | ok | generated |
| State paths are JSON Pointers (RFC 6901) | nowhere claimed; the task asked to verify | `src/jud/mod.rs::present`: dot-separated keys, an array index is a canonical decimal "as a JSON Pointer's" (`01`, `+0` index nothing); no `/`, no `~0`/`~1` escaping, no empty segment | n/a | `reference/jud-format.md` says the syntax is not RFC 6901 and names the one rule borrowed |
| Fingerprints are `sha256:` over RFC 8785 canonical JSON of values inside `spec` | `docs/jud.md`, `docs/stability.md` | `src/eval/canonical.rs`, `src/jud/rubric.rs::fingerprint`; the vector on the page is checked by `tests/jud.rs` | ok | `reference/jud-format.md` |
| Every name in a document is a name, never a path | README, `docs/jud.md` | `src/eval/mod.rs::is_name`: ASCII alphanumeric first, then alphanumeric, `.`, `_`, `-` | ok | `reference/jud-format.md` states the grammar |
| The container runs as uid 65532, `HOME=/home/jud`, CA bundle at `/etc/ssl/certs/ca-certificates.crt`, `SSL_CERT_FILE` set, workdir `/work` | `docs/container.md` | `Dockerfile` | ok | `reference/container-image.md` |
| Release tags pushed: `X.Y.Z`, `X.Y`, `latest` | `docs/container.md` | `.github/workflows/release.yml` `image` job | ok | `reference/container-image.md` |
| Targets: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `aarch64-apple-darwin`; no Intel macOS | `docs/cli.md` | `release.yml` matrix | ok | `start/install.md` |
| `cargo binstall judgment` installs the release tarball | `docs/cli.md` | `Cargo.toml [package.metadata.binstall]`, checked in `release.yml` | ok | `start/install.md` |
| `brew install chussenot/tap/jud` | README, `docs/cli.md` | `scripts/homebrew-formula.sh`, the `homebrew` job | ok | `start/install.md` |
| `mise use -g github:chussenot/judgment` installs `jud` | `docs/cli.md` | the release assets' names match mise's github backend | ok | `start/install.md` |
| Rust 1.91 is the minimum | README | `Cargo.toml rust-version = "1.91"` | ok | `start/install.md`, `reference/crate.md` |
| Features: `http` (default), `openapi`, `jud`, `cli` and what each pulls | README, `docs/tour.md`, `docs/cli.md` | `Cargo.toml [features]` | ok | the features matrix in `reference/crate.md` |
| `Error` is `#[non_exhaustive]`, grouped by remedy, 23 variants | `docs/tour.md`, `docs/implementation.md` | `src/error.rs` | ok | `reference/crate.md` lists the groups and links the rustdoc for the variants |
| Metrics: Brier (1950), ECE over 10 bins (Naeini et al. 2015), Wilson interval at z 1.96 | `docs/jud/cases.md`, `docs/testing.md`, rustdoc | `src/eval/metrics.rs`, `ECE_BINS = 10`, `Z_95 = 1.96` | ok | `concepts/rubrics-cases-recordings.md` cites the papers once |
| The examples replay `jev-1.13.0`'s answers of 2026-10-03 | `docs/patterns.md` | `examples/recordings/*/` metadata | ok | `guides/patterns.md` |
| ADR 0020 is proposed, not decided | `docs/decisions/0020`, `docs/stability.md` | the record's status line | ok | `reference/stability.md` documents today's behaviour (`jud/v1.3` only) and links 0020 unchanged |
| The plugin's `format.md` is "condensed from `docs/jud.md`" and lists the 1.3 rules | `plugins/jud/skills/jud/references/format.md` | `src/jud/*`; no check holds the two together | ok today, unguarded | generated from the spec by `scripts/gen-plugin-format.sh`, checked in CI (section 5) |
| Ollama 0.40.0 serves 2 to 26 options, `tev1:0.8b` answers in one to two seconds | `docs/open-weights.md` | a measurement of 2026-10-06 | record | kept as a dated record in `guides/configure-a-backend.md` |
| What the hosted API, Laya and Clef did on the dates given | `docs/verification/*`, `docs/research/*` | dated records | record | moved untouched, links fixed |
| Container verified on 2026-10-07 against 0.10.4 | `docs/container.md` | record | record | kept, dated, in `reference/container-image.md` |

No claim audited pointed at a defect in the code; no beads issue is opened
for the code. One behaviour is documented as a difference rather than a
defect: the examples select a model with `TYPESAFE_MODEL` and the `jud`
command does not read that variable (`model` comes from the file or the
default). Whether the command should read it is a product question; it is
filed as a beads issue so it is not lost, and the documentation says what
the command does today.

## 3. Duplication map

Concept → every page that explains it today → the one page that owns it in
the new tree. Every other page links there.

| Concept | Today | Canonical home |
|---|---|---|
| Install routes (cargo, mise, Homebrew, release tarballs, binstall, container, completion) | README (one-liners), `docs/cli.md` (every route), `docs/container.md` (pull), `docs/skill.md` (plugin install) | `start/install.md`; the plugin's install stays in `guides/use-the-claude-code-plugin.md` |
| Backend configuration (variables, file, precedence, defaults) | `docs/cli.md`, `docs/container.md` (inside the image), `docs/open-weights.md` (Ollama file), README | `reference/configuration.md` (the facts); `guides/configure-a-backend.md` (the procedures per server) |
| Replay (`--replay`, `JUD_REPLAY`, recordings as a backend) | README, `docs/cli.md`, `docs/container.md`, `docs/testing.md`, `docs/jud/recording.md` | `guides/record-replay-and-test.md`; `reference/cli.md` for the flag |
| The `.jud` format rules | `docs/jud.md`, `docs/jud/{rubric,cases,recording}.md` (restated with examples), plugin `format.md` (condensed copy), SKILL.md (the rules writers trip on), README (envelope) | `reference/jud-format.md`; the plugin's copy is generated from it |
| What a rubric, a case, a recording is (the why) | `docs/jud/*.md` intros, README, `docs/jud.md` preamble, decision 0014 | `concepts/rubrics-cases-recordings.md` |
| Typed handles, the response check, the retry loop | README ("what it guarantees"), `docs/design.md`, `docs/tour.md`, `docs/implementation.md`, decision 0003 | `concepts/how-judgment-works.md` for the mechanisms; `project/internals.md` for the rule tables; the numbers in `reference/crate.md` |
| The module map | `docs/tour.md`, README's table, `src/lib.rs` rustdoc | `reference/crate.md` (modules → docs.rs links); the rustdoc stays the API reference |
| Exit status | `docs/cli.md`, `docs/container.md`, `docs/stability.md`, `docs/open-weights.md` | `reference/cli.md` |
| Subcommands and flags | `docs/cli.md` (a table), `docs/skill.md` (`jud check`), `docs/container.md` (examples), README | `reference/cli.md`, generated from the clap help |
| Stability promises | `docs/stability.md`, decisions 0016–0020, README, CHANGELOG preamble | `reference/stability.md` for the promise today; the decisions for why |
| The Rust client comparison | README (one table), `docs/research/client-comparison.md` (five grids), `docs/research/system-one-client-libraries.md` (the earlier survey) | `project/research/client-comparison.md`; the README links it in one line. The two research pages are not merged: each is a dated record and the later cites the earlier |
| Open-weight models (Ollama, Clef, Laya) | `docs/open-weights.md`, `docs/container.md` (host Ollama), `docs/research/compatible-servers-and-models.md`, `docs/verification/laya-typed-decisions.md` | `guides/configure-a-backend.md` for the procedures; the research and verification pages stay as records |
| The plugin | `docs/skill.md`, `plugins/jud/README.md`, `CLAUDE.md` | `guides/use-the-claude-code-plugin.md`; the plugin README becomes a pointer |
| Verification method | `docs/verification/method.md`, `docs/testing.md` (the property table), `CLAUDE.md` | `project/verification/method.md`; the property table moves to `project/internals.md` |
| Patterns | `docs/patterns.md`, README's examples list, `src/lib.rs` | `guides/patterns.md` |
| Releasing | `docs/releasing.md`, decision 0013, `CLAUDE.md` | `project/releasing.md` |

## 4. Old → new mapping

Pages that move or split. ADRs move as a folder; their content is not edited,
only the links the move breaks.

| Old | New | Why |
|---|---|---|
| `README.md` (405 lines) | `README.md` (~120 lines) | front door: pitch, demo, install one-liners, one CLI and one Rust example, links; every example still held by `tests/readme.rs` |
| `docs/index.md` | `docs/index.md` | three sentences, the demo, pick your path by reader |
| `docs/cli.md` §install | `start/install.md` | every route in one place, one page for all readers |
| `docs/cli.md` §what a run does, §replay | `start/first-decision-cli.md` | a tutorial from zero to a verdict with no account (replay), then a server |
| README quickstart | `start/first-decision-rust.md` | the same, in Rust, with `Fake` then `Client` |
| `docs/cli.md` §configuration, `docs/open-weights.md`, `docs/container.md` §backends, `docs/research/system-one-on-google-cloud.md` (the how) | `guides/configure-a-backend.md` | one procedure per server, the precedence stated once |
| `docs/jud/rubric.md` (the how) | `guides/write-a-rubric.md` | how-to only |
| `docs/jud/cases.md` (the how) | `guides/label-cases.md` | how-to only |
| `docs/testing.md`, `docs/jud/recording.md` (the how) | `guides/record-replay-and-test.md` | Fake, Recorder, Replay, `jud --replay`, in one procedure |
| `docs/jud/rubric.md` §tuning, `examples/jud_calibration.rs` | `guides/tune-thresholds.md` | the sweep, bands, `tuned_on`, how to read the metrics |
| `docs/container.md` §CI, `docs/cli.md` §replay | `guides/run-in-ci.md` | `jud check` on documents, replay in a job, the image in a pipeline |
| `docs/container.md` (the how) | `guides/run-in-a-container.md` | mounts, uid, stdin, keys, the host's Ollama: too long for `install.md`, a task of its own (added to the proposed tree) |
| `docs/skill.md` | `guides/use-the-claude-code-plugin.md` | the plugin as a how-to; the eval record stays as a dated note on the page |
| `docs/patterns.md` | `guides/patterns.md` | unchanged in substance |
| `docs/cli.md` §subcommands, §exit status | `reference/cli.md` | generated from the clap help, one block per command |
| `docs/cli.md` §configuration (the table) | `reference/configuration.md` | the variables, the file, precedence, defaults, the diagram |
| `docs/jud.md` | `reference/jud-format.md` | the normative spec; BCP 14 keywords; standards linked at first mention |
| `docs/tour.md`, README's feature and guarantee lists | `reference/crate.md` | features matrix, modules → docs.rs, error groups, limits, defaults |
| `docs/container.md` (what is inside) | `reference/container-image.md` | contents, user, paths, tags, attestation (added to the proposed tree) |
| `docs/stability.md` | `reference/stability.md` | unchanged in substance, numbers unpinned |
| new | `reference/glossary.md` | the terms every page links to |
| README's why, `docs/design.md` preamble | `concepts/system-one.md` | what a System One model answers and why that suits a decision |
| `docs/jud/*.md` (the what and why) | `concepts/rubrics-cases-recordings.md` | the three documents as one loop |
| `docs/design.md` + `docs/tour.md` | `concepts/how-judgment-works.md` | handles, the check, the retry loop, with the diagrams |
| `docs/implementation.md`, `docs/testing.md` §properties | `project/internals.md` | trimmed to what a contributor needs beyond the rustdoc |
| `CLAUDE.md` §rules (the human part) | `project/contributing.md` | the gates, the hooks, the commit convention, Diátaxis for the docs |
| `docs/releasing.md` | `project/releasing.md` | moved |
| `docs/verification/*` | `project/verification/*` | moved untouched, links fixed |
| `docs/research/*` | `project/research/*` | moved untouched, links fixed |
| `docs/decisions/*` | `project/decisions/*` | moved untouched, links fixed |

Deleted after their content moved: `docs/design.md`, `docs/tour.md`,
`docs/implementation.md`, `docs/testing.md`, `docs/patterns.md`,
`docs/open-weights.md`, `docs/jud.md`, `docs/jud/*.md`, `docs/cli.md`,
`docs/container.md`, `docs/skill.md`, `docs/stability.md`,
`docs/releasing.md`.

Deviations from the proposed tree, with why:

- `guides/run-in-a-container.md` and `reference/container-image.md` are
  added. The container page is 138 lines of two types; folding its how-to
  into `install.md` would make the install page about Docker, and its
  reference part (what is inside, tags, the user) has no other home.
- `project/contributing.md` is added: the task asks for Diátaxis to be cited
  on a contributing page, and `CLAUDE.md` is an agent file that the package
  excludes.
- The nav keeps nested groups (`Project` → `Verification`, `Research`,
  `Decisions`). `scripts/gen-llms-txt.sh` refused nested groups; it is
  extended to one level of nesting, naming the section "Parent: Child", so
  the tree is not flattened to suit the script.
- `docs/research/client-comparison.md` and
  `docs/research/system-one-client-libraries.md` are not merged (section 3).

## 5. Choices

**The plugin's `references/format.md`.** Generated from
`docs/reference/jud-format.md` by `scripts/gen-plugin-format.sh`, checked by
`--check` in `mise run docs:check` and CI. The alternative, a test that
compares the two pages' field sets, would hold the field names together and
nothing else: the rules a writer trips on (`strict`, `fallback`, what
`options_from: request` requires) are prose, and a test that parses prose
tables is brittle and silent on the rules. A generator ships the spec itself
inside the plugin, with its relative links rewritten to absolute URLs on the
default branch and a header saying where it came from, so the plugin stays
self-contained and cannot drift. `SKILL.md` remains the condensed guide; it
is prose for an agent and is not generated.

**`reference/cli.md`.** Generated by `scripts/gen-cli-reference.sh` from the
clap tree: a hand-written header (frontmatter, the variables, exit status)
and one fenced block per command, the `--help` text verbatim, between
markers. `--check` runs in `docs:check` and CI, so the page cannot drift from
the binary. Chosen over a test that diffs the page against the help: the
generator gives the writer a way to refresh the page, and the check is the
same comparison.

**Rust blocks.** A tutorial's code is an `examples/` file, held to the page
verbatim by `tests/docs_examples.rs` (the generalisation of
`tests/readme.rs`), because a tutorial is run, not only compiled. A guide's
fragments are compiled as doctests by `#[cfg(doctest)]` includes in
`src/lib.rs`, gated on the features they need.

**`.jud` blocks.** A whole document on a page is a file under
`examples/jud/`, named by an HTML comment before the fence, held verbatim by
the same test, and read by `jud check` in `mise run jud:check` and CI. A
fragment (one question, one gate) is marked `yaml` with no file and is not
checked.

**Standards.** Linked at first mention per page, only where the code relies
on the standard: RFC 8785 (`src/eval/canonical.rs`), RFC 8259 (JSON state),
RFC 3339 (`recorded_at`), RFC 9110 (`Retry-After`, the status set), YAML
1.2.2 (`serde-saphyr`), FIPS 180-4 (`sha256:` fingerprints), SemVer and
Keep a Changelog (`CHANGELOG.md`), Conventional Commits (`cog`), the
Kubernetes object model (the envelope, decision 0018), Brier 1950, Naeini
et al. 2015 and Wilson 1927 (`src/eval/metrics.rs`), BCP 14 (the spec's
keywords), Diátaxis (`project/contributing.md`). RFC 6901 is cited only to
say the state-path syntax is not it.
