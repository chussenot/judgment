---
title: Stability
description: What stays the same across versions and for how long, so a document, a recording or a dependency written today keeps working; the jud/v1 format is stable, a minor version only adds, a reader accepts every v1 document for at least twelve months after a later minor is published, fingerprints exclude the envelope and never change for a document's spec, and the crate's 0.x releases say what they break.
status: current
last_reviewed: 2026-10-07
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

The `jud` command follows the crate's version, and its contract is three things: the exit status, the verdict JSON on stdout, and the configuration it reads ([The jud command line](cli.md), [Configuration](configuration.md)). A change to any of them is listed as breaking.

## In one table

| What | Promise | Since |
|---|---|---|
| A `jud/v1` document | Valid today, valid under every later `v1` minor | `jud/v1.3`, crate 0.8.0 |
| A `v1` minor | Adds only; never removes, renames, narrows or changes meaning | this page |
| Readers of an earlier `v1` minor | Accepted for at least twelve months after a later minor is published | this page |
| A fingerprint | Over `spec` only; the envelope is outside it; one value per content, for good | `jud/v1.3`, decision 0018 |
| The wire | The vendored OpenAPI document; a change moves the contract test, the CHANGELOG and the README together | crate 0.3.0 |
| The crate | 0.x: a minor may break and says so; a patch does not | crate 0.3.0 |
| The `jud` command | Exit status, verdict JSON and configuration change only in a minor, listed as breaking | crate 0.9.0 |
