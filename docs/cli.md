---
title: The jud command line
description: The jud binary evaluates JSON input from stdin against a .jud Rubric and prints the verdicts, so a decision written as a file composes with jq, yq, cat and curl; what it does, how to install it with mise or by hand, how the backend is configured with TypeSafe as the default, the exit status, and the other subcommands.
status: current
last_reviewed: 2026-10-06
tags: [judgment, jud, cli, mise, release, configuration]
---

# The jud command line

`jud` evaluates JSON input against a `.jud` Rubric and returns the resulting verdicts. The rubric file is the decision: the questions a System One model is asked and the policy that turns its answers into actions. Standard input is the state the questions are asked about. Nothing else is needed.

```sh
cat input.json | jud rubric.jud
jq '.customer' customer.json | jud customer.jud
yq -o=json '.spec' resource.yaml | jud resource.jud
curl -s https://example.test/event | jud event.jud
```

The point is composition. Any tool that produces JSON prepares the state; `jud` asks the configured backend and prints one verdict per question as JSON; `jq` reads what comes out. The decision itself lives in a file a reviewer can read, not in a script. [What a rubric is](jud/rubric.md) says why.

## What happens on a run

1. The rubric file is read with the crate's own reader (`Rubric::parse`): a document that is not a `kind: Rubric`, or not valid `jud/v1.3`, is refused with the field named, before anything is read from stdin.
2. Stdin is read to the end and parsed as one JSON value. Empty input, invalid JSON and a stream of several JSON values (what `jq '.items[]'` prints) are each refused with a message that says what to pipe instead.
3. The rubric is lowered to the request for that state (`Rubric::lower`): the questions whose `when` holds, with the parts their `part_when` keeps. A rubric whose Choice takes its options from the request cannot be lowered by `jud` yet, and says so.
4. The configured backend answers (`Client`, the crate's own System One client, with the official SDKs' retries), and the response is verified against the questions as every backend's is.
5. The policy reads the answers into verdicts (`Rubric::apply`), which are printed to stdout as a JSON object keyed by question id, in the rubric's order, in the library's own serialisation:

```json
{
  "actionable": {
    "verdict": "yes",
    "probability": 0.91
  },
  "desk": {
    "verdict": "option",
    "key": "billing",
    "confidence": 0.8
  },
  "tone": {
    "verdict": "level",
    "index": 2,
    "label": "angry",
    "value": 1.9,
    "confidence": 0.9
  }
}
```

A Noul gives `yes` or `no` with its probability; a Choice gives `option` with the key, the confidence and the band when the gate has bands; a Score gives `level` with the index, the label, the weighted value, the confidence, and `reached` when the gate has `level_at_least`; a deferred answer gives `deferred` with the fallback, the nearest option or level and the bar it was below. The shape is `judgment::jud::Verdict`, the same the examples print.

`jud` is a thin adapter: parsing, lowering, the call, verification and the policy are the library's, and a program using the crate gets the same verdicts for the same state.

## Installing it

A release publishes one tarball per platform, so a version manager resolves it without ambiguity, and no Rust toolchain is needed.

```sh
mise use -g github:chussenot/judgment@latest
jud --version
```

Pin a version with `@0.9.0`; `mise ls-remote github:chussenot/judgment` lists what exists. The tool is named after the repository and the executable is `jud`; mise finds it inside the tarball.

By hand, with the checksum verified first:

```sh
TAG=v0.9.0
TARGET=x86_64-unknown-linux-musl
BASE="https://github.com/chussenot/judgment/releases/download/$TAG"
curl -fsSLO "$BASE/jud-$TAG-$TARGET.tar.gz"
curl -fsSLO "$BASE/SHA256SUMS"
sha256sum --ignore-missing -c SHA256SUMS
tar -xzf "jud-$TAG-$TARGET.tar.gz"
install -m755 "jud-$TAG-$TARGET/jud" /usr/local/bin/
```

Each tarball carries a build-provenance attestation: `gh attestation verify jud-$TAG-$TARGET.tar.gz --repo chussenot/judgment` says which workflow, commit and run produced it.

From source, with a Rust toolchain: `cargo install judgment --features cli` from crates.io, or `mise run install` in a checkout.

### Shell completion

`jud completion <shell>` prints a completion script on stdout, for `bash`, `zsh`, `fish`, `elvish` and `powershell`. It is generated from the same command tree the binary parses arguments with, so a subcommand or a flag is completed the moment it exists, and nothing is checked in to go stale. Where each shell wants it:

```sh
jud completion bash > ~/.local/share/bash-completion/completions/jud   # or /etc/bash_completion.d/jud
jud completion zsh  > "${fpath[1]}/_jud"
jud completion fish > ~/.config/fish/completions/jud.fish
```

Regenerate it after upgrading: a script written by an older binary completes that binary's commands, which is the one way it can still fall behind. `mise run install` does that for the three files above when their directory exists. The script reads no configuration and needs no key, so a profile can source it before anything else is set up; where a file name is expected (`RUBRIC`, `check FILE...`, `--state-file`, `--cases`) the shell completes paths.

### Platforms

| Target | Runs on |
|---|---|
| `x86_64-unknown-linux-musl` | any x86-64 Linux, statically linked, `FROM scratch` included |
| `aarch64-unknown-linux-musl` | any arm64 Linux |
| `aarch64-apple-darwin` | Apple-silicon macOS |

Every leg is built on a native runner and the binary is run before it is packaged, so no platform ships a binary nothing executed. Intel macOS has no native runner and is not cross-compiled; it installs from crates.io. There is no Windows build; the configuration directory and the pipeline model assume a Unix shell.

## Configuration

The backend is resolved in this order, each level overriding the one below:

| Level | Base URL | API key | Model | Timeout |
|---|---|---|---|---|
| Environment | `TYPESAFE_BASE_URL` | `TYPESAFE_API_KEY` | | |
| `~/.config/jud/config.yaml` | `base_url` | `api_key` | `model` | `timeout_secs` |
| Default | `https://api.typesafe.ai` | none | `jev-latest` | 30 |

The environment variables are the ones the crate and its examples already read; the file adds the two values the crate has no variable for. `XDG_CONFIG_HOME` moves the file's directory, as it does for every tool that honours it.

```yaml
# ~/.config/jud/config.yaml: every field optional
base_url: https://api.typesafe.ai
model: jev-latest
timeout_secs: 30
# api_key: ...   # prefer TYPESAFE_API_KEY, which no file on disk holds
```

`examples/jud/config-tev1.yaml` is the same file written for a local Ollama serving `tev1:0.8b`, where the key is a placeholder and may sit in the file ([Open-weight models without an account](open-weights.md)).

`jud config` prints what a run would use and where each value came from, with the key reported as `environment`, `config_file` or `missing` and never shown:

```sh
$ jud config
{
  "config_file": "/home/you/.config/jud/config.yaml",
  "config_file_present": false,
  "base_url": "https://api.typesafe.ai",
  "base_url_from": "default",
  "model": "jev-latest",
  "model_from": "default",
  "timeout_secs": 30,
  "api_key": "environment"
}
```

### Credentials

A key for the hosted API comes from `TYPESAFE_API_KEY`. A run with no key is refused before any call, naming the variable and the file. A key in the configuration file is accepted for a machine where the environment is awkward to set, with the usual caution about a secret on disk.

### Other System One backends

TypeSafe is the default, not the only backend. The crate talks to any server that speaks the System One wire through the same client, so another server is a base URL and nothing more: `TYPESAFE_BASE_URL=http://localhost:11434 jud rubric.jud` for an Ollama serving a decision model, or `base_url` in the file for good. [Compatible servers and models](research/compatible-servers-and-models.md) lists what has been run; `tools/systemone/serve.py` is the shim for the ones that do not expose the path directly. There is no provider switch in `jud`, because `SystemOne` is already the abstraction and the client already serves every compatible server.

## Exit status

| Status | Meaning |
|---|---|
| 0 | Verdicts printed on stdout. |
| 1 | The backend was asked and the call failed: a transport error after the retries, an HTTP error, or a response that does not fit the rubric. The message names the backend. |
| 2 | Something fixable before any call: no rubric argument, a file that cannot be read, a document that is not a valid Rubric, stdin that is empty or not one JSON value, a missing API key, a malformed configuration file. |

Every error goes to stderr, prefixed `jud:`; stdout carries verdicts and nothing else, so a pipeline never reads an error as a result.

## The other subcommands

`jud` was first the reader as a command, for the [jud plugin](skill.md), and keeps those subcommands:

| Command | What it does |
|---|---|
| `jud check FILE...` | Reads documents as the crate does, binds cases to the rubric they name among the files, verifies recordings against the request they answer, prints names and fingerprints; status 2 when any document is refused. |
| `jud lower RUBRIC --state JSON` / `--state-file PATH` / `--cases FILE` | Prints the request a rubric lowers to for a state or for every case, so a `when` or a `part_when` is seen rather than guessed. |
| `jud config` | The resolved backend, as above. |
| `jud completion SHELL` | A completion script for the shell, generated from the command tree ([above](#shell-completion)). |
| `jud --version` | The version, the manifest's. |

Nothing writes a file. A `.jud` document is written by a person or an agent, with the plugin, and checked by `jud check` before it is handed over.

## In the repository

The binary is `src/bin/jud/` behind the `cli` feature (`jud` and `http`, plus `clap` and `clap_complete`, which a library build never compiles): `main.rs` holds the command tree, from which clap derives the help, the usage errors and the completions, and sets the exit status, `run.rs` is the evaluation path, `config.rs` the resolution above, `tools.rs` the reader subcommands. `tests/jud_cli.rs` runs it as a subprocess against a wiremock server standing in for any backend, through the environment and through the configuration file, with no key and no network. `.github/workflows/release.yml` builds, checks and packages it on every platform above when a version tag is pushed, and attaches the tarballs and `SHA256SUMS` to the release the crate's publish already creates ([Releasing](releasing.md)). [Decision 0019](decisions/0019-a-command-line-for-the-format.md) says why the command, the configuration file and the release shape are what they are.
