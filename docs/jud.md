---
title: The .jud format
description: A YAML document format for a rubric with its policy, the labelled cases it is graded on and the recordings of what a model answered, with content fingerprints any implementation computes the same way; the specification, the reading rules and how the crate implements it.
status: current
last_reviewed: 2026-10-04
tags: [judgment, jud, format, specification, rubric, cases, recordings, yaml, fingerprint]
---

# The `.jud` format

A System One request is simple: a `state`, a `model` and a map of typed questions. What is hard to keep is everything around it. The questions live in code; the threshold a probability is acted on at lives in a configuration file or a constant; the labelled examples that threshold was tuned on live in a notebook; the answers the model gave last month live nowhere. When the model version moves, nobody can say which of the four changed, or whether the threshold still holds. `.jud` gives each of them a file with a stated shape, an identity and a fingerprint, so a policy can say which rubric and which cases it was tuned on, a recording can say which request and which server it answers, and two tools can exchange all three without agreeing on anything but the files.

A `.jud` file is one YAML 1.2 document. JSON is valid YAML, so a JSON file with the same fields is the same document. Three kinds of document share one envelope, and nothing in the format depends on this crate: the questions are the wire's own shape, the fingerprints are a published canonicalisation, and the [JSON Schemas](../schemas/jud/) under `schemas/jud/` state each kind formally. This page is the specification; `judgment::jud` (feature `jud`, off by default) is one implementation of it, and [`examples/jud/`](../examples/jud/) holds a complete set of documents with a runnable round trip.

## Design

Five choices shape the format; the [decision record](decisions/0014-a-file-format-for-rubrics-cases-and-recordings.md) weighs the alternatives.

- **The wire shape is the shape.** A rubric's questions are written exactly as `POST /v1/systemone` sends them: `type`, `instructions`, `criteria`. No translation layer, nothing to learn beyond the API's own documentation, and lowering a rubric into a request loses nothing.
- **Policy beside the question, never in it.** The threshold a Noul is acted on at, the confidence bar a Choice must clear and the fallback below it are the application's reading of an answer, not part of the question. They sit in `policy`, keyed by question id, with the `tuning` they came from, and are never sent to a model.
- **Identity by content.** A rubric's questions, a set of cases and a request each have a fingerprint: the SHA-256 of their canonical JSON ([RFC 8785](https://www.rfc-editor.org/rfc/rfc8785)). Any implementation computes the same fingerprint for the same content, so a `tuning.cases` of `sha256:…` means one exact set of labels whichever tool wrote it, and an edited set is visibly another one.
- **Strict reading.** A field the format does not define, a gate on a question of the wrong primitive, a label naming an option the rubric does not offer, a `from_turn` on a state that is not a conversation: each is refused with its path, never ignored or defaulted. The off-list rule the crate applies to answers applies to labels too.
- **YAML, under the 1.2 core schema.** People write and review rubrics, and comments and multi-line strings matter for that. Only `true` and `false` are booleans, so an option called `yes` is the string `yes`; a duplicate key is an error.

What the format is not: a prompt or template language (a question is data, not a program), a report format (metrics are derived from cases and recordings, and any tool prints them its own way), or a container for soft labels (a case expects one answer; a distribution over answers is left for a later version, see [Extending the format](#extending-the-format)).

## The envelope

Every document starts with two fields.

| Field | Value | Meaning |
|---|---|---|
| `jud` | `1` | The format version. A reader that does not read this version refuses the document. |
| `kind` | `rubric`, `cases` or `recording` | Which shape follows. Any other value is refused. |

The file extension is `.jud`. A reader decides what a document is by its `kind`, never by its file name, so `triage.jud`, `triage-cases.jud` and `recordings/refund.jud` are conventions, not rules.

## `rubric`

The questions a request sends, in wire shape and wire order, and the policy an application reads the answers by.

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | A name for the rubric, stable across edits, non-empty. What a cases document's `rubric` and a recording's `rubric` may name. |
| `version` | no | An edition of the rubric, free-form; a number is read as its text. The questions' fingerprint is the exact identity. |
| `description` | no | What the rubric decides, for the person reading it. |
| `questions` | yes | A map of question id to question, at least one, in the order the model sees them. |
| `policy` | no | A map of question id to gate. |
| `tuning` | no | Where the gates came from. |

### Questions

A question is exactly what the API receives, so the API's documentation is the reference for what each field means. The reader applies the checks a request builder applies before sending, and refuses a question that would be refused: a Choice needs 2 to 255 options with non-empty keys, a Score needs 2 to 10 levels none of which is null, a Noul needs instructions or criteria that describe at least one outcome, and no id is empty.

```yaml
questions:
  actionable:                 # the id: the key the answer comes back under; the model never sees it
    type: noul
    instructions: Does `message` ask for something to be done?
    criteria:                 # optional: what yes and no mean
      true: a request, a report of a fault
      false: thanks, small talk, an automated receipt
  desk:
    type: choice
    instructions: Which desk should take `message`?
    criteria:                 # option key to description (null allowed), in the order the model sees them
      billing: Invoices, payments, refunds
      technical: Errors, outages, anything about using the product
      none_of_these: Not clearly any desk
  tone:
    type: score
    instructions: How upset is the writer of `message`?
    criteria: [calm, annoyed, angry]   # levels, lowest first; a level may be an object
```

`instructions` may be a string, an object, an array or `null`; absent and `null` are the same, and both are sent to the API as `null`. A Noul's criteria keys are `true` and `false`, which YAML reads as booleans and the wire carries as strings; a reader accepts them bare or quoted and refuses any other key. The order of `questions` and of a Choice's `criteria` is significant: it is the order the model sees, and a reader preserves it.

### Policy

A gate says where one question's answer becomes an action. Every field is optional, and each applies to one primitive; a gate that does not fit its question, or names a question the rubric does not have, is refused.

| Field | Applies to | Meaning | Absent |
|---|---|---|---|
| `threshold` | Noul | Yes at this probability of yes and above. | 0.5 |
| `confidence` | Choice, Score | Acted on at this confidence and above; deferred below it. | 0, nothing deferred |
| `fallback` | Choice, Score | What a deferred answer falls back to: an offered option key, or a level by its text or its index as a string. | none |
| `note` | any | Why the bar is where it is. | |

Reading a response through the policy gives one verdict per question: for a Noul, yes or no with the probability; for a Choice, the chosen option with its confidence, or deferred with the fallback, the option the model would have chosen and the bar; for a Score, the level nearest to the weighted score (as the crate's `Score::nearest_level` reads it) with its index, text and the confidence, or deferred likewise. A verdict is derived, not stored: it is what an application does, and a `.jud` file never records what a model should have been made to do.

```yaml
policy:
  actionable:
    threshold: 0.55
    note: best F1 on the labelled cases; 0.5 lets the automated receipt through
  desk:
    confidence: 0.45
    fallback: none_of_these
  tone:
    confidence: 0.3
```

### Tuning

Where the gates came from. Every field is optional, and a rubric written by hand has none, which is itself information.

| Field | Meaning |
|---|---|
| `cases` | The cases document, by id or by fingerprint (`sha256:…`). A fingerprint says exactly which labels. |
| `model` | The versioned model that answered them (`jev-1.13.0`): a gate is tuned per version. |
| `server` | The server that answered, as a base URL. Two servers speaking the same wire answer differently. |
| `tuned_at` | When, RFC 3339. |
| anything else | Kept as given: the accuracy and coverage at the chosen bar, the sweep itself, a link to the run. `tuning` is the one place in a rubric that accepts fields the format does not name. |

### Fingerprint

A rubric's fingerprint is the [fingerprint](#fingerprints) of its `questions` map alone: the exact identity of what the model is asked, unchanged by the id, the version, the description or the policy. A recording's `rubric` or a cases document's `rubric` may name a rubric by its id, when the id is enough, or by this fingerprint, when it is not.

## `cases`

Labelled states a rubric is graded on and its gates are tuned on.

| Field | Required | Meaning |
|---|---|---|
| `id` | no | A name for the set; the fingerprint is its exact identity. |
| `rubric` | no | The rubric the labels are for, by id or by fingerprint. A reader binding the cases to a rubric checks the name against both. |
| `description` | no | Where the cases came from. |
| `cases` | yes | At least one case, in document order. |

A case:

| Field | Required | Meaning |
|---|---|---|
| `id` | no | A name, non-empty and unique in the document; a case without one is named by its position, `#3`. |
| `state` | yes | Any JSON: the state the questions are asked about. An array is a conversation, one element per turn. |
| `expect` | no | The right answer, by question id, for the questions this case is labelled for. A question left out is asked and not graded. |
| `tags` | no | Free labels for slicing a report. |
| `note` | no | Why the label is what it is. |

What `expect` holds depends on the question's primitive, and a label that does not fit is refused when the cases are bound to their rubric:

| Primitive | Label | Grades as |
|---|---|---|
| Noul | `true` or `false` | `yes` or `no` |
| Noul, over a conversation | `{from_turn: n}`: true from turn `n` (zero-based) on; `{from_turn: null}`: never | `yes` when the conversation has a turn `n`, else `no`; per turn, see below |
| Choice | an offered option key | the key |
| Score | a level's index (a number) or a level's text | the index, as a string |

The "grades as" column is the vocabulary the API's own answers are graded in: a Noul answer is `yes` at 0.5 and above, a Choice answer is its option key, a Score answer's distribution is keyed by level index. A label in that vocabulary needs no translation at grading time, and a report's confusion matrix reads in the rubric's own terms.

```yaml
cases:
  - id: refund-angry
    state:
      message: I was charged twice last month and nobody has refunded me. Fix it today or I cancel.
    expect: {actionable: true, desk: billing, tone: angry}
    tags: [billing]
  - id: receipt
    state:
      message: Your payment of 12.00 EUR was received. This is an automated message.
    expect: {actionable: false, desk: none_of_these}      # tone left unlabelled: asked, not graded
    note: the hard one; about billing, but asks nothing
```

### Conversations

A state that is an array is a conversation, and a question asked about it can change its answer from one turn to the next. `{from_turn: n}` labels a Noul with the turn at which it becomes true. Grading the whole conversation resolves the label once: `yes` if the conversation has a turn `n`. Grading turn by turn (`Case::per_turn` in the crate) cuts the state after each turn, resolves a `from_turn` label to `true` or `false` for that turn, and keeps every other label at the last turn only, since those label the whole conversation. Each turn is its own request, with its own fingerprint and its own recording.

```yaml
  - id: escalates
    state:
      - {role: user, text: Hi, my export has been stuck at 99% for an hour.}
      - {role: assistant, text: Sorry about that. Could you tell me the export id?}
      - {role: user, text: "It's exp_4411. Honestly, can I just talk to a person?"}
    expect:
      wants_human: {from_turn: 2}
```

`from_turn` is refused on a question that is not a Noul, on a state that is not an array, and with a turn past the last one (`null` says never). `{}` and `{from_turn: 0, until: 1}` are not labels.

### Fingerprint

A cases document's fingerprint is the [fingerprint](#fingerprints) of its `cases` array, as JSON: the labels and the states, with their ids, tags and notes, in order. The document's own id and description are not part of it. It is what a rubric's `tuning.cases` names.

## `recording`

One model response to one request, kept so a run can be replayed, graded again or compared with a later model's without another call.

| Field | Required | Meaning |
|---|---|---|
| `case` | yes | The case it answers, or the request hash when a recorder keyed it by content. |
| `response` | yes | The response as received: `model`, `answers` and `usage` in the wire's shape, plus `request_id` and any top-level field the server added. |
| `elapsed_ms` | yes | Wall-clock time of the call. |
| `fingerprint` | no | The [request fingerprint](#fingerprints): the identity of the state and the questions, model excluded. The key a replay finds the recording by. |
| `request_hash` | no | This crate's own 16-hex-digit content hash, written by its `Recorder`; another implementation leaves it out. |
| `rubric` | no | The rubric the questions came from, by id or by fingerprint. |
| `server` | no | The server that answered, as a base URL. |
| `recorded_at` | no | When, RFC 3339 in UTC. A `jev-latest` alias moves; the date says which version it could have been. |

```yaml
jud: 1
kind: recording
case: receipt
response:
  model: jev-1.13.0
  answers:
    actionable: {type: noul, noul: 0.52}
    desk:
      type: choice
      choice: billing
      probabilities: {account: 0.02, billing: 0.55, none_of_these: 0.4, technical: 0.03}
      confidence: 0.4
  usage: {input_tokens: 410, output_tokens: 38}
  request_id: req_01a1…
elapsed_ms: 140
fingerprint: sha256:8afe973c8409a45fc93b67678fa8954351ea4a5291b1ba58a9e6754c289890cd
rubric: inbox-triage
server: https://api.typesafe.ai
recorded_at: "2026-10-04T11:58:00Z"
```

A recording is verified against the questions of the request that finds it before it is replayed, as a live response would be, so a recording that no longer fits the rubric fails naming the question rather than replaying an answer the rubric would refuse.

## Fingerprints

A fingerprint is `sha256:` followed by the lowercase hexadecimal SHA-256 of the canonical JSON of a value. Canonical JSON is [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) (JSON Canonicalization Scheme): object members sorted by key, comparing keys as sequences of UTF-16 code units; no whitespace; strings escaped minimally (`"`, `\`, and control characters, as JSON requires, with the two-character escapes where they exist); numbers formatted as ECMAScript's `Number.prototype.toString` formats them (`1` not `1.0`, `0.1`, `1e+21`, `1e-7`). Any implementation that follows the RFC produces the same bytes, so the same content has one fingerprint everywhere.

| Fingerprint of | Over |
|---|---|
| a rubric | its `questions` map |
| a cases document | its `cases` array |
| a request | `{"questions": <the questions map>, "state": <the state>}` |

The request fingerprint leaves the model out on purpose. The model is the thing a comparison varies: the same request answered by `jev-1.13.0` and by a local model has the same fingerprint, and each recording says in `response.model`, `server` and `recorded_at` who answered it. A replay that must tell two models apart keeps their recordings in two directories.

A known vector, for an implementation to check itself against: the fingerprint of `{"b": "x", "a": 1}` is `sha256:ecf9e98ec0641e23113ff3ce8bdc78d0ddd249886517fd4a7f68cc83d4e65667`, the SHA-256 of the fifteen bytes `{"a":1,"b":"x"}`.

This crate's `Recorder` also writes its older `request_hash`, a 16-hex-digit hash of its own canonical form, and its `Replay` finds a recording by either; `request_hash` is not part of the format and another implementation neither writes nor needs it.

## Reading rules

An implementation reads a document under these rules, and the crate's [`tests/jud.rs`](../tests/jud.rs) pins that its reader and the schemas agree on each.

- YAML 1.2 core schema: `true` and `false` are the booleans; `yes`, `no`, `on`, `off` and `y` are strings; a leading-zero number is decimal. A duplicate key is an error. JSON is accepted as YAML.
- `jud` must be present and equal to 1; `kind` must be present and one of the three. Both are checked before anything else, so a document of another version or kind is refused by name.
- A field the kind does not define is refused, with its path. The exceptions are `tuning` in a rubric and `response` in a recording, which keep what they are given, and `state`, which is any JSON.
- Order is significant and preserved for `questions`, a Choice's `criteria`, a Score's `criteria`, `cases` and `expect`.
- A rubric's questions pass the request builder's checks, and each gate fits its question's primitive and names an offered option or an existing level. A cases document's labels fit their questions when the cases are bound to a rubric; a document can be read without its rubric, and is then only checked for shape.
- A `threshold` or `confidence` is a number from 0 to 1 inclusive. A level index is a non-negative integer below the number of levels.
- A reader that writes a document back writes the same fields, so a document survives a read and a write with its fingerprints unchanged.

## The loop

The format exists for one loop, which the example [`examples/jud_calibration.rs`](../examples/jud_calibration.rs) runs end to end over the documents in `examples/jud/`:

1. **Read and bind.** Parse the rubric and the cases; bind the cases to the rubric, so a label that does not fit fails before any call.
2. **Answer.** Send each case's state with the rubric's questions, to a server or to a replay of earlier recordings, and record what came back with its fingerprint.
3. **Grade.** Grade each response against the case's labels, in the answer's own vocabulary, into judgments; summarise per question into accuracy with its interval, Brier score and calibration error.
4. **Tune.** Sweep the Noul's threshold and read off the one with the best F1; table the Choice's confidence bar against accuracy and coverage and read off the lowest bar that keeps the accuracy wanted.
5. **Write back.** Put the gates into the rubric's `policy`, with the cases' fingerprint, the model, the server and the time in `tuning`, and write the rubric out. The next person to open it sees what the numbers rest on.

```sh
cargo run -p judgment --features jud --example jud_calibration
```

## Schemas and conformance

`schemas/jud/rubric.schema.json`, `cases.schema.json` and `recording.schema.json` (JSON Schema 2020-12, sharing `common.schema.json`) state the shape of each kind, and the crate's `tests/jud.rs` validates every document under `examples/jud/`, and every document the crate writes, against them. A schema cannot state the cross-checks (a gate against its question's primitive, a label against the options offered, `from_turn` against the state's shape); those are the reading rules above, and the same test pins that a document the schema refuses, the reader refuses too.

An implementation conforms when it reads all three kinds under the reading rules, refuses what they refuse with the field named, computes the fingerprints of this page (checked against the known vector), and writes documents that validate against the schemas and read back unchanged.

## In the crate

`judgment::jud`, behind the `jud` feature, is this crate's implementation: `parse` for any kind, `Rubric` with `parse`, `to_yaml`, `fingerprint`, `gate` and `apply` (verdicts through the policy), `Cases` with `parse`, `to_yaml`, `fingerprint` and `bind`, `Case::per_turn` for conversations, `grade` for one case against one response, and `parse_recording` and `recording_to_yaml` for the third kind. A rubric's `questions` is a `Questions`, so it goes straight to any `SystemOne` backend, and `Questions::handle` gives a typed handle to a question read from a file. `Replay` reads `.jud` recordings beside its `.json` ones and finds either by fingerprint. The fingerprints are `eval::canonical`, the sweeps and gate tables `eval::tuning`, and `eval::Recording` carries the recording kind's fields.

## Extending the format

`jud: 1` names exactly the fields on this page. A reader refuses a field it does not know, so a document written for a later version is refused whole rather than half-read; a later version that adds a field takes the next number, and a reader says which numbers it reads. Two additions are foreseen and deliberately not in 1: a soft label (a distribution over options or levels, for grading calibration against a panel's disagreement rather than one annotator's pick), and a verdict kind (what an application decided, for auditing a policy's record). Both wait for a second implementation to need them.
