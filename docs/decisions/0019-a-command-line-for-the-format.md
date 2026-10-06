---
title: 0019 A command line for the format, released as a binary
description: Why the jud binary evaluates stdin against a Rubric file rather than taking a bespoke input language, why its backend is the crate's client with TypeSafe as the default and a base URL as the only switch, why it reads a configuration file where the crate reads only the environment, and why it is released as one tarball per platform in the shape of pact's releases.
status: accepted
date: 2026-10-06
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-06
tags: [judgment, jud, cli, decision, release, mise]
---

# 0019 A command line for the format, released as a binary

## Context and problem statement

A `.jud` Rubric is a decision written as a file, so that a reviewer can read it and a program can run it. Until now the program had to be Rust: the crate's examples run a rubric, and the `jud` binary read documents for the plugin, but nothing let a shell evaluate a state against a rubric. The people who write rubrics with the plugin, and the pipelines that produce the states (`jq` over an event, `yq` over a manifest, `curl` from an API), had no way to run one without a crate in the loop.

Four things had to be decided: what the command takes and where, which backend answers and how it is chosen, where its configuration lives, and how a binary reaches a machine that has no Rust toolchain.

## Decision drivers

- The crate is the implementation; a command must not re-implement any of it, or the two drift.
- A Unix tool composes through stdin and stdout; a bespoke input language or an output format of its own would not.
- `SystemOne` is already the abstraction over backends, and the client already serves every server that speaks the wire; a provider switch in the command would be a second abstraction over the first.
- A tool installed on a workstation needs a place for its configuration that the crate, a library, never needed.
- A release must be installable without Rust, resolve unambiguously under a version manager, and keep the crate's own publish to crates.io intact.

## Considered options

1. **stdin is the state, the argument is the rubric** (`cat state.json | jud rubric.jud`), JSON only; verdicts printed in the library's own serialisation.
2. The argument is the state file and the rubric comes from a flag or a search path.
3. A richer input language (YAML states, several states per run, options supplied on the command line).
4. For the backend: a `--provider` switch with named providers.
5. For the backend: the crate's client, TypeSafe by default, another server by base URL only.
6. For configuration: environment variables only, as the crate.
7. For configuration: `~/.config/jud/config.yaml` under the environment, the environment winning.
8. For the release: `cargo install` only; `cargo-dist`; a hand-written matrix in the shape of [pact's](https://github.com/chussenot/pact/blob/main/.github/workflows/release.yml), one tarball per platform, checksums, attestation, assets on the release the crate's publish already creates.

## Decision outcome

Options 1, 5, 7 and the hand-written matrix of 8.

**The pipeline model.** The rubric is the one argument because it is the decision and rarely changes; the state is what varies and is what the shell is good at producing, so it comes from stdin as one JSON value. JSON only, because every tool in the pipeline speaks it and `yq -o=json` turns YAML into it; a second input language would be a thing to learn and to parse. The output is the verdict map as `judgment::jud::Verdict` serialises it, the same bytes the examples print, so a program and a shell see one representation. Several states per run, and options supplied for a Choice that takes them from the request, are left for later: a run that needs them says so rather than guessing.

**The backend.** The command builds the crate's `Client` and asks through `SystemOne`. TypeSafe is the default because the crate's defaults are the SDKs' and that is what a first run should hit; another server is a base URL because that is all another server is to the client. A provider abstraction would have named what a URL already names.

**The configuration file.** The crate reads `TYPESAFE_API_KEY` and nothing else, which is right for a library; a command on a workstation needs to remember a base URL and a model between runs, and a key where the environment is awkward. `~/.config/jud/config.yaml`, with `XDG_CONFIG_HOME` honoured, holds `base_url`, `api_key`, `model` and `timeout_secs`; the environment wins over the file, so the crate's variables keep their meaning and a one-off `TYPESAFE_BASE_URL=... jud` still works. `jud config` prints the resolution with the key never shown, because "which backend did that hit" is the first support question.

**The release.** One tarball per platform named `jud-<tag>-<triple>.tar.gz`, built on native runners so that every leg runs the binary it built, stripped and re-signed on macOS, checked (`--version`, `check` over the examples, `config` without a network) before it is packaged, with `SHA256SUMS` and a build-provenance attestation, attached to the GitHub release the crate's publish already creates from the same tag. That is pact's shape, taken rather than re-derived: it is what makes `mise use -g github:chussenot/judgment@latest` resolve, and what lets a checksum and a provenance be verified. `cargo-dist` would have generated a pipeline the repository owns without having written; `cargo install` alone would have kept the Rust toolchain a prerequisite. Intel macOS and Windows are not built, for the reasons pact gives: no native runner for the first, and a Unix pipeline model for the second.

### Consequences

- `cargo install judgment --features jud` still works; the tarball is the way without Rust.
- A failed binary leg stops the publish, crates.io included, because the publish job needs every leg. That is deliberate: a version whose binary does not build is not released, and the runbook says how to retag once the cause is fixed.
- The command's exit status is a contract: 0 verdicts, 1 the backend failed, 2 something fixable before a call. Scripts may rely on it.
- The `jud` feature now also builds the binary's dependencies on an async runtime (`tokio` with `rt`), which the `http` feature already carried.

### Confirmation

`tests/jud_cli.rs` runs the binary as a subprocess against a wiremock server for every outcome: verdicts, the serialisation, an invalid or missing rubric, invalid or empty stdin, a backend failure, an answer that does not fit, missing credentials, the default backend, a backend configured in the file and the environment overriding it, a malformed file. The release workflow's check step runs the packaged binary on each platform.

## Pros and cons of the options

- **stdin state, rubric argument (chosen).** Composes with every tool; one argument to remember. Cannot batch several states in one call without a wrapper.
- **State as the argument.** Reads naturally for a file on disk, but puts the varying thing where the shell cannot pipe it.
- **A richer input language.** Would let one run do more, at the price of a parser the crate does not have and a format nobody else writes.
- **A provider switch.** Familiar from other tools, but a second name for what the base URL already selects, and a list to keep in step with the servers page.
- **The client, URL-selected (chosen).** No new abstraction; every server the crate has been verified against is reachable the same way.
- **Environment only.** Simplest, and what the crate does; but a workstation tool that needs four values in the environment of every shell is one people stop using.
- **A configuration file (chosen).** One more place a value can come from, made legible by `jud config`.
- **`cargo install` only.** No pipeline to maintain; a toolchain as a prerequisite for a tool whose users write YAML.
- **`cargo-dist`.** A thousand lines of generated workflow to own.
- **The hand-written matrix (chosen).** Two screens of YAML with every unusual line explained, already proven on pact.

## More information

- [The jud command line](../cli.md), the user's page.
- [Releasing](../releasing.md), the runbook with the binary legs.
- [Decision 0013](0013-releases-cut-with-cocogitto-and-published-from-ci.md), the crate's release process the binary joins.
- [pact's release workflow](https://github.com/chussenot/pact/blob/main/.github/workflows/release.yml) and [install page](https://github.com/chussenot/pact/blob/main/docs/install.md), the shape taken.
