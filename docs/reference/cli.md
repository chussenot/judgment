---
title: The jud command line
description: Every command, flag, argument and environment variable of the jud binary, with each command's help text as the binary prints it, the verdict JSON it writes, and the exit status.
status: current
last_reviewed: 2026-10-07
tags: [judgment, jud, cli, reference]
---

# The jud command line

`jud` evaluates a JSON state against a `.jud` Rubric and prints one verdict per question. The help blocks on this page are the binary's own `--help` output, refreshed by `scripts/gen-cli-reference.sh` and checked in CI, so they cannot drift from the command tree. [Install](../start/install.md) puts the binary on your `PATH`; [Configuration](configuration.md) is where the backend comes from.

## `jud [OPTIONS] [RUBRIC]`

<!-- help: jud -->
```text
Evaluate JSON input against a .jud Rubric and print the verdicts.

The rubric file holds the questions and the policy (kind: Rubric); stdin holds the JSON state the questions are asked about. The configured System One backend answers (TypeSafe by default), the policy reads the answers, and one verdict per question is printed as JSON on stdout.

Usage: jud [OPTIONS] [RUBRIC]
       jud <COMMAND>

Commands:
  config      The backend a run would use: base URL, model, timeout, whether an API key is set
  check       Read documents as the crate reads them
  lower       Print the request a rubric lowers to, for a state or for every case
  completion  Print a shell completion script for jud's commands and flags
  help        Print this message or the help of the given subcommand(s)

Arguments:
  [RUBRIC]
          The Rubric document to evaluate; the state comes on stdin

Options:
      --replay <DIR>
          Answer from the recordings in this directory instead of a server.

          The crate's Replay backend: a recording whose request fingerprint matches this state and rubric answers, verified against the questions as a server's response would be; no key, no network. A state nobody recorded is an error, never a guess.

          [env: JUD_REPLAY=]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Examples:
  cat state.json | jud RUBRIC.jud
  jq '.customer' event.json | jud rubric.jud
  yq -o=json '.spec' resource.yaml | jud rubric.jud

Exit status: 0 verdicts printed; 1 the backend call failed; 2 the
invocation, a file, the state or the configuration is wrong.
```

The state is read from stdin to the end and parsed as one JSON value ([RFC 8259](https://www.rfc-editor.org/rfc/rfc8259)); empty input, invalid JSON and a stream of several values are refused with status 2. A rubric whose Choice takes its options from the request (`options_from: request`) cannot be evaluated by `jud` and is refused with a message saying so.

### Verdicts

Stdout carries a JSON object keyed by question id, in the rubric's order, the serialisation of `judgment::jud::Verdict`:

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

| `verdict` | Primitive | Fields |
|---|---|---|
| `yes`, `no` | Noul | `probability` |
| `option` | Choice | `key`, `confidence`, and `band` when the gate has bands |
| `level` | Score | `index`, `label`, `value`, `confidence`, and `reached` when the gate has `level_at_least` |
| `deferred` | Choice, Score | `fallback`, the nearest option or level, and the bar it was below |

## `jud config`

<!-- help: jud config -->
```text
The backend a run would use: base URL, model, timeout, whether an API key is set.

Resolved from TYPESAFE_API_KEY and TYPESAFE_BASE_URL, then ~/.config/jud/config.yaml, then the defaults.

Usage: jud config

Options:
  -h, --help
          Print help (see a summary with '-h')
```

The output is in [Configuration](configuration.md#jud-config).

## `jud check`

<!-- help: jud check -->
```text
Read documents as the crate reads them.

Cases are bound to their rubric among the files, recordings are verified against the request they answer; status 2 when any document is refused.

Usage: jud check <FILE>...

Arguments:
  <FILE>...
          The .jud documents to read

Options:
  -h, --help
          Print help (see a summary with '-h')
```

One line per document with its kind, path, name, `apiVersion` and what was checked; indented lines with its fingerprints; `error` lines naming the file and the field; a summary. A cases document is bound to the rubric it names among the files; a recording whose `metadata.name` names a case of a bound document is verified against that case's request and its fingerprint compared.

## `jud lower`

<!-- help: jud lower -->
```text
Print the request a rubric lowers to, for a state or for every case

Usage: jud lower [OPTIONS] <RUBRIC>

Arguments:
  <RUBRIC>  The Rubric document to lower

Options:
      --state <JSON>       The JSON state, inline
      --state-file <PATH>  The JSON state, read from a file
      --options <JSON>     Supplied options for `options_from: request` questions, as a JSON object of question key to option key to text
      --cases <FILE>       A Cases document: lower every case it holds against the rubric
  -h, --help               Print help
```

Prints the request the rubric lowers to, as the `questions` map the wire carries, for one state or for every case of a cases document.

## `jud completion`

<!-- help: jud completion -->
```text
Print a shell completion script for jud's commands and flags.

Generated from the same command tree clap parses, so it cannot drift from the binary. Writes to stdout; docs/start/install.md says where each shell wants it.

Usage: jud completion <SHELL>

Arguments:
  <SHELL>
          The shell whose syntax to emit

          [possible values: bash, elvish, fish, powershell, zsh]

Options:
  -h, --help
          Print help (see a summary with '-h')
```

[Shell completion](../start/install.md#shell-completion) says where each shell wants the script.

## Environment

| Variable | Read by | Meaning |
|---|---|---|
| `TYPESAFE_API_KEY` | a run | The key sent as the bearer; overrides `api_key` in the file |
| `TYPESAFE_BASE_URL` | a run | The server; overrides `base_url` in the file |
| `JUD_REPLAY` | a run | A directory of recordings to answer from instead of a server; `--replay` overrides it |
| `XDG_CONFIG_HOME` | every command | Moves the configuration file's directory |

[Configuration](configuration.md) has the precedence and the file.

## Exit status

| Status | Meaning |
|---|---|
| 0 | Verdicts printed on stdout. |
| 1 | The backend was asked and the call failed: a transport error after the retries, an HTTP error, a response that does not fit the rubric, or no recording for the state under `--replay`. |
| 2 | Something fixable before any call: no rubric argument, a file that cannot be read, a document that is not a valid Rubric, stdin that is empty or not one JSON value, a missing API key, a malformed configuration file; for `check`, any document refused. |

Every error goes to stderr, prefixed `jud:`; stdout carries verdicts and nothing else, so a pipeline never reads an error as a result. Nothing writes a file.
