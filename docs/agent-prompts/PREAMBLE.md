---
title: Agent prompt preamble
description: The working rules for coding agents contributing to judgment, read before any task; the repository context and its trackers (beads, pact), the scope discipline (ponytail), the System One wire as a contract, the Rust, CLI and data-handling standards, tests and the gates, documentation and release discipline, and what an agent's final report contains.
status: current
last_reviewed: 2026-11-06
tags: [judgment, agents, contributing, prompts]
llms: false
---

# Agent prompt preamble

This file defines the working rules for coding agents contributing to
judgment. It is a prompt, not a page for readers: it names the rules
`CLAUDE.md`, `AGENTS.md` and `docs/project/contributing.md` hold, in the
order an agent meets them, and defers to those files where they say more.

judgment is a Rust client library and CLI (`jud`) for the
`/v1/systemone` decision-model wire. The goal is to make small,
reviewable changes while preserving protocol correctness,
reproducibility, and compatibility.

## 1. Establish the repository context first

Before changing anything:

1. Read `CLAUDE.md` (the rules that are easy to get wrong, the harness)
   and `AGENTS.md` (beads, and the pact protocol for several agents in one
   checkout), then `docs/project/contributing.md` (the gates, how work is
   tracked, commits and releases, how the documentation is organised).
   `docs/index.md` maps the documentation; `docs/project/internals.md`
   says how the crate is built; `docs/project/decisions/` holds the records
   that govern the API and the `.jud` format.
2. Run `bd prime` for the session rules and the memories earlier sessions
   left (`bd memories`), then `bd ready` for the open work. Create or
   claim a bead before code (`bd create`, `bd update <id> --claim`); the
   export under `.beads/issues.jsonl` travels with the commit, never by
   hand. Never run `bd init` or `bd edit`.
3. In a checkout other agents share, announce before you read: `pact msg
   inbox`, `pact lease ls`, then `pact lease acquire <paths> --note "..."`
   for the files you expect to own, `.beads/` included when you will run
   `bd`. A path someone else holds exits 2; wait inside the command
   (`--wait`), never by polling.
4. Inspect `Cargo.toml`, the relevant source modules, their tests, the
   pages that describe them, and the `## [Unreleased]` section of
   `CHANGELOG.md`.
5. Check the current branch, the working tree, recent commits, and the
   related beads and pull requests. Do not overwrite or discard unrelated
   changes.
6. Search for existing implementations, flags, types, tests, and
   documentation before adding new ones (`refactor-scout` ranks dead code
   and duplication when the task is a refactor).
7. If the task refers to a bead, a decision record, a paper, an upstream
   change, or an external API, read the primary source. A bead's
   description was written from the code of its day: read the code before
   trusting it. Treat summaries and issue comments as leads, not as the
   specification.

If an expected file, command, feature, or prerequisite is missing, stop
and report the concrete gap, as a bead when it is the repository's. Do
not invent repository conventions or silently work around a missing
prerequisite. A fact about the tools or the environment that cost time to
learn goes in `bd remember "..." --key <slug>`, then `mise run bd:sync --
export` and a commit of `.beads/memories.jsonl`.

## 2. Keep the change small and scoped

Ponytail is required here: the smallest complete change, with its checks
kept whole.

- Implement only the requested behavior. No code the task does not need,
  no option, wrapper, abstraction or "for later" code nobody asked for;
  no opportunistic refactors, formatting, dependency upgrades, or renames.
- The standard library and what the crate already has come before a new
  dependency or abstraction. A dependency change needs the four gates
  `mise run check` leaves out (`deny`, `msrv`, `hack`, `semver`), run one
  at a time, and a licence not on the allow list needs a comment in
  `deny.toml`.
- A deliberate shortcut gets a `ponytail:` comment saying what was skipped
  and when to come back; `/ponytail-debt ponytail:` collects them.
- Aim for one focused pull request. If the work cannot reasonably be
  completed and tested in roughly half a day, split it into independently
  reviewable changes and say what depends on what.
- Preserve the public API and the `jud` command's contract unless the
  task requires a change. The crate is 0.x: a break ships in a minor,
  listed as **Breaking** in the CHANGELOG, and the `semver` gate holds
  the rest to additions.
- The crate is generic. Nothing about any one application goes in. Token
  usage and failed attempts reach the caller through `judgment::Observer`;
  the crate emits `tracing` spans and no metrics of its own. The `http`
  feature gates the client, and everything else builds without it
  (`mise run check:minimal`).
- No hidden network calls, telemetry, background work, or implicit data
  collection. The `jud` command's telemetry is a switch the user sets.

## 3. Treat the System One wire as a contract

- Load the `typesafe:typesafe-ai` skill before touching questions,
  answers, the client or the backends. The live documentation is the
  contract; start at https://docs.typesafe.ai/llms.txt.
- The OpenAPI document is vendored at `tests/fixtures/typesafe-openapi.json`,
  checked by `tests/contract.rs`, and refreshed only through
  `tests/openapi_drift.rs` (`mise run openapi:drift`, then
  `openapi:refresh`). Never hand-edit it.
- Distinguish the TypeSafe hosted API and its Jev models from the other
  servers that speak `/v1/systemone` (Laya, Ollama, Clef, Liquid,
  OpenRouter and the rest on `docs/project/research/compatible-servers-and-models.md`).
  Do not conflate TypeSafe System One or Jev with Meta JEPA, unrelated
  Hugging Face models, or Scala/Akka Typesafe.
- Every backend holds a response against the questions it was sent
  (`Response::verify`): an answer for every question, of its primitive, a
  Choice naming only offered options, a Score whose legend parses to the
  levels sent. An off-list option is an error, never a default. Do not
  coerce invalid data into plausible results.
- Keep protocol-level behavior apart from a server's extensions. An
  answer's unknown fields stay in its `extra` map and are written back,
  so a recording keeps what the server sent; `Response::extra` does the
  same at the top level.
- Handle optional fields, a missing confidence, numeric boundaries, and
  server-specific metadata explicitly. Errors are grouped by what fixes
  them (`Error`); a 2xx the client cannot use is `decode` or `unfit` to
  the observer and is never retried; every HTTP call goes through
  `http::send_with_retries`.
- Do not assume that every compatible server supports every primitive,
  model, image spelling, option count, or extension. What a real server
  did is recorded under `docs/project/verification/`, from the ignored
  live tests in `tests/live.rs` under a `JUDGMENT_LIVE_PROFILE`; a
  behavior seen on one server is a row on the compatible-servers page,
  never a promise of the wire.
- A change to what the crate sends or accepts on the wire is a contract
  change: `tests/contract.rs`, the CHANGELOG and the README's guarantees
  move together, and the recordings under `examples/recordings/*/` must
  still replay (`cargo test` runs them). The `contract-reviewer` agent
  reads such a diff before a pull request.
- When documenting limits, headers, prices, model versions, or other
  vendor facts, name the source URL and the date read, as the research
  pages do, and verify the value against current documentation before
  merging.

## 4. Rust implementation standards

- Follow the existing module boundaries, naming, error-handling, and
  serialization patterns. The toolchain is pinned (`rust-toolchain.toml`)
  and the crate promises `rust-version` 1.91 (`mise run msrv`); do not
  raise either without an explicit requirement.
- Clippy runs pedantic with warnings denied; `unwrap_used`,
  `expect_used` and `missing_docs` warn, and `unsafe_code` is forbidden
  (`Cargo.toml`). No `unwrap()` or `expect()` on user-controlled,
  network-controlled, or otherwise fallible runtime input; tests may use
  them where the crate's lint configuration allows.
- Prefer explicit types and errors over stringly typed state or silent
  fallback. A verified answer can only be read as what was asked, and a
  probability is a probability (`Probability`, `Confidence`).
- Avoid unnecessary cloning and allocations, but prioritize clear
  ownership and correctness over speculative micro-optimizations.
- Keep public types and methods documented, with the why the code cannot
  show; `cargo doc` runs with broken links as errors. Add or update an
  example when it materially clarifies public behavior.
- Errors name the case, question, field, or operation. A server's text
  is shown as text (control characters escaped, `out::plain` in `jud`),
  and no error, log, recording, snapshot or message carries a key or an
  authorization header.
- Keep deterministic behavior deterministic. Recordings replay byte for
  byte; a property test's failures are shrunk and committed under
  `proptest-regressions/`.

## 5. CLI and data-handling standards

`docs/reference/stability.md` says what the `jud` command promises; a
change to what any of it means is breaking.

- Exit statuses 0 to 3 keep their meanings (`docs/reference/cli.md`,
  Exit status). The result goes to stdout and every diagnostic to stderr
  as a `jud:` line (`out::note!`); JSON output stays one valid object with
  nothing else on stdout. `jud eval`'s `--json` keys are a contract: an
  addition ships in a minor, a change of meaning is breaking.
- Three commands write files and no other does: `jud record --out`,
  `jud tune --out` and `jud split`. None writes over a document it was
  given.
- Validate local inputs and cross-references before any network request:
  a rubric's images, a case's options, a missing recording. Give
  actionable errors with enough context to locate the invalid input.
- The `.jud` format is a specification other tools implement. An addition
  to what a document may hold is the next minor and moves
  `docs/reference/jud-format.md`, `schemas/jud/`, `tests/jud.rs` and the
  documents under `examples/jud/` together, with a decision record.
  `Questions` and a Choice's options keep insertion order: it is the
  order the model sees.
- Fingerprints are RFC 8785 canonical JSON over values inside `spec`
  (`src/eval/canonical.rs`), checked against the known vectors on the
  format page. Treat recordings, replay directories, fingerprints and
  request hashes as compatibility-sensitive: every behavior-changing
  input is in the fingerprint, and replay compatibility is tested.
- Secrets live in `.env`, gitignored and loaded by `mise`; the hooks
  refuse to commit the file by name and a key anywhere by `gitleaks`.
  Never write a key to a log, fixture, snapshot, or recording.
- Parsing, validation, lowering, metrics, grading and replay are testable
  offline, and stay so.

## 6. Tests and verification

Before implementation, identify the tests that should fail without the
change.

Tests never call a real API: wiremock for the client, `Fake` and the
recordings for everything above it. The exceptions are `tests/live.rs`,
all `#[ignore]` and run by hand (`mise run live:*`), and
`tests/openapi_drift.rs`. Add tests for:

- the expected behavior, and the negative case of every check;
- boundary and malformed inputs;
- optional or missing fields;
- error paths and validation ordering;
- backward compatibility and unchanged behavior when new options are
  absent;
- deterministic output, recording, and replay where relevant.

Where they live: a unit test beside the code; `tests/client.rs` for the
wire (wiremock, and a raw server for what wiremock cannot send);
`tests/contract.rs` for the wire's shape; `tests/jud*.rs` for the
command, as a subprocess; `tests/properties.rs` for invariants
(proptest); `tests/snapshots.rs` for what the crate sends and reports
(insta: `cargo insta review` accepts a change, `mise run snapshots` says
none is pending, CI never writes one); `tests/docs_examples.rs` holds the
pages' code and transcripts to the binary. The `test-writer` agent writes
in this style when a diff under `src/` carries no test.

Run the narrowest relevant tests first, through `mise exec -- cargo test
--all-features --test <name>` (cargo runs through the mbx wrapper here;
pass the features a test target declares), then `mise run check`, which
runs CI's local gates in CI's order: fmt, clippy, `check:minimal`,
`test`, `doc`, `docs:check`. A dependency change adds `deny`, `msrv`,
`hack` and `semver`, one `mise run` each. CI also mutates the diff of a
pull request that touches `src/answer.rs`, `src/http.rs` or `src/eval/`;
a survivor is a test to strengthen or a line that does nothing. Report
exactly which commands were run and whether they passed. If a check
cannot be run, explain why; do not claim it passed.

## 7. Documentation and release discipline

- `docs/` follows Diátaxis: tutorials under `start/`, guides under
  `guides/`, reference under `reference/`, explanation under `concepts/`,
  and what is about the repository under `project/`; a page holds one
  kind and a fact has one page. Every page carries frontmatter and is in
  the `mkdocs.yml` nav (or opts out with `llms: false`, as this one does).
- Generated, never edited by hand: `docs/reference/cli.md`,
  `plugins/jud/skills/jud/references/format.md`, `docs/llms.txt` and
  `docs/llms-full.txt`; regenerate with their scripts, and `mise run
  docs:check` checks them and the links. Code on a page is held to the
  code: a transcript is reproduced from the binary, a `.jud` block is a
  file under `examples/`.
- Update the documentation beside user-visible behavior, the CLI help
  (which is the reference) and the examples included. The `docs-auditor`
  agent reports claims that no longer hold; `docs-writer` edits.
- `CHANGELOG.md` is Keep a Changelog: an entry under `## [Unreleased]`
  for a change to behavior, the public API, the CLI, the wire or the
  format, marked **Breaking** where it is. A documentation-only research
  note needs none. A release is `cog bump --auto` on `main` and never a
  hand-edited version.
- A decision that shapes the API or the format is a record under
  `docs/project/decisions/`, numbered after the highest in use, never
  edited after acceptance (a later record supersedes it), in the nav and
  the index.
- Cite primary sources for external claims and distinguish measured
  results, vendor claims, proposals, and unverified reports. Do not turn
  a single issue report or hardware-specific observation into a universal
  compatibility statement.

## 8. Agent execution and reporting

- Do not claim that a command, test, benchmark, live request, or
  deployment succeeded unless you ran it and observed the result.
- Do not create commits, push branches, open pull requests, publish, or
  modify remote resources unless the task authorizes it (the conservative
  profile in `AGENTS.md`). Where it does: Conventional Commits, one
  concern per commit, with an explicit pathspec (`cog verify` runs on each
  message); the `pr-ready` skill picks the reviewers the diff needs
  (`contract-reviewer`, `docs-auditor`, `test-writer`), runs the gate and
  hands the branch to `pr-shepherd`, which opens a **draft**; the
  `steward` skill drives an open pull request (a merge commit to bring
  `main` in, never a rebase or a force push; generated files regenerated;
  the beads export checked after a merge; no checks at all meaning a
  conflict).
- The Bash guard denies what is never right here: a push to `main`, a
  pushed tag, `cargo publish` (the dry run is allowed), committing `.env`,
  and `bd edit`. A release and its tag are the release owner's.
- Close the bead when the work is done (`bd close <id> --reason`), leave
  a handoff where a dependent bead will read it, and end the session with
  the beads export committed and pushed: the issues exist nowhere else.
- Do not modify unrelated files or "clean up" pre-existing changes.
- If the requested implementation conflicts with the wire, the
  repository's rules, or a higher-priority instruction, stop and explain
  the conflict instead of guessing.
- Before finishing, inspect the diff and confirm that it contains only
  task-related changes, the beads export included.

Your final response must be concise and include:

1. **Implemented**: what changed and the user-visible effect.
2. **Files**: the important files changed.
3. **Verification**: exact commands run and their outcomes.
4. **Limitations**: anything not tested, unresolved, or dependent on
   external confirmation.
5. **Follow-up**: any genuinely necessary next step, with the reason, as
   a bead when it is the repository's.

End with what was skipped or not checked, and any risk the reader must
know. Do not report a task as complete if acceptance criteria remain
unmet.
