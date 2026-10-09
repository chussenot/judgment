---
title: Stability
description: What stays the same across versions and for how long, so a document, a recording or a dependency written today keeps working; the jud/v1 format is stable, a minor version only adds, a reader accepts every v1 document for at least twelve months after a later minor is published, fingerprints exclude the envelope and never change for a document's spec, the jud command's exit status, JSON output and file writes are a contract, and the crate's 0.x releases say what they break.
status: current
last_reviewed: 2026-10-09
tags: [judgment, jud, stability, versioning, compatibility, fingerprint, reference]
---

# Stability

What is promised, what is not, and for how long, for the format, its fingerprints, the wire, the crate and the command. The [decision records](../project/decisions/README.md) say why each rule is as it is; this page is the one to cite.

## The format: `jud/v1` is stable

The `.jud` format is at `apiVersion: jud/v1.3` ([The .jud format](jud-format.md)), and the `v1` line is stable:

- **A valid document stays valid.** A document that a `jud/v1` reader accepts today is accepted by every later `jud/v1` reader. Nothing is removed, renamed or given a new meaning inside the `v1` line: not a field of the envelope, not a field of `spec`, not a kind, not a rule of reading.
- **A minor version only adds.** A later minor (`1.4`, `1.5`) may add an optional field, a value a field may take, or a kind. It may not make an optional field required, narrow what a field accepts, or change what an existing field means. A document that uses nothing a later minor added is, byte for byte, a document of the earlier minor.
- **Readers accept every `v1` document for at least twelve months.** When a later minor is published, every reader of the format that this repository ships (the crate's `judgment::jud`, the `jud` command, the plugin's checks) accepts documents of each earlier `v1` minor for at least twelve months from that publication. The date is the release date of the crate version that first reads the new minor, recorded in the `CHANGELOG`.
- **A change of meaning is `jud/v2`.** Anything a minor may not do takes a new major version, with its own `apiVersion`, its own schemas and a page that maps `v1` onto it. A `v2` reader is not required to read `v1`; a `v1` document is moved by rewriting it.

Today the one `v1` minor published is `1.3`, and the crate's reader reads exactly `apiVersion: jud/v1.3`. How the next minor is spelled, and how a reader of it also reads `1.3`, is [decision 0020](../project/decisions/0020-how-a-v1-minor-is-spelled.md), proposed and not applied; whichever spelling is accepted, the promises on this page hold.

## Fingerprints exclude the envelope and never change

A rubric's questions, a rubric's policy, a cases document's cases and a request each have a fingerprint: `sha256:` over the [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) canonical JSON of a value inside `spec` ([Fingerprints](jud-format.md#fingerprints)).

- **The envelope contributes nothing.** `apiVersion`, `kind` and the whole of `metadata` are outside every fingerprint. Renaming a document, labelling it, annotating it, or moving it to a later `apiVersion` leaves each fingerprint as it was.
- **A document's `spec` has one fingerprint, for good.** The value fingerprinted and the canonicalisation are fixed for `v1`. A minor may add a field to `spec`; a document that does not use it fingerprints as it did. A change to what is fingerprinted or how would be a change of meaning, so it is `v2`.
- **The known vector holds.** The specification carries a worked fingerprint; `tests/jud.rs` checks the crate against it, and another implementation can check itself the same way.

## The wire

What the crate sends and accepts on `POST /v1/systemone` follows the TypeSafe HTTP API reference, vendored as `tests/fixtures/typesafe-openapi.json` and checked by `tests/contract.rs`. A change there is a contract change: the test, the `CHANGELOG` and the README's guarantees move together, and the recordings under `examples/recordings/` must still replay. The decoder keeps an unknown answer kind readable as `Answer::Unknown` rather than failing the response ([Internals](../project/internals.md)).

## The crate and the command

The crate is 0.x on crates.io: a minor release may break, a patch release does not, so a consumer pins the minor ([Install](../start/install.md#the-crate)). Every release lists its breaking changes under its own heading in [`CHANGELOG.md`](../../CHANGELOG.md), in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) form under [Semantic Versioning](https://semver.org/spec/v2.0.0.html), and a breaking change arrives in a minor bump, never a patch ([Releasing](../project/releasing.md)).

The `jud` command follows the crate's version, and its contract is what a script reads and what the command writes. A change to what an existing status, JSON key, file write or setting means, or removing one, is listed as breaking. An addition ships in a minor release, is listed under Added and joins the contract.

- **Exit status.** 0, 1, 2 and 3, as [the command line reference](cli.md#exit-status) defines them. Status 3 is `jud eval`'s alone: the evaluation ran and a `--min-accuracy` bar was not met. Statuses 0, 1 and 2 keep their meaning.
- **Stdout.** The result goes to stdout and every diagnostic to stderr, with the one exception that [the command line reference](cli.md#exit-status) names: `jud check`'s refusals are lines of its result. The result is the verdict JSON of a run ([Verdicts](cli.md#verdicts)) and, for `jud eval --json`, the keys and types of [the JSON report](cli.md#the-json-report). `jud tune` prints the `policy` and `tuning` blocks of the `jud/v1` format, which hold under that format's own promises. The text of `jud eval`'s report, the tables and notes of `jud tune` and the progress of `jud record` are for a person: a script reads `--json` and the exit status.
- **Files written.** Three commands write files and no other does: `jud record`, into its `--out` directory, `jud tune --out`, one rubric file, and `jud split`, two new cases files. None writes over a rubric or a cases document it was given, and `jud split` writes over no file at all; [the command line reference](cli.md#exit-status) states the rule.
- **Configuration.** What the command reads, and in what order ([Configuration](configuration.md)), including which commands read `JUD_REPLAY`.

Exit status 3, `jud eval`'s `--json` keys and the two places that write files are such additions: they are part of the contract from the release that ships `jud record`, `jud eval` and `jud tune`, and not a break ([decision 0021](../project/decisions/0021-record-eval-and-tune-from-the-command-line.md)). So are `jud split`, the files it writes, and the `outcomes`, `majority` and `signals` keys of `jud eval --json` ([decision 0022](../project/decisions/0022-jud-reports-what-the-tuning-loop-counted-by-hand.md)).

## In one table

| What | Promise | Since |
|---|---|---|
| A `jud/v1` document | Valid today, valid under every later `v1` minor | `jud/v1.3`, crate 0.8.0 |
| A `v1` minor | Adds only; never removes, renames, narrows or changes meaning | this page |
| Readers of an earlier `v1` minor | Accepted for at least twelve months after a later minor is published | this page |
| A fingerprint | Over `spec` only; the envelope is outside it; one value per content, for good | `jud/v1.3`, decision 0018 |
| The wire | The vendored OpenAPI document; a change moves the contract test, the CHANGELOG and the README together | crate 0.3.0 |
| The crate | 0.x: a minor may break and says so; a patch does not | crate 0.3.0 |
| The `jud` command | Exit status (0 to 3), the verdict JSON, `jud eval`'s `--json` keys, the places that write files (`jud record --out`, `jud tune --out`, `jud split`) and the configuration: a change to what one means, or removing one, only in a minor, listed as breaking; an addition is listed under Added | crate 0.9.0; status 3, `eval --json` and the file writes from the release that ships `record`, `eval` and `tune` |
