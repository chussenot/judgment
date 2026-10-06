# The .jud format, version 1.2: field reference

Condensed from `docs/jud.md`, which is the specification and wins on any
difference. Three kinds of YAML document share one envelope. `jud check`
(`scripts/jud.sh check`) applies every rule here and names the field it refuses.

## Contents

1. Envelope and names
2. YAML rules the reader enforces
3. `rubric`: questions, declarations, policy, tuning
4. `cases`: a case, labels per primitive, conversations, options
5. `recording`
6. Fingerprints
7. What the reader refuses (checklist)

## 1. Envelope and names

```yaml
jud: 1.2          # the number 1.2, never the string "1.2"
kind: rubric      # rubric | cases | recording
```

A **name** is ASCII letters, digits, `.`, `_` and `-`, starting with a letter
or a digit: `inbox-triage`, `refund-angry`, `T-98423`. No `/`, `\`, space,
leading `.`, and not empty. Every id is a name: a rubric's `id`, a cases
document's `id`, a case's `id`, a recording's `case`. A name is never a path,
so a tool can name a file after it and never leave its directory.

Declare `jud: 1.2`. The 1.2 rules add no field; the declaration says the file
was written under them. A document that uses a 1.1 feature (section 3 and 4)
would need at least `jud: 1.1`; `1.2` covers everything.

## 2. YAML rules the reader enforces

- YAML 1.2 core schema: only `true` and `false` are booleans. `yes`, `no`,
  `on`, `off`, `y` are **strings**, so an option key `yes` is fine and a Noul
  label must be the bare `true` or `false`.
- A duplicate key is an error.
- A merge key (`<<: *anchor`) is refused. A plain anchor and alias
  (`&rule` / `*rule`) on a scalar or a map value is fine; the examples share
  a sentence between questions that way.
- An explicit tag (`!secret`, `!!python/...`) is refused. `!!binary` stays
  the literal text, never decoded.
- JSON is accepted as YAML; a `.jud` can be JSON.
- Unknown fields are refused with their path, except: `tuning` and a
  recording's `response` keep anything; `state` is any JSON; top-level `x-`
  keys are kept and ignored (1.1).
- Order is kept and matters for `questions`, a Choice's `criteria`, a Score's
  `criteria`, `bands`, `cases`, `expect` and a case's `options`: it is the
  order the model sees. The parts of an instructions object carry no order.

## 3. `rubric`

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | A name, stable across edits. What `cases.rubric` and `recording.rubric` refer to. |
| `version` | no | Free-form edition (`2`, `2026-10`). The questions' fingerprint is the exact identity. |
| `description` | no | What the rubric decides, for the reader. |
| `questions` | yes | Map of question id to question, at least one, in the order the model sees them. |
| `policy` | no | Map of question id to gate. |
| `tuning` | no | Where the gates came from. Any fields; `cases`, `model`, `server`, `tuned_at` have a meaning. |
| `x-…` | no | Extension keys the reader ignores (1.1). |

### Questions

A question is exactly what the wire sends. The id is the key the answer comes
back under; the model never sees it. `instructions` is a string, an object
(named parts; sent as an object), an array, or absent. For a Choice the
model sees the option keys and their descriptions, in order.

```yaml
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

### Declarations (1.1; never sent to the model)

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
| `bands` | Choice, Score (1.1) | `[{at_least, verdict}, …]`, bars strictly decreasing, verdict names distinct, non-empty; below the last band the answer is deferred. One of `confidence` or `bands`, not both. | |
| `fallback` | Choice, Score | What a deferred answer falls back to: an offered **static** option key (for a Choice), or a level by text or by index as a string (for a Score). | none |
| `level_at_least` | Score only (1.1) | A level by index (number) or text; the verdict says whether the nearest level reached it. | |
| `strict` | any bar (1.1) | `true`: met above the bar, not at it. | `false` |
| `note` | any | Why the bar is where it is. Part of the policy fingerprint. | |

`threshold` on a Choice or a Score, and `confidence`, `bands`, `fallback` or
`level_at_least` on a Noul, are refused.

```yaml
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
tuning:
  cases: sha256:…          # the cases document's fingerprint (or its id)
  model: jev-1.13.0        # the versioned model that answered them
  server: https://api.typesafe.ai
  tuned_at: 2026-10-04T12:00:00Z
  labelled: 7              # anything else is kept as given
```

A rubric written by hand has no `tuning`; that absence says the bars are guesses.

## 4. `cases`

| Field | Required | Meaning |
|---|---|---|
| `id` | no | A name for the set. |
| `rubric` | no | The rubric the labels are for, by id or by fingerprint. |
| `description` | no | Where the cases came from. |
| `cases` | yes | At least one case, in order. |
| `x-…` | no | Extension keys (1.1). |

A case:

| Field | Required | Meaning |
|---|---|---|
| `id` | no | A name, unique in the document; otherwise the case is `#3` by position. |
| `state` | yes | Any JSON. An array is a conversation, one element per turn. |
| `expect` | no | The right answer by question id, for the questions this case labels. A question left out is asked, not graded. |
| `options` | no | (1.1) Question id → option key → description: the options supplied for this case's request, for every Choice with `options_from: request` that the case's request asks. |
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

## 5. `recording`

| Field | Required | Meaning |
|---|---|---|
| `case` | yes | A name: the case it answers (`<case>-turn-<n>` for one turn of a conversation), or a request hash. |
| `response` | yes | The response as received: `model`, `answers` (by question id, each with its `type`), `usage`, `request_id`, any other top-level field the server sent. |
| `elapsed_ms` | yes | Wall-clock time of the call, an integer. |
| `fingerprint` | no | The request fingerprint; `jud check` prints the expected one. |
| `request_hash` | no | This crate's own 16-hex hash; another writer leaves it out. |
| `rubric` | no | The rubric, by id or fingerprint. |
| `server` | no | Base URL that answered. |
| `recorded_at` | no | RFC 3339 in UTC, quoted (`"2026-10-04T11:58:00Z"`). |

```yaml
jud: 1.2
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
| a rubric | its `questions` map |
| a rubric's policy | its `policy` map (1.2) |
| a cases document | its `cases` array |
| a request | `{"questions": …, "state": …}` |

Neither fingerprint sees key order: canonical JSON sorts keys, so reordering
questions or options changes what the model sees without moving a
fingerprint. The questions' fingerprint is not an integrity check of the
policy; pin both when a moved bar must be noticed.

## 7. What the reader refuses (checklist)

- `jud` missing, a string, or a version other than 1, 1.0, 1.1, 1.2.
- `kind` missing or not one of the three.
- An id that is not a name (path, space, leading dot, empty).
- A field the kind does not define (typo in a field name, `threshold` under
  `questions`, a gate field on the wrong primitive).
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
- A 1.1 feature under `jud: 1` (declare 1.2 and this never happens).
- A merge key, an explicit tag, a duplicate key.
- When bound: a label for a question not asked, a label of the wrong shape, a
  Choice label not offered to that case, a cases `rubric` that names neither
  the rubric's id nor its fingerprint, `options` for a question that does not
  take them or under a key it already offers.
