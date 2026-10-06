# The .jud format, version 1.3: field reference

Condensed from `docs/jud.md`, which is the specification and wins on any
difference. Three kinds of YAML document share one envelope. `jud check`
(`scripts/jud.sh check`) applies every rule here and names the field it refuses.

## Contents

1. Envelope: `apiVersion`, `kind`, `metadata`, `spec`
2. YAML rules the reader enforces
3. `Rubric`: questions, declarations, policy, tuning
4. `Cases`: a case, labels per primitive, conversations, options
5. `Recording`
6. Fingerprints
7. What the reader refuses (checklist)

## 1. Envelope: `apiVersion`, `kind`, `metadata`, `spec`

The envelope is a manifest's, as a Kubernetes object or a Backstage catalog
entry is written. Four top-level keys, and no other:

```yaml
apiVersion: jud/v1.3        # a string, exactly this; the only version the reader reads
kind: Rubric                # Rubric | Cases | Recording, PascalCase
metadata:
  name: inbox-triage        # required on every kind; a name, never a path
  version: 2                # Rubric only; free-form edition, a number is read as its text
  description: ...          # Rubric and Cases
  labels: {team: support}   # string -> string
  annotations: {k: v}       # string -> string; what the format does not name
spec:
  ...                       # the kind's fields, sections 3 to 5
```

| Field | Required | Rule |
|---|---|---|
| `apiVersion` | yes | The string `jud/v1.3`. Any other value, a number, or a missing key is refused by name. A later format takes `jud/v1.4`; a reader reads one. |
| `kind` | yes | `Rubric`, `Cases` or `Recording`; the case is part of the value. |
| `metadata.name` | yes | A name: what `spec.rubric` and a file name refer to; for a Recording, the case it answers. |
| `metadata.version` | no | Rubric only. A free-form edition (`2`, `2026-10`); the questions' fingerprint is the exact identity. Refused on Cases and Recording. |
| `metadata.description` | no | Rubric and Cases: what it decides, where the cases came from. Refused on a Recording. |
| `metadata.labels` | no | Map of string to string. |
| `metadata.annotations` | no | Map of string to string: where a tool keeps what the format does not name (an editor's layout, a labelling tool's provenance, a shared YAML anchor). Not validated, part of no fingerprint. |
| `spec` | yes | Everything the kind holds. A field the kind does not define is refused with its path. |

A **name** is ASCII letters, digits, `.`, `_` and `-`, starting with a letter
or a digit: `inbox-triage`, `refund-angry`, `T-98423`. No `/`, `\`, space,
leading `.`, and not empty. `metadata.name` and a case's `id` are names. A
name is never a path, so a tool can name a file after it and never leave its
directory.

A writer emits the keys in that order: `apiVersion`, `kind`, `metadata`
(`name`, then `version`, `description`, `labels`, `annotations` when
present), `spec`. The field names inside `spec` are the wire's and the
format's, in snake_case.

## 2. YAML rules the reader enforces

- YAML 1.2 core schema: only `true` and `false` are booleans. `yes`, `no`,
  `on`, `off`, `y` are **strings**, so an option key `yes` is fine and a Noul
  label must be the bare `true` or `false`.
- A duplicate key is an error.
- A merge key (`<<: *anchor`) is refused. A plain anchor and alias
  (`&rule` / `*rule`) on a scalar or a map value is fine; the examples share
  a sentence between questions that way, with the anchor on an annotation.
- An explicit tag (`!secret`, `!!python/...`) is refused. `!!binary` stays
  the literal text, never decoded.
- JSON is accepted as YAML; a `.jud` can be JSON.
- Unknown fields are refused with their path, except: `spec.tuning` and a
  recording's `spec.response` keep anything; `state` is any JSON;
  `metadata.annotations` and `metadata.labels` hold any string under any key.
- Order is kept and matters for `questions`, a Choice's `criteria`, a Score's
  `criteria`, `bands`, `cases`, `expect` and a case's `options`: it is the
  order the model sees. The parts of an instructions object carry no order.

## 3. `Rubric`

`metadata.name` is the rubric's id, stable across edits: what `spec.rubric`
of a Cases or Recording document refers to. `metadata.version` and
`metadata.description` are its edition and its purpose. Under `spec`:

| Field | Required | Meaning |
|---|---|---|
| `questions` | yes | Map of question id to question, at least one, in the order the model sees them. |
| `policy` | no | Map of question id to gate. |
| `tuning` | no | Where the gates came from. Any fields; `cases`, `model`, `server`, `tuned_at` have a meaning. |

### Questions

A question is exactly what the wire sends. The id is the key the answer comes
back under; the model never sees it. `instructions` is a string, an object
(named parts; sent as an object), an array, or absent. For a Choice the
model sees the option keys and their descriptions, in order.

```yaml
spec:
  questions:
    actionable:                         # Noul: a yes/no, answered with P(yes)
      type: noul
      instructions: Does `message` ask for something to be done?
      criteria:                         # optional; keys are exactly true and false
        true: a request, a report of a fault
        false: thanks, small talk, an automated receipt
    desk:                               # Choice: one option, answered with a
      type: choice                      #   probability per option and a confidence
      instructions: Which desk should take `message`?
      criteria:                         # 2 to 255 options, non-empty keys, in order;
        billing: Invoices, payments, refunds        #   a description or null
        technical: Errors, outages, how-to
        none_of_these: Not clearly any desk
    tone:                               # Score: a level on a scale, answered with
      type: score                       #   a weighted position and a confidence
      instructions: How upset is the writer of `message`?
      criteria: [calm, annoyed, angry]  # 2 to 10 levels, lowest first; a level
                                        #   may be an object; none is null
```

Limits the reader applies before anything is sent: a Choice needs 2 to 255
options with non-empty keys; a Score needs 2 to 10 levels, none null; a Noul
needs instructions or criteria that describe at least one outcome; no id is
empty.

Write Score levels as plain strings. A level may be an object, but its
"text" is then its JSON (`{"angry":"..."}`), which is what `level_at_least`
and a case's label would have to repeat. Put a level's definition in the
instructions (a `scale` part, or the instructions text) and keep the level
itself a word.

### Declarations (never sent to the model)

| Field | On | Meaning |
|---|---|---|
| `when` | any question | A state path; the question is asked only when the path is **present** in the state. |
| `part_when` | any question | Part name → state path: that part of an instructions **object** is sent only when the path is present. Every name must be a key of the instructions object. |
| `options_from: request` | Choice | The options come with each request, sent before the static ones in `criteria`; `criteria` may then hold fewer than two, or none. The request still needs 2 to 255. |

A **state path** is dot-separated keys, a number indexing an array:
`customer.account`, `turns.0.text`. It is present when it leads to a value
that is not `null`, `""`, `[]` or `{}`; `false` and `0` are present. That is
the only test the format makes on a state; anything richer is computed by the
application and put in the state.

```yaml
    duplicate_of:
      type: choice
      instructions:
        question: Which entry in `customer.open_tickets` is `message` about?
        account: Prefer a ticket on the same account (`customer.account`).
      criteria: {none: A new request}
      options_from: request
      when: customer.open_tickets
      part_when: {account: customer.account}
```

A Noul whose instructions object loses every part through `part_when` and
has no criteria is refused rather than sent asking nothing.

### Policy

One gate per question id; every field optional; a gate that does not fit its
question's primitive, or names a question the rubric lacks, is refused.

| Field | Applies to | Meaning | Absent |
|---|---|---|---|
| `threshold` | Noul only | Yes at this P(yes) and above; 0 to 1. | 0.5 |
| `confidence` | Choice, Score | Acted on at this confidence and above; deferred below. | 0 |
| `bands` | Choice, Score | `[{at_least, verdict}, …]`, bars strictly decreasing, verdict names distinct, non-empty; below the last band the answer is deferred. One of `confidence` or `bands`, not both. | |
| `fallback` | Choice, Score | What a deferred answer falls back to: an offered **static** option key (for a Choice), or a level by text or by index as a string (for a Score). | none |
| `level_at_least` | Score only | A level by index (number) or text; the verdict says whether the nearest level reached it. | |
| `strict` | any bar | `true`: met above the bar, not at it. | `false` |
| `note` | any | Why the bar is where it is. Part of the policy fingerprint. | |

`threshold` on a Choice or a Score, and `confidence`, `bands`, `fallback` or
`level_at_least` on a Noul, are refused.

```yaml
spec:
  policy:
    actionable: {threshold: 0.55, note: best F1 on the labelled cases}
    desk:
      bands:
        - {at_least: 0.70, verdict: route}
        - {at_least: 0.40, verdict: confirm}
      fallback: none_of_these
    tone: {level_at_least: angry}
```

### Tuning

```yaml
spec:
  tuning:
    cases: sha256:…          # the cases document's fingerprint (or its name)
    model: jev-1.13.0        # the versioned model that answered them
    server: https://api.typesafe.ai
    tuned_at: 2026-10-04T12:00:00Z
    labelled: 7              # anything else is kept as given
```

A rubric written by hand has no `tuning`; that absence says the bars are guesses.

## 4. `Cases`

`metadata.name` names the set (required); `metadata.description` says where
the cases came from. Under `spec`:

| Field | Required | Meaning |
|---|---|---|
| `rubric` | no | The rubric the labels are for, by its `metadata.name` or by its questions fingerprint. |
| `cases` | yes | At least one case, in order. |

A case:

| Field | Required | Meaning |
|---|---|---|
| `id` | no | A name, unique in the document; otherwise the case is `#3` by position. |
| `state` | yes | Any JSON. An array is a conversation, one element per turn. |
| `expect` | no | The right answer by question id, for the questions this case labels. A question left out is asked, not graded. |
| `options` | no | Question id → option key → description: the options supplied for this case's request, for every Choice with `options_from: request` that the case's request asks. |
| `tags` | no | Free labels. |
| `note` | no | Why the label is what it is. |

Labels, by the question's primitive:

| Primitive | Label |
|---|---|
| Noul | `true` or `false` (bare; `yes` is a string and refused) |
| Noul over a conversation | `{from_turn: n}`: true from turn `n` (zero-based) on; `{from_turn: null}`: never |
| Choice | an option key the case's request offers: static, or supplied in `options` |
| Score | a level's index (number, 0-based) or a level's text |

Binding cases to a rubric lowers each case's request from its state and
options, then checks every label against it. Refused: a label for a question
the request does not ask (its `when` is not present in this state); a Choice
label that is not among the options offered to this case; a Score index past
the last level; `from_turn` on a non-Noul, on a non-array state, or past the
last turn; `{}` or extra keys beside `from_turn`. A case for a rubric with
`options_from: request` must supply enough options for the request to have
2 to 255.

```yaml
apiVersion: jud/v1.3
kind: Cases
metadata:
  name: inbox-triage-cases
  description: Seven messages, labelled by the support lead.
spec:
  rubric: inbox-triage
  cases:
    - id: refund-angry
      state: {message: I was charged twice. Fix it today or I cancel.}
      expect: {actionable: true, desk: billing, tone: angry}
      tags: [billing]
    - id: receipt
      state: {message: Your payment of 12.00 EUR was received.}
      expect: {actionable: false, desk: none_of_these}   # tone: asked, not graded
      note: about billing, but asks nothing
    - id: escalates
      state:
        - {role: user, text: My export is stuck.}
        - {role: assistant, text: Could you share the export id?}
        - {role: user, text: Can I just talk to a person?}
      expect: {wants_human: {from_turn: 2}}
```

## 5. `Recording`

`metadata.name` is the case it answers: the case's name, `<case>-turn-<n>`
for one turn of a conversation, or a request hash. A Recording takes no
`metadata.version` and no `metadata.description`. Under `spec`:

| Field | Required | Meaning |
|---|---|---|
| `response` | yes | The response as received: `model`, `answers` (by question id, each with its `type`), `usage`, `request_id`, any other top-level field the server sent. |
| `elapsed_ms` | yes | Wall-clock time of the call, an integer. |
| `fingerprint` | no | The request fingerprint; `jud check` prints the expected one. |
| `request_hash` | no | This crate's own 16-hex hash; another writer leaves it out. |
| `rubric` | no | The rubric, by its `metadata.name` or fingerprint. |
| `server` | no | Base URL that answered. |
| `recorded_at` | no | RFC 3339 in UTC, quoted (`"2026-10-04T11:58:00Z"`). |

```yaml
apiVersion: jud/v1.3
kind: Recording
metadata:
  name: receipt
spec:
  response:
    model: jev-1.13.0
    answers:
      actionable: {type: noul, noul: 0.52}
      desk:
        type: choice
        choice: billing
        probabilities: {account: 0.02, billing: 0.55, none_of_these: 0.4, technical: 0.03}
        confidence: 0.4
      tone:
        type: score
        score: 0.35                                   # the weighted level index
        legend: {"0": calm, "1": annoyed, "2": angry} # must parse to the levels sent
        probabilities: {"0": 0.7, "1": 0.25, "2": 0.05}
        confidence: 0.7
    usage: {input_tokens: 410, output_tokens: 38}
  elapsed_ms: 140
  fingerprint: sha256:…
  rubric: inbox-triage
  recorded_at: "2026-10-04T11:58:00Z"
```

A recording is verified against the request's questions before it is
replayed: an answer for every question, of its primitive, a Choice naming an
offered option, a Score whose `legend` parses to the levels sent and whose
`probabilities` are keyed by level index. Write recordings by
hand only for tests; a `Recorder` writes real ones.

## 6. Fingerprints

`sha256:` + lowercase hex SHA-256 of RFC 8785 canonical JSON (keys sorted, no
whitespace, ECMAScript number formatting). `jud check` prints them.

| Of | Over |
|---|---|
| a rubric | its `spec.questions` map |
| a rubric's policy | its `spec.policy` map |
| a cases document | its `spec.cases` array |
| a request | `{"questions": …, "state": …}` |

The envelope is part of no fingerprint: `metadata` and the keys around the
map or array are outside it, so a document moved from an older envelope
keeps every fingerprint it had. Neither fingerprint sees key order: canonical
JSON sorts keys, so reordering questions or options changes what the model
sees without moving a fingerprint. The questions' fingerprint is not an
integrity check of the policy; pin both when a moved bar must be noticed.

## 7. What the reader refuses (checklist)

- `apiVersion` missing, a number, or any value other than the string
  `jud/v1.3`.
- An old `jud:` key: the earlier envelope is refused by name, never read.
- `kind` missing, not one of the three, or in the wrong case (`rubric` for
  `Rubric`).
- `metadata` or `metadata.name` missing.
- `metadata.version` on a Cases or Recording document; `metadata.description`
  on a Recording.
- A top-level key outside `apiVersion`, `kind`, `metadata`, `spec`: an `x-`
  key, or a kind's field (`questions`, `cases`, `response`) left outside
  `spec`.
- A name that is not a name (path, space, leading dot, empty): `metadata.name`
  or a case's `id`.
- A field the kind does not define (typo in a field name, `threshold` under
  `questions`, a gate field on the wrong primitive, a non-string value under
  `annotations` or `labels`).
- A Choice with fewer than 2 or more than 255 options; an empty option key.
- A Score with fewer than 2 or more than 10 levels, or a null level.
- A Noul `criteria` with a key other than `true` and `false`.
- `fallback` naming an option the Choice does not offer statically, or a level
  the Score does not have; `level_at_least` past the last level.
- `bands` empty, bars not strictly decreasing, a blank or repeated verdict;
  `bands` together with `confidence`.
- A bar outside 0 to 1.
- `when` or a `part_when` path malformed; a `part_when` name that is not a
  key of the instructions object; `part_when` on string instructions;
  `options_from` with a value other than `request`, or on a non-Choice.
- A merge key, an explicit tag, a duplicate key.
- When bound: a label for a question not asked, a label of the wrong shape, a
  Choice label not offered to that case, a `spec.rubric` that names neither
  the rubric's `metadata.name` nor its fingerprint, `options` for a question
  that does not take them or under a key it already offers.
