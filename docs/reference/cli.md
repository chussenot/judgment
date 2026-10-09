---
title: The jud command line
description: Every command, flag, argument and environment variable of the jud binary, with each command's help text as the binary prints it, the verdict JSON a run writes, the report jud eval prints and its JSON keys, what jud record, jud tune and jud split write, and the exit status.
status: current
last_reviewed: 2026-10-09
tags: [judgment, jud, cli, record, eval, tune, split, reference]
---

# The jud command line

`jud` evaluates a JSON state against a `.jud` Rubric and prints one verdict per question. `jud record`, `jud eval` and `jud tune` run a Rubric over a Cases document: they keep the model's answers as recordings, grade them against the labels and propose each gate's bar. The help blocks on this page are the binary's own `--help` output, refreshed by `scripts/gen-cli-reference.sh` and checked in CI, so they cannot drift from the command tree. [Install](../start/install.md) puts the binary on your `PATH`; [Configuration](configuration.md) is where the backend comes from. Why the loop is three commands: [decision 0021](../project/decisions/0021-record-eval-and-tune-from-the-command-line.md).

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
  record      Answer every case once and write the recordings
  eval        Grade a model's answers against the labelled cases
  tune        Propose each gate's bar from recorded answers
  split       Split a Cases document into a tuning set and a held-out set
  completion  Print a shell completion script for jud's commands and flags
  help        Print this message or the help of the given subcommand(s)

Arguments:
  [RUBRIC]
          The Rubric document to evaluate; the state comes on stdin

Options:
      --replay <DIR>
          Answer from the recordings in this directory instead of a server.

          The crate's Replay backend: a recording whose request fingerprint matches this state and rubric answers, verified against the questions as a server's response would be; no key, no network. A state nobody recorded is an error, never a guess. `jud eval` and `jud tune` take their own --replay after the subcommand; `jud record` never replays, it writes recordings with --out.

          [env: JUD_REPLAY=]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Examples:
  cat state.json | jud RUBRIC.jud
  jq '.customer' event.json | jud rubric.jud
  yq -o=json '.spec' resource.yaml | jud rubric.jud

  # The loop over labelled cases: ask once, then grade and tune offline.
  jud record rubric.jud cases.jud --out recordings/
  jud eval rubric.jud cases.jud --replay recordings/
  jud tune rubric.jud cases.jud --replay recordings/
  jud split cases.jud          # cases-tune.jud and cases-holdout.jud

Exit status: 0 success; 1 a backend call failed, or a recording is missing
under --replay; 2 wrong before any call: the invocation, a file, the state or
the configuration; 3 `jud eval` only: a --min-accuracy gate was not met.
```

The state is read from stdin to the end and parsed as one JSON value ([RFC 8259](https://www.rfc-editor.org/rfc/rfc8259)); empty input, invalid JSON and a stream of several values are refused with status 2. A rubric whose Choice takes its options from the request (`options_from: request`) cannot be evaluated by `jud` and is refused with a message saying so; a case carries its own options, so [the case commands](#the-case-commands) can ask it.

`--replay` is an option of the command it follows. `jud --replay DIR eval RUBRIC CASES` is refused with status 2 and a message that names where the flag goes. The `--replay` above and `JUD_REPLAY` are those of `jud RUBRIC`; `jud eval` and `jud tune` take their own after the subcommand, and `jud record` takes none.

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
| `level` | Score | `index`, `label`, `value`, `confidence`, `band` when the gate has bands, and `reached` when the gate has `level_at_least` |
| `deferred` | Choice, Score | `fallback` (`null` when the gate names none), `nearest`, `confidence` and `bar` |

`nearest` is the option key a Choice named, or the index of the level a Score is nearest to, as a string. `bar` is the bar the answer was below, the lowest when the gate has bands. A `desk` the triage rubric defers, because the answer `billing` at 0.4 is below the bar of 0.45:

```json
"desk": {
  "verdict": "deferred",
  "fallback": "none_of_these",
  "nearest": "billing",
  "confidence": 0.4,
  "bar": 0.45
}
```

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

## The case commands

`jud record`, `jud eval` and `jud tune` take the same two documents, a `RUBRIC` and the `CASES` labelled for it, and differ in where the answers come from and what is written.

| Command | Answers come from | Writes | Needs a key |
|---|---|---|---|
| `jud record` | the configured backend, one call per request not yet recorded | recordings, under `--out DIR` | yes |
| `jud eval` | the configured backend, or the recordings under `--replay DIR` | nothing | unless `--replay` is given |
| `jud tune` | the recordings under `--replay DIR`, always | with `--out PATH`, a rubric file | no |

`record` is the only command that spends calls on purpose; `eval` spends them when it has no replay. [`jud split`](#jud-split) takes the cases alone and splits them into a tuning set and a held-out set. [Tune thresholds](../guides/tune-thresholds.md) and [Run in CI](../guides/run-in-ci.md) show them at work.

The three share these rules.

- **Documents.** Both are read as `jud check` reads them and the cases are bound to the rubric. A document that is refused, or a label that does not fit its question, is status 2 and names the file and the field.
- **Requests.** A case's request is its `state` lowered with its own `options`, so a Choice with `options_from: request` is asked over the case's options. A conversation (a `state` that is an array) with a `from_turn` label is one request per turn, the state cut after that turn, named `ID-turn-N` with N counted from 0. Every other case is one request.
- **Before any call.** Every request is lowered, and every label of a conversation's turn is checked, before any call is made or any answer is taken from a recording. A request the rubric does not lower for the case, or one that asks nothing because every `when` fails for its state, is status 2 and names the case.
- **Recordings.** Under a replay a recording answers a request by the request's fingerprint, or by its request hash, and never by its file name. It is verified against the questions as a server's response would be.
- **Missing recordings.** Under a replay every request is tried before the run fails. Status 1 then names every case that has no recording and says to record it with `jud record`. A recording that matches a request and no longer fits its questions is status 1 at that case.

## `jud record`

<!-- help: jud record -->
```text
Answer every case once and write the recordings.

Each case's request is lowered from its state and its own options and sent to the configured backend; the verified response is written to DIR/CASE.jud with the request fingerprint, the rubric, the server and the time. A conversation labelled with `from_turn` is recorded turn by turn as CASE-turn-N.jud. A request already recorded in DIR is kept and not asked again, so an interrupted run resumes.

Record always asks the configured backend and never reads `JUD_REPLAY`, so a run spends calls. --dry-run says how many, to whom and for how long, and asks nothing.

Usage: jud record [OPTIONS] --out <DIR> <RUBRIC> <CASES>

Arguments:
  <RUBRIC>
          The Rubric document the cases are for

  <CASES>
          The Cases document to answer

Options:
      --out <DIR>
          The directory the recordings are written to, created if needed

      --refresh
          Ask every case again, replacing the recordings already in DIR

      --dry-run
          Ask nothing and write nothing: print what a run would do. The requests, how many DIR already answers, which are to be asked or replaced, the backend and model it would ask, whether a key is set, and how long the run would take at the median time of the recordings in DIR

  -h, --help
          Print help (see a summary with '-h')
```

Reads `RUBRIC`, `CASES` and the recordings already in `DIR`. Asks the backend [Configuration](configuration.md#the-jud-command) resolves, so it needs a key. It takes no `--replay` and never reads `JUD_REPLAY`, though an empty `JUD_REPLAY` is refused as it is by every command ([Replay](configuration.md#replay)). Writes into `--out` and nowhere else, never over the rubric or the cases.

### What it writes

One file per request, `DIR/NAME.jud`, where `NAME` is the case's `id`, or `ID-turn-N` for a turn of a conversation. `DIR` is created when it does not exist. Each file is a [Recording](jud-format.md#recording) under a one-line comment: the verified response, the request's fingerprint, the rubric's name, the server and the time.

- **Server.** It is written without the userinfo, the query and the fragment of the base URL, so a recording can be committed.
- **Atomic.** The text goes to a temporary file in `DIR` and is renamed onto the recording's name, so a reader never sees half a file. A temporary name never ends in `.jud` or `.json`.
- **Verified.** A response that does not fit its questions is never written.

### What it asks

The requests are asked one at a time, in the cases' order. A recording is written as soon as its answer is verified.

- **Kept.** A request that a recording in `DIR` already answers is not asked again, so an interrupted run resumes and a new case costs one call.
- **Refreshed.** `--refresh` asks every request again and replaces the file named after each case. It does not read `DIR` and does not remove other files, so a stale recording under another name stays beside the new one. The `jud: warning:` that names two files recording one request is then the cue to delete the stale one.
- **Stale.** A recording of the request, in the case's own file, that no longer fits the questions is asked again and replaced.

### Refusals before any call

Status 2, before the first call:

- A case without an `id`, named by its position counted from 0: a recording is a file named after its case.
- Two requests that would share one file name, such as a case named like a turn of a conversation.
- A recording that would be written over the rubric or the cases, whether the path names the input, a symbolic link to it or a hard link. `--refresh` does not lift this.
- A stale recording of a request in a file that is not the case's own, which the run would leave beside the new one, unless `--refresh` is given. Delete or move it.
- A file in `DIR` that does not read as a recording, unless `--refresh` is given, which does not read `DIR`.
- A missing key, or a `DIR` that cannot be created.

The refusals under [the case commands](#the-case-commands) apply as well.

### What it prints

Stdout is empty. Stderr carries one line per request, then the tally. It needs a key, so what follows is plain text from a second run against a server, after one recording was deleted and another edited by hand so that it no longer fits its questions:

```text
replaced refund-angry (stale)
recorded thanks (2/7, 1 ms)
kept login-loop
kept receipt
kept close-account
kept how-to-export
kept invoice-vat
recorded 2, kept 5 in recordings
```

A line for each of `recorded NAME (N/TOTAL, MS ms)`, `kept NAME` and `replaced NAME (stale)`, then `recorded R, kept K in DIR`, where a replaced recording counts as recorded. When two files in `DIR` record one request, a `jud: warning:` line names them, since a replay answers from one of them.

A call that fails stops the run at that case with status 1. The message names the case, its position, how many were recorded and kept before it, and `DIR`. The recordings already written stay, and the next run resumes from them. Exit status: 0 when every request has a recording, written or kept; 1 when a call failed or a recording could not be written; 2 for a refusal.

### A dry run

`--dry-run` reads what a run reads, the documents bound, every request lowered and the recordings in `DIR` matched by fingerprint, and then stops: it asks nothing, creates no directory, writes no file and needs no key. It prints, on stdout:

```text
93 requests: 88 already recorded in recordings/tev1, 5 to ask (0 replacing a stale recording)
to ask: NAME, NAME, ...
stale, to replace: NAME, ...
backend http://127.0.0.1:11434, model tev1:0.8b (from config_file)
API key: set (config_file)
time: about 10.7 s a request (median of 88 recordings in recordings/tev1), about 53.6 s for 5
```

The `to ask` and `stale` lines appear only when there is a name to give. The model's source is `environment`, `config_file` or `default`; the key's is `environment` or `config_file`, or the line says it is missing and that the run would stop before the first call. The time is the median `elapsed_ms` of the recordings already in `DIR`, so it is the backend's own pace; with none it is unknown until the first request answers. With `--refresh` nothing is counted as kept. Two cases that lower to one request are counted once to ask and once kept, as a run asks once. The refusals a run makes before any call are status 2 here as in a run: a document that does not read or bind, a case without an id, an `--out` that is a file rather than a directory, and a stale recording that sits in a file the run would not write.

## `jud eval`

<!-- help: jud eval -->
```text
Grade a model's answers against the labelled cases.

Answers every case from the configured backend, or from the recordings in DIR with --replay (no key, no network), and grades each against its expected answer: per question the accuracy with its 95% interval, the Brier score, the calibration error, how often the rubric's gate acts rather than defers and how often it is right when it acts, and every miss by the model. With --min-accuracy the command exits with status 3 when a question falls short, which makes it a CI check. Against a server eval keeps nothing: use `jud record` to keep the answers.

Usage: jud eval [OPTIONS] <RUBRIC> <CASES>

Arguments:
  <RUBRIC>
          The Rubric document the cases are for

  <CASES>
          The Cases document to grade

Options:
      --replay <DIR>
          Answer from the recordings in this directory instead of a server

          [env: JUD_REPLAY=]

      --json
          Print the report as one JSON object on stdout instead of text

      --min-accuracy <[QUESTION=]ACCURACY>
          Fail with status 3 when accuracy is below this, 0 to 1: `0.9` for every labelled question, `desk=0.95` for one. Repeatable. It is the model's accuracy per question, not the policy's. A question with no labelled case does not meet its bar

  -h, --help
          Print help (see a summary with '-h')
```

Reads `RUBRIC`, `CASES` and, with `--replay DIR` or `JUD_REPLAY`, the recordings in `DIR`; the flag overrides the variable. Writes nothing, against a server too: use [`jud record`](#jud-record) to keep what a server answered. Without a replay it asks the configured backend once per request and needs a key. With one it needs no key and no network.

Stdout carries the report, as text or, with `--json`, as one JSON object. Both come from one value, so they say the same things. Status 0 whatever the accuracy, unless a [gate](#gating-on-accuracy) is given.

### The text report

The report, with the variable parts in capitals. [Tune thresholds](../guides/tune-thresholds.md) shows one for the triage documents.

```text
rubric NAME: questions FINGERPRINT, policy FINGERPRINT
cases NAME: FINGERPRINT, N cases
N requests, model MODEL

QUESTION
  labelled L, correct C, accuracy A (95% interval LOW to HIGH)
  brier B, calibration error E, confidence when right R, when wrong W
  outcomes, labelled/answered/right: OUTCOME L/A/R, OUTCOME L/A/R
  majority OUTCOME, M of L (SHARE)
  gate: acts on A of T, defers D, accuracy when acted X
  warning: SIGNAL

model misses (N)
  QUESTION  CASE: expected E, predicted P, confidence C

--min-accuracy
  met      QUESTION A >= BAR
  NOT MET  QUESTION A < BAR
```

There is one block per question, in the rubric's order. When several models answered, `model MODEL` reads `models MODEL, MODEL`. A question no case labels prints `not labelled` in place of its metrics, outcomes and majority lines, a question without a gate prints `gate: none`, and a run with no miss prints `model misses: none`. A `warning:` line appears per signal raised, none when there is none. The `--min-accuracy` section appears only with a gate flag.

A report holds three things that are not the same number: what the model got right, what the policy does with its answers, and where the model was wrong.

- **The model.** `labelled`, `correct`, `accuracy` with its 95 % interval, the Brier score, the calibration error and the mean confidence when right and when wrong, each `-` when there is none. They read the model's own answer: a Noul is yes from a probability of 0.5, a Choice is the option it picked, a Score is its most probable level. [The metrics](../concepts/rubrics-cases-recordings.md#the-metrics) defines them.
- **The gate.** `acts on A of T, defers D` counts what the rubric's gate for the question does with the T answers that asked it. A Noul's gate never defers, so it acts on every answer. `accuracy when acted` is the accuracy of the policy's own verdicts among the answers the gate acted on and a case labels. A Noul is read at the gate's `threshold` and `strict`, a Choice by the option the verdict names, a Score by the level the policy reads, the one nearest the weighted score. It is not the question's `accuracy`, and moving a bar moves one and not the other.
- **The outcomes.** For every answer the question offers, in its order (`yes` and `no`, a Choice's options, a Score's levels by their text), how many labels name it, how often the model gave it on a labelled case, and how often rightly. Each count is over the labelled answers, so the labels and the answers each add up to `labelled`. An outcome no case labels shows `0/…`. Options supplied per request follow, as the answers name them.
- **The majority.** The commonest label, the first in the question's order on a tie, and its share of `labelled`: what always giving that one answer would score.
- **The warnings.** One line per signal, each read from the numbers above, and none from fewer than ten answers. The thresholds are in the JSON report's `signal_rules`; a change to one is a change of meaning ([stability](stability.md)).
  - `not shown to beat always answering OUTCOME (SHARE): the interval reaches down to LOW` when the accuracy's 95 % interval reaches down to the majority's share, and the labels name more than one outcome.
  - `collapsed: the model answered OUTCOME on N of L labelled cases; …` when one answer takes at least 0.8 of the labelled answers and at least 0.2 more than the largest share any label has.
  - `never answered: OUTCOME (N labelled), …; …` when an outcome at least 3 labels name is never the model's answer on a labelled case. A skewed set hides this from `collapsed`: a model that always says `yes` to a set that is 88 % `yes` answers one outcome only 0.12 more often than the labels name it.
  - `the gate defers D of T: they fall back to FALLBACK` (or `nothing acts on them` without a fallback) when the gate defers at least 0.5 of the answers it sees (`defers_most`) or at least 0.9 (`defers_nearly_all`); one of the two, never both.
- **The misses.** `model misses` lists the model's own readings that the labels call wrong, one line per miss: question, case, expected, predicted and the model's confidence in what it predicted, which for a Noul is the larger of p and 1 - p. A Noul is `yes` or `no`, a Choice is an option key and a Score is a level index. A miss is not the policy's: a Noul answered 0.52 is a miss against a `false` label, and a gate at 0.55 says no and is right.

### The JSON report

`--json` prints one object. Key names are `snake_case`; they and the types below are part of [the command's contract](stability.md#the-crate-and-the-command). Numbers are unrounded. A value that cannot be computed is `null`, never a string.

| Key | Type | Meaning |
|---|---|---|
| `rubric` | object | The rubric the cases were graded against. |
| `rubric.name` | string | Its `metadata.name`. |
| `rubric.fingerprint` | string | `sha256:` over its questions. |
| `rubric.policy_fingerprint` | string | `sha256:` over its policy, so a moved bar is as visible as a changed question. |
| `cases` | object | The cases that were asked. |
| `cases.name` | string | Its `metadata.name`. |
| `cases.fingerprint` | string | `sha256:` over its cases. |
| `cases.count` | integer | Cases in the document, not requests. |
| `requests` | integer | Requests answered: one per case, one per turn for a conversation labelled with `from_turn`. |
| `models` | array of strings | The distinct `model` the responses named, in the order first seen. |
| `questions` | array of objects | One per question of the rubric, in its order. |
| `questions[].id` | string | The question's id. |
| `questions[].labelled` | integer | Answers a case labels. |
| `questions[].correct` | integer | Labelled answers the model got right. |
| `questions[].accuracy` | number or `null` | `correct` over `labelled`; `null` when nothing is labelled. |
| `questions[].accuracy_interval95` | array of two numbers or `null` | The 95 % Wilson interval, low then high. |
| `questions[].brier` | number or `null` | The mean Brier score. |
| `questions[].ece` | number or `null` | The expected calibration error, over ten bins. |
| `questions[].confidence_when_right` | number or `null` | `null` when no labelled answer was right. |
| `questions[].confidence_when_wrong` | number or `null` | `null` when no labelled answer was wrong. |
| `questions[].gate` | object or `null` | The policy's gate at work; `null` when the policy has none for the question. |
| `questions[].gate.acted` | integer | Answers the gate acted on. |
| `questions[].gate.deferred` | integer | Answers the gate deferred. |
| `questions[].gate.accuracy_when_acted` | number or `null` | The policy's accuracy among the acted answers a case labels; `null` when there are none. |
| `questions[].misses` | array of objects | The model's misses; `[]` when there are none. |
| `questions[].misses[].case` | string | The case's id, `#N` for a case without one (its position, counted from 0), or `ID-turn-N`. |
| `questions[].misses[].expected` | string | The label, in the answer's words. |
| `questions[].misses[].predicted` | string | The model's reading, in the same words. |
| `questions[].misses[].confidence` | number | The model's confidence in `predicted`. |
| `questions[].outcomes` | array of objects | One per outcome the question offers, in its order, then any other the labels or answers name. |
| `questions[].outcomes[].outcome` | string | In the label's words: `yes` or `no`, an option key, a level's index. |
| `questions[].outcomes[].level` | string or `null` | A Score level's text; `null` for a Noul or a Choice. |
| `questions[].outcomes[].labelled` | integer | Labels that name it. |
| `questions[].outcomes[].predicted` | integer | Labelled answers in which the model gave it. |
| `questions[].outcomes[].correct` | integer | Of those, the right ones. |
| `questions[].majority` | object or `null` | The commonest label; `null` when nothing is labelled. |
| `questions[].majority.outcome` | string | In the label's words. |
| `questions[].majority.level` | string or `null` | A Score level's text. |
| `questions[].majority.labelled` | integer | Labels that name it. |
| `questions[].majority.share` | number | `labelled` over the question's `labelled`. |
| `questions[].signals` | array of strings | In this order, each when raised: `no_better_than_majority`, `collapsed`, `never_answered`, then `defers_most` or `defers_nearly_all`, as [the text report](#the-text-report) defines them; `[]` when none is. |
| `min_accuracy` | array of objects | One entry per question a `--min-accuracy` flag checks; `[]` without the flag. |
| `min_accuracy[].question` | string or `null` | The question; `null` when a bare bar found no question with a label. |
| `min_accuracy[].bar` | number | The bar as given. |
| `min_accuracy[].labelled` | integer | Labelled answers of the question; 0 when it has none. |
| `min_accuracy[].accuracy` | number or `null` | The question's `accuracy`. |
| `min_accuracy[].met` | boolean | `false` when `accuracy` is below the bar or there is none. |
| `signal_rules` | object | The thresholds the signals were read with, so two reports read under different ones can be told apart. |
| `signal_rules.min_answers` | integer | The fewest answers any signal is read from: 10. |
| `signal_rules.collapse_share` | number | The share one answer must take for `collapsed`: 0.8. |
| `signal_rules.collapse_margin` | number | How far above the largest label share it must be: 0.2. |
| `signal_rules.never_answered_min` | integer | The fewest labels an outcome needs for `never_answered`: 3. |
| `signal_rules.defers_most` | number | The deferred share for `defers_most`: 0.5. |
| `signal_rules.defers_nearly_all` | number | The deferred share for `defers_nearly_all`: 0.9. |

The same report with one question kept and `--min-accuracy desk=0.9`:

```json
{
  "rubric": {
    "name": "inbox-triage",
    "fingerprint": "sha256:fda347c2cba6deaa8ad784360c46a8289adbc0284b373c3d9162ce11f3e7ac8a",
    "policy_fingerprint": "sha256:1585094f59711426a98044a00ded2ec747010b5369e941f33a2732a50262e99f"
  },
  "cases": {
    "name": "inbox-triage-cases",
    "fingerprint": "sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6",
    "count": 7
  },
  "requests": 7,
  "models": [
    "jev-1.13.0"
  ],
  "questions": [
    {
      "id": "desk",
      "labelled": 7,
      "correct": 6,
      "accuracy": 0.8571428571428571,
      "accuracy_interval95": [
        0.48686549668097007,
        0.9743210440510253
      ],
      "brier": 0.15471428571428572,
      "ece": 0.15142857142857136,
      "confidence_when_right": 0.7566666666666668,
      "confidence_when_wrong": 0.4,
      "gate": {
        "acted": 6,
        "deferred": 1,
        "accuracy_when_acted": 1.0
      },
      "misses": [
        {
          "case": "receipt",
          "expected": "none_of_these",
          "predicted": "billing",
          "confidence": 0.4
        }
      ],
      "outcomes": [
        {
          "outcome": "billing",
          "level": null,
          "labelled": 2,
          "predicted": 3,
          "correct": 2
        },
        {
          "outcome": "technical",
          "level": null,
          "labelled": 2,
          "predicted": 2,
          "correct": 2
        },
        {
          "outcome": "account",
          "level": null,
          "labelled": 1,
          "predicted": 1,
          "correct": 1
        },
        {
          "outcome": "none_of_these",
          "level": null,
          "labelled": 2,
          "predicted": 1,
          "correct": 1
        }
      ],
      "majority": {
        "outcome": "billing",
        "level": null,
        "labelled": 2,
        "share": 0.2857142857142857
      },
      "signals": []
    }
  ],
  "min_accuracy": [
    {
      "question": "desk",
      "bar": 0.9,
      "labelled": 7,
      "accuracy": 0.8571428571428571,
      "met": false
    }
  ],
  "signal_rules": {
    "min_answers": 10,
    "collapse_share": 0.8,
    "collapse_margin": 0.2,
    "never_answered_min": 3,
    "defers_most": 0.5,
    "defers_nearly_all": 0.9
  }
}
```

### Gating on accuracy

`--min-accuracy [QUESTION=]ACCURACY`, repeatable, makes the command exit with status 3 when a question's accuracy is below its bar.

- **The bar.** A number from 0 to 1, else status 2. A bare `0.9` applies to every question that has a labelled case. `desk=0.95` applies to the question `desk`; a name the rubric does not ask is status 2, and the message lists the questions.
- **What is compared.** The question's `accuracy`, the model's, as the report prints it, and never `accuracy when acted`. The comparison is on the unrounded value, and the text shows as many digits as it takes to read true: `desk 0.857 < 0.86`.
- **No labels.** A question with no labelled case does not meet its bar, named or not. A bare bar when no question has a label is one check that fails, so a gate cannot pass because nothing was measured.
- **The result.** The report is complete on stdout first. Then a line on stderr, `jud: N question(s) below --min-accuracy: desk 0.86 < 0.95`, and status 3. A reader that closes stdout early does not change the status.

## `jud tune`

<!-- help: jud tune -->
```text
Propose each gate's bar from recorded answers.

Reads the recordings in DIR (it never calls a backend), grades them against the cases, sweeps each gate and prints a proposal: a Noul's threshold by best F1, a Choice's or Score's confidence bar as the lowest that keeps --target-accuracy over at least --min-covered cases, a Score's `level_at_least` by best F1. The tables go to stderr. Stdout carries the proposed policy and tuning blocks as YAML, starting at column 0: paste them under `spec:`, indented two spaces. --out writes the whole rubric with the proposal applied instead.

Usage: jud tune [OPTIONS] --replay <DIR> <RUBRIC> <CASES>

Arguments:
  <RUBRIC>
          The Rubric document whose gates are tuned

  <CASES>
          The Cases document the gates are tuned on

Options:
      --replay <DIR>
          The recordings to tune from; `jud record` writes them

          [env: JUD_REPLAY=]

      --target-accuracy <ACCURACY>
          The accuracy a Choice's or Score's confidence bar must keep, 0 to 1

          [default: 0.95]

      --min-covered <N>
          The fewest cases a confidence bar must still cover

          [default: 3]

      --out <PATH>
          Write the whole rubric with the proposal applied to this file, instead of printing the blocks. The rubric is serialised again: its comments and layout are lost and `version` is written as text. Never the rubric, the cases or a file in DIR

  -h, --help
          Print help (see a summary with '-h')
```

Reads `RUBRIC`, `CASES` and the recordings in `--replay DIR`, which is required as the flag or as `JUD_REPLAY`. It never calls a backend and needs no key. It grades the recordings against the labels as `jud eval` does and sweeps each gate.

### What it proposes

| Question | Gate field | Rule |
|---|---|---|
| Noul | `threshold` | The threshold with the best F1, over 0.05 to 0.95 in steps of 0.05. A tie goes to the lower. |
| Choice, Score | `confidence` | The lowest bar, over 0 to 0.95 in steps of 0.05, whose answers at or above it are right at least `--target-accuracy` of the time over at least `--min-covered` answers. |
| Score with `level_at_least` | `level_at_least` | The level with the best F1 for "this level or higher". A tie goes to the lower. Written as the gate named it: an index stays an index, a text becomes the level's text. |

`--target-accuracy` is a number from 0 to 1, default 0.95. `--min-covered` is at least 1, default 3. Anything else is status 2, before the documents are read.

A proposal changes the field it proposes and the gate's `note`, which says how the number was found. Everything else in the gate is kept as written: `fallback`, `strict`, the other fields. A gate stays as written, with a line on stderr saying why, when:

- **Bands.** The gate has `bands`. The table is printed and nothing is proposed.
- **No bar.** No bar reaches the target over enough answers, or a Noul has no F1 because no case is labelled yes, or no answer reaches yes.
- **No gate.** The policy has none for the question. The question is skipped.
- **No label.** No case labels the question. It is skipped.

### On stderr

Tables and notes go to stderr. A question's header reads `jud: ID (KIND): L labelled cases, accuracy A (95% interval LO-HI)`, followed by its table. The row of the proposal carries a `proposed` marker at its end.

| Table | Columns | Rows |
|---|---|---|
| Noul threshold | `threshold`, `accuracy`, `precision`, `recall`, `f1` | 0.05 to 0.95 |
| Choice or Score bar | `bar`, `covered`, `coverage`, `correct`, `accuracy` | 0 to 0.95 |
| Score level | `level`, `accuracy`, `precision`, `recall`, `f1` | level 1 up |

`covered` counts the answers at or above the bar, `coverage` is that over the labelled answers, and `correct` and `accuracy` are among the covered. A line `jud: ID: propose threshold 0.55 (now 0.55): NOTE` follows, with `confidence` or `level_at_least` for the others. `now` is the gate's current value, or `unset` when it has none.

[Tune thresholds](../guides/tune-thresholds.md) shows the notes for the triage documents.

Warnings, none of which changes the status:

- **Thin data.** The 95 % interval is wider than 0.2, so the bar is a guess with a number on it.
- **A bar of 0.** The gate would defer nothing.
- **A Score's bar table.** Printed for every Score whose `confidence` bar is proposed. The table reads the most probable level as the answer, where the policy reads the level nearest the weighted score. The level table reads the nearest, as the policy does.
- **A strict gate.** The sweep counts an answer at the bar as acting, and a strict gate acts above it. The line says how many answers sit exactly at the proposed bar when any do, and otherwise that one can.
- **Duplicates.** Two files in `DIR` record one request, so a replay answers from one of them.

### On stdout

The proposed `policy` and `tuning` blocks, as YAML, starting at column 0. Indent them two spaces to paste them under `spec:`. The `policy` block holds every gate of the rubric in its order, the proposed fields changed. The `tuning` block is the provenance:

| Key | Value |
|---|---|
| `cases` | The fingerprint of the cases document. |
| `model` | The one model that answered the recordings. |
| `server` | The server the recordings name, without credentials, query or fragment; left out when none names one or they disagree. |
| `tuned_at` | The time of the run, [RFC 3339](https://www.rfc-editor.org/rfc/rfc3339) in UTC. |
| `labelled` | A map of question id to the labelled answers its proposal was read from; only the questions that were proposed. |

[Tune thresholds](../guides/tune-thresholds.md) shows the blocks for the triage documents.

When nothing is proposed, there is no block and no file: a line on stderr says so, and the status is 0.

### With `--out`

`--out PATH` writes the whole rubric with the proposal and the `tuning` block applied. Stdout stays empty and stderr says `jud: wrote the tuned rubric to PATH`. A file already at `PATH` is replaced. The rubric is serialised again, so its comments and layout are lost and `version` is written as text. `jud check` reads the result.

`--out` is refused with status 2, before the documents are read, when `PATH`:

- **Is an input.** It is the rubric or the cases, however spelled: a `./` path, a symbolic link or a hard link.
- **Is among the recordings.** It is in `DIR`, is one of its recordings under another name, or is a link whose write would land there. A replay reads every `.jud` and `.json` in `DIR` as a recording.

Without `--out`, `jud tune` writes nothing.

### Refusals and failures

Status 2: the settings above; a rubric with no gates, since there is nothing to tune; recordings that come from more than one model, since a bar is tuned per model, and the message names which cases each model answered; a `DIR` that cannot be read or holds a file that is not a recording; and the refusals under [the case commands](#the-case-commands). Status 1: a request with no recording, or one whose recording no longer fits.

## `jud split`

<!-- help: jud split -->
```text
Split a Cases document into a tuning set and a held-out set.

Every Nth case (the Nth, the 2Nth, ...) goes to NAME-holdout.jud, the others to NAME-tune.jud, beside CASES or in --out. Cases are copied as read, so recordings made over CASES answer both halves. Prints each half's count, fingerprint and labels per question, and warns about a label one half has and the other lacks. With --rubric, the labels are read as the rubric reads them (a Score's level by its text whether written as text or index), checked against it, and listed in its order. Refuses to write over an existing file.

Usage: jud split [OPTIONS] <CASES>

Arguments:
  <CASES>
          The Cases document to split

Options:
      --every <N>
          Hold out every Nth case, 2 or more: 4 holds out a quarter

          [default: 4]

      --out <DIR>
          The directory the two halves are written to; CASES's own by default

      --rubric <RUBRIC>
          The Rubric the cases are for: read the labels as it reads them, so a Score level written as `0` and as `calm` is one level

  -h, --help
          Print help (see a summary with '-h')
```

Reads `CASES` and writes two new files, `STEM-tune.jud` and `STEM-holdout.jud`, beside it or under `--out` (created when needed), where `STEM` is the name of the `CASES` file without its extension. The Nth case, the 2Nth and so on go to the holdout half, the others to the tune half, in their order. Each case is copied as read, state, labels, options, tags and note (its values, not its bytes: comments go, and an anchor is written out), so a recording that answers a case in the whole set answers it in its half, and a split never needs a recording again. The halves are named `NAME-tune` and `NAME-holdout` after the set's `metadata.name`, keep its `spec.rubric`, labels and annotations, and say in `metadata.description` how they were made; each starts with a one-line comment naming its source. A half's fingerprint is over its own cases.

Stdout gives each half's path, name, count and fingerprint, then per question the labels it carries and how many cases carry each. A conversation's `from_turn` label counts as the turns it labels: `true` from that turn on, `false` before it, or `false` throughout for `null`. With `--rubric`, the labels are read as the rubric reads them and listed in its order: a Score level is its text, whether the label wrote the text or the index. Without it, a label is its own word, and a note on stderr says when a level is named by its index. A label one half has and the other lacks is a `jud: warning:` line on stderr, since a held-out number on an outcome the tuning set never saw says little. Status 2, before anything is written: a document that does not read, `--every` under 2, fewer cases than `--every` (nothing would be held out), cases that do not fit the `--rubric` given, a half that would be written over the input, or a half's file that already exists. The two halves are written together or not at all: when the second cannot be written, the first is removed. `jud split` asks no backend and needs no key.

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
| `TYPESAFE_API_KEY` | `jud config`, `jud record`, and `jud RUBRIC` and `jud eval` when they have no replay | The key sent as the bearer; overrides `api_key` in the file |
| `TYPESAFE_BASE_URL` | the same commands | The server; overrides `base_url` in the file |
| `JUD_REPLAY` | the commands that take `--replay` ([which ones](configuration.md#replay)) | A directory of recordings to answer from instead of a server; `--replay` overrides it |
| `XDG_CONFIG_HOME` | the same commands | Moves the configuration file's directory |

[Configuration](configuration.md) has the precedence, the file and what an empty value means.

## Exit status

| Status | Meaning |
|---|---|
| 0 | Success. Verdicts printed; recordings written or kept; the report printed with every `--min-accuracy` bar met, or none given; a proposal printed, written, or none to make. |
| 1 | The backend was asked and the call failed: a transport error after the retries, an HTTP error, a response that does not fit the rubric. Under a replay, no recording for a request, or a recording that no longer fits its questions. For `jud record`, a recording that could not be written. |
| 2 | Wrong before any call, and fixable: no rubric argument, a file that cannot be read, a document that is not valid, cases that do not fit their rubric, stdin that is empty or not one JSON value, a missing API key, a malformed configuration file, an unknown flag or a flag with a bad value, an `--out` that would overwrite an input, a `--min-accuracy` that names no question; for `check`, any document refused. |
| 3 | `jud eval` only. The evaluation ran, the report is on stdout, and a `--min-accuracy` bar was not met. |

Every failure goes to stderr as `jud: MESSAGE`. A usage error that clap catches prints clap's own `error:` text instead, with status 2. `jud check` is the exception: its refusals are `error` lines in its result on stdout. Stdout carries the command's result and nothing else: the verdicts of a run, the lines of `jud check` and `jud lower`, the JSON of `jud config`, `jud eval`'s report, `jud tune`'s blocks and the completion script. `jud record` prints nothing on stdout but its `--dry-run` plan, and `jud split` prints its report there. Everything else is a diagnostic and goes to stderr: the progress and tally of `jud record`, the tables and notes of `jud tune`, and the line that says a bar was not met. A pipeline never reads an error as a result. A reader that closes stdout early (`jud check FILE | head -1`) is not an error for any command: nothing panics, and the status is the command's own.

Three commands write files, and nothing else does. `jud record` writes recordings into its `--out` directory. `jud tune --out` writes one rubric file. `jud split` writes two new cases files, and refuses when either exists. None writes over a rubric or a cases document it was given, and `jud tune --out` never writes among the recordings. Every other command, `jud eval` and a plain run among them, writes no file.
