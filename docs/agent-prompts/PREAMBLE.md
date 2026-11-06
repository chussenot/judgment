---
title: Agent prompt preamble
description: The working rules for coding agents contributing to judgment, read before any task; establish the repository context, keep the change small and scoped, treat the System One wire as a contract, the Rust, CLI and data-handling standards, tests and verification, documentation and release discipline, and what an agent's final report contains.
status: current
last_reviewed: 2026-11-06
tags: [judgment, agents, contributing, prompts]
llms: false
---

# Agent prompt preamble

This file defines the working rules for coding agents contributing to
judgment.

judgment is a Rust client library and CLI (`jud`) for the
`/v1/systemone` decision-model wire. The goal is to make small,
reviewable changes while preserving protocol correctness,
reproducibility, and compatibility.

## 1. Establish the repository context first

Before changing anything:

1. Read the repository-level `CLAUDE.md` and `AGENTS.md`,
   `docs/project/contributing.md`, relevant architecture and design
   documents (`docs/project/internals.md`, `docs/project/decisions/`),
   and any more specific instructions in the directories you will touch.
2. Inspect `Cargo.toml`, the workspace structure, the relevant source
   modules, tests, documentation, and the current `CHANGELOG.md`
   conventions.
3. Check the current branch, working-tree status, recent commits, and
   related open issues or PRs. Do not overwrite or discard unrelated
   user changes.
4. Search for existing implementations, flags, types, tests, and
   documentation before adding new ones.
5. If the task refers to an issue, specification, paper, upstream
   change, or external API, read the primary source. Treat summaries
   and issue comments as leads, not as the specification.

If an expected file, command, feature, or prerequisite is missing, stop
and report the concrete gap. Do not invent repository conventions or
silently work around a missing prerequisite.

## 2. Keep the change small and scoped

- Implement only the requested behavior. Avoid opportunistic
  refactors, unrelated formatting, dependency upgrades, and broad
  renames.
- Aim for one focused PR. If the work cannot reasonably be completed
  and tested in roughly half a day, split it into independently
  reviewable changes and explain the dependency.
- Preserve existing public APIs and CLI behavior unless the task
  explicitly requires a change.
- Prefer the smallest compatible implementation that fits established
  project patterns.
- Do not add a dependency when the standard library or an existing
  dependency is sufficient.
- Do not introduce hidden network calls, telemetry, background work,
  or implicit data collection.
- Do not hard-code vendor-specific assumptions when they belong in
  configuration or protocol data.

## 3. Treat the System One wire as a contract

- Distinguish the TypeSafe Jev service from other servers that
  implement `/v1/systemone`.
- Do not conflate TypeSafe System One / Jev with Meta JEPA, unrelated
  Hugging Face models, or Scala/Akka Typesafe.
- Use the protocol specification and current primary vendor
  documentation as the authority for request and response semantics.
- Keep protocol-level behavior separate from vendor-specific
  extensions. Preserve unknown or optional response fields where the
  existing design expects forward compatibility.
- Handle optional fields, missing confidence values, numeric
  boundaries, and server-specific metadata explicitly. Do not silently
  coerce invalid data into plausible results.
- Do not assume that every compatible server supports every primitive,
  model, image input, option count, or extension.
- When documenting limits, headers, prices, model versions, or other
  vendor-specific facts, include the source URL and verify the value
  against current documentation before merging.
- Never describe a behavior as guaranteed by the wire when it is only
  observed in one provider or server implementation.

## 4. Rust implementation standards

- Follow the existing module boundaries, naming, error-handling, and
  serialization patterns.
- Prefer explicit types and errors over stringly typed state or silent
  fallback behavior.
- Avoid unnecessary cloning and allocations, but prioritize clear
  ownership and correctness over speculative micro-optimizations.
- Keep public types and methods documented. Add or update examples
  when they materially clarify public behavior.
- Preserve MSRV and dependency constraints declared by the project. Do
  not raise them without an explicit requirement.
- Do not use `unwrap()` or `expect()` on user-controlled,
  network-controlled, or otherwise fallible runtime input. Tests may
  use them where appropriate.
- Ensure errors identify the relevant case, question, field, or
  operation without leaking secrets or full sensitive payloads.
- Keep deterministic behavior deterministic. If randomness is
  required, make its source and reproducibility explicit.

## 5. CLI and data-handling standards

- Preserve existing command behavior and output unless a change is
  explicitly requested.
- Validate all local inputs and cross-references before making network
  requests whenever possible.
- Give actionable validation errors with enough context to locate the
  invalid input.
- Keep text and machine-readable output consistent. JSON output must
  remain valid JSON and must not contain progress messages or
  diagnostics on stdout.
- Send diagnostics to stderr where appropriate.
- Treat recordings, replay files, fingerprints, and cache keys as
  compatibility-sensitive interfaces. Include all behavior-changing
  inputs in fingerprints and test replay compatibility.
- Never write API keys, authorization headers, or other secrets to
  logs, fixtures, snapshots, or recordings.
- Make offline tests possible for parsing, validation,
  transformations, metrics, and replay logic.

## 6. Tests and verification

Before implementation, identify the tests that should fail without the
change.

Add tests for:

- the expected behavior;
- boundary and malformed inputs;
- optional or missing fields;
- error paths and validation ordering;
- backward compatibility and unchanged behavior when new options are
  absent;
- deterministic output, recording, and replay where relevant.

Prefer offline unit and integration tests. Do not make ordinary test
suites depend on live vendor credentials or external service
availability.

Run the narrowest relevant tests first, then the repository's documented
quality checks. Discover the current commands from `mise.toml`, the CI
workflows, and project documentation rather than guessing. Report
exactly which commands were run and whether they passed. If a check
cannot be run, explain why; do not claim it passed.

## 7. Documentation and release discipline

- Update documentation alongside user-visible behavior, including CLI
  help/reference material and examples.
- Update `CHANGELOG.md` according to the repository's existing release
  convention when behavior, public API, CLI, or user-facing output
  changes.
- Do not add a changelog entry for a documentation-only research-note
  update unless the project convention requires it.
- Keep examples executable or clearly label them as illustrative.
- Cite primary sources for external claims and distinguish measured
  results, vendor claims, proposals, and unverified reports.
- Do not turn a single issue report or hardware-specific observation
  into a universal compatibility statement.

## 8. Agent execution and reporting

- Do not claim that a command, test, benchmark, live request, or
  deployment succeeded unless you actually ran it and observed the
  result.
- Do not create commits, push branches, open PRs, publish crates, or
  modify remote resources unless the task explicitly authorizes that
  action.
- Do not modify unrelated files or "clean up" pre-existing changes.
- If the requested implementation conflicts with the protocol,
  repository conventions, or a higher-priority instruction, stop and
  explain the conflict instead of guessing.
- Before finishing, inspect the diff and confirm that it contains only
  task-related changes.

Your final response must be concise and include:

1. **Implemented**: what changed and the user-visible effect.
2. **Files**: the important files changed.
3. **Verification**: exact commands run and their outcomes.
4. **Limitations**: anything not tested, unresolved, or dependent on
   external confirmation.
5. **Follow-up**: any genuinely necessary next step, with the reason.

Do not report a task as complete if acceptance criteria remain unmet.
