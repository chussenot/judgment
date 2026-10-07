---
name: jud
description: Write, review or fix .jud documents (a rubric with its policy, labelled cases, a recording) in version 1.3 of the format the judgment crate reads, and check them with the crate's own reader. Use this whenever a task touches a .jud file, a rubric, typed questions with thresholds, labelled cases for tuning a threshold, or turning questions written in code into a document another tool can read, even when the user does not say "jud" or "rubric".
---

# Writing .jud documents (version 1.3)

A `.jud` file is a decision written down so a person can review it and a
program can run it: the typed questions a System One model is asked about a
state, and the thresholds at which its answers become actions. Three kinds
share one YAML envelope, a manifest's (`apiVersion`, `kind`, `metadata`,
`spec`): a **Rubric** (questions + policy), **Cases** (labelled states the
rubric is graded on) and a **Recording** (what a model answered once).
`docs/reference/jud-format.md` in the judgment repository is the specification,
shipped in this plugin as `references/format.md`;
`docs/concepts/rubrics-cases-recordings.md` is the reasoning behind the three kinds. This skill is the working guide.

What makes a document good is not the syntax, which `jud check` enforces, but
two things the checker cannot see: the questions are narrow and atomic, with
criteria that mean the same thing to every reader and to the model; and the
policy is honest about where its numbers come from.

This skill is part of the `jud` plugin, which also carries three commands:
`/jud:rubric <brief>` writes a rubric, `/jud:cases <rubric> [brief]` writes
the cases it is tuned on, `/jud:check [files]` runs the reader and explains
every refusal. Each follows this guide; `${CLAUDE_PLUGIN_ROOT}` is the
plugin's directory, so the checker is
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh` from anywhere.

## Workflow

1. **Decide the kind.** A decision needs a rubric. Thresholds need cases to
   be tuned on, so a rubric usually comes with a cases document. A recording
   is written by hand only for a test fixture.
2. **Read `references/format.md`** before writing: the specification itself,
   with the envelope, every field per kind, the gate fields per primitive,
   the label shapes, and the reading rules the reader refuses by. It is 400
   lines and saves a round of refusals.
3. **Write the document** with the envelope first: `apiVersion: jud/v1.3`,
   `kind`, `metadata` with its `name`, then everything the kind holds under
   `spec`:

   ```yaml
   apiVersion: jud/v1.3
   kind: Rubric
   metadata:
     name: inbox-triage
     description: What the rubric decides, for the reader.
   spec:
     questions: ...
     policy: ...
   ```

   Start from the nearest example in `examples/jud/` of the judgment
   repository when one is at hand (`triage.jud` and `triage-cases.jud` for
   one message with a tuned policy; `handoff.jud` and `handoff-cases.jud` for
   a conversation; `routing.jud` and `routing-cases.jud` for per-request
   options, `when`, `part_when` and bands).
4. **Check it** with the crate's reader, rubric and cases together so the
   labels are checked against the requests they grade:

   ```sh
   ${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh check path/to/rubric.jud path/to/cases.jud
   ```

   The script finds `jud` on PATH (`mise use -g github:chussenot/judgment@latest`
   or `cargo install judgment --features cli`),
   else builds it from a judgment checkout (`JUDGMENT_DIR`, the project, or
   the one the plugin sits in). Fix every `error` line; it names the field. A refusal is never
   worked around by loosening the document (dropping a label, widening an
   option list); it is a defect in the document, or in the question.
5. **When the request depends on the state** (`when`, `part_when`,
   `options_from: request`), look at what each case actually sends:

   ```sh
   ${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh lower path/to/rubric.jud --cases path/to/cases.jud
   ${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh lower path/to/rubric.jud --state '{"message": "..."}'
   ```

   A question that is missing from a case's request cannot be labelled for
   that case; a part that is missing was left out by `part_when`.
6. **Report** the files written and the `jud check` summary line, with the
   fingerprints when the rubric will be pinned or the cases named by `tuning`.

## The rules writers trip on

Each of these is a refusal `jud check` will name; knowing them first saves the
round trip.

- `apiVersion` is the string `jud/v1.3`, exactly: the reader reads that one
  value and refuses any other, a number, a missing key, or a document that
  still starts with a `jud:` key. `kind` is PascalCase: `Rubric`, `Cases`,
  `Recording`; `rubric` is refused.
- `metadata.name` is required on every kind and is a **name**: letters,
  digits, `.`, `_`, `-`, starting with a letter or digit. `inbox-triage` yes;
  `inbox triage`, `./triage`, `.hidden` no. A case's `id` inside `spec.cases`
  is a name too.
- The top level holds the four keys and nothing else. What a kind defines
  sits under `spec`; what the format does not name goes in
  `metadata.annotations` (or `metadata.labels`), string to string, where the
  reader keeps it and a shared YAML anchor can live.
- `metadata.version` belongs to a Rubric only; `metadata.description` to a
  Rubric or Cases. Either on a Recording, or `version` on Cases, is refused.
- Only `true` and `false` are booleans. `yes` and `no` are strings: fine as
  option keys, refused as Noul labels.
- A **Choice** offers 2 to 255 options, each key non-empty, in the order the
  model sees them. Give it a `none_of_these` (or `other`) option when the state
  might fit none, so the model has somewhere to put its doubt.
- A **Score** has 2 to 10 levels, lowest first, none null, each a plain
  string.
- A **Noul**'s `criteria` keys are exactly `true` and `false`.
- A gate fits its primitive: `threshold` is Noul-only; `confidence`, `bands`,
  `fallback`, `level_at_least` are Choice or Score (`level_at_least` Score-only).
- `fallback` names an option the Choice offers **statically** in `criteria`,
  or a Score level by text or index-as-string. A supplied option cannot be a
  fallback.
- `bands` bars strictly decrease, verdict names are distinct and non-empty,
  and a gate has `bands` or `confidence`, not both.
- A label in `expect` fits the question: `true`/`false` for a Noul,
  `{from_turn: n}` only for a Noul over an array state, an offered option key
  for a Choice, a level index or text for a Score. A label for a question whose
  `when` is not present in that case's state is refused: a question not asked
  has no answer to grade.
- For a Choice with `options_from: request`, every case supplies its options
  under `options.<question id>`, and the request still needs 2 to 255 in total.
- No merge keys (`<<`), no explicit tags. A plain anchor and alias on a scalar
  or a map value is fine for sharing one sentence between questions; put the
  anchor in an annotation so no question has to carry it.
- An unknown field is refused with its path, so a typo in a field name is a
  refusal, not a silently ignored key. `spec.tuning`, a recording's
  `spec.response`, `metadata.annotations` and `metadata.labels` are the only
  open maps.
- The policy is never sent to the model, so a threshold in the instructions
  ("answer yes only if you are quite sure") has moved the bar into the model
  where it cannot be tuned. Keep the decision out of the question.

## Writing questions that hold up

- **One question, one thing.** "Which desk, and is it urgent?" is two
  questions. A model answers a compound question with one probability about
  nothing in particular.
- **The criteria carry the meaning.** `Does the message convey urgency?`
  reads differently to every grader; `true: a deadline, a service down, a
  threat to cancel` reads the same to all of them and to the model.
- **Name the state.** Instructions that say `message` and criteria that say
  what is in it tell the model where to look; the state is JSON and the model
  sees all of it. Write the state keys the application will really send.
- **Ask only when there is something to ask.** A question about open tickets
  returns noise when none is open: `when: customer.open_tickets`.
- **Facts the application has stay in code.** Whether the plan is enterprise,
  whether the order is older than a week: computed by the caller, put in the
  state, never asked of the model.
- **Be honest in the policy.** A hand-written rubric has no `tuning` block,
  and a `threshold: 0.5` with no `note` says "a guess" plainly. A `note` on a
  gate says why the bar is where it is. Do not invent a `tuning` block, a
  fingerprint or a model version: `tuning.cases` is written by the tuning run
  (`examples/jud_calibration.rs`) from the real cases fingerprint.

## Writing cases a threshold can rest on

A cases document is the labelled examples a rubric is graded on, and the
only thing a threshold can be tuned against. The reader checks that every
label fits its question; it cannot check that the labels are right or that
they cover anything. That is the writer's job.

- **Start from the rubric.** List each question, its primitive, each Choice's
  options and whether they come with the request, each Score's levels, and
  every `when`. A label is only valid for a question the case's request
  actually asks: a case whose state lacks the path a `when` names cannot
  label that question, and the reader refuses it.
- **Take real states.** The keys the application really sends, in their real
  shape, so that a `when` behaves in the cases as it will in production. An
  invented state tests an invented request.
- **Cover the outcomes.** Every option of a Choice and every level of a Score
  should be the right answer at least once, including the way out: the
  `none_of_these` option, the lowest level, the `false` of a Noul. A set with
  no negative cases tunes a bar that never says no. Say in the reply how many
  cases label each outcome, so the gaps are visible.
- **Label what the case is sure about.** A question left out of `expect` is
  asked and not graded. A wrong label costs more than a missing one, because
  the bar moves to fit it. Label in the answer's own vocabulary: bare `true`
  or `false` for a Noul, an offered option key for a Choice, a level's text or
  index for a Score.
- **Keep the hard ones, and say why.** The cases that were argued about are
  the ones that define the line. Give each a `note` with the argument, so the
  next labeller does not reopen it, and leave the question unlabelled on a
  case that is genuinely ambiguous about it.
- **A conversation is labelled by turn.** The state is an array of turns;
  a Noul takes `{from_turn: n}` for the zero-based turn at which it becomes
  true (assistant turns count) or `{from_turn: null}` for never; every other
  label stands for the last turn.
- **Supply what the request needs.** For a Choice with `options_from:
  request`, each case supplies its own options under `options`, as the
  application would that day, and the request still needs 2 to 255.
- **Name the rubric.** `spec.rubric` by name while the questions move, by
  fingerprint once a run must be reproducible.
- **Never copy the model's answers in.** A case says what the answer should
  be; a recording says what it was. A label copied from a recording turns the
  test into a test of nothing.
- **Count honestly.** A handful of cases finds a document that is wrong and
  shows the loop; it does not tune a bar. Say so rather than letting a
  `tuning` block imply otherwise.

## Turning questions written in code into a rubric

The builder calls map one to one, because a question in a rubric is the
wire's own shape: `questions.choice::<T>(id, instructions)` with an
`options!` enum is a Choice whose `criteria` are the enum's keys and
descriptions in declaration order; `questions.noul(id, instructions, None)`
is a Noul with no `criteria` (leave the key out; `None` and absent are the
same on the wire); `questions.score(id, instructions, levels)` is a Score
with those levels. The map goes under `spec.questions`, in the code's order,
and the rubric's `metadata.name` is what `Rubric::new(name, questions)`
would be given. What the code does with an answer is the policy:
`confidence.at_least(0.7)` is `confidence: 0.7`, `is_yes(0.6)` is
`threshold: 0.6`, a strict `>` is `strict: true`; a comparison of the chosen
option (`chosen == Billing`) stays in the caller and gets no gate. Add
nothing the code does not send (a `none_of_these` option changes the
request), and say so in the reply. To prove parity, `scripts/jud.sh lower`
prints the request the rubric sends; compare it with what the code sends
(a `Fake` or a `Recorder` shows that), and the request fingerprint `jud
check` prints for a recording is the one a `Replay` keys by. No probe crate
is needed.

## Moving an old document forward

Files written under the earlier envelope (a `jud:` key with `kind`, `id`
and the kind's fields at the top level) exist, and the reader refuses them
by name rather than reading them: there is one envelope, and a document is
moved by rewriting it. The content under `spec` is unchanged in name and
meaning, and no fingerprint moves (a rubric's is over `spec.questions`, a
cases document's over `spec.cases`), so a `tuning.cases` or a pinned
fingerprint still holds after the move.

| Old envelope | New envelope |
|---|---|
| `jud: 1`, `1.1`, `1.2` | `apiVersion: jud/v1.3` |
| `kind: rubric`, `cases`, `recording` | `kind: Rubric`, `Cases`, `Recording` |
| `id` (rubric, cases) | `metadata.name`, now required on cases too |
| a recording's `case` | `metadata.name` |
| `version` (rubric) | `metadata.version` |
| `description` (rubric, cases) | `metadata.description` |
| `x-…` keys | `metadata.annotations`, each value written as a string; an anchor that lived there moves with it |
| a cases document's `rubric` | `spec.rubric` |
| everything else (`questions`, `policy`, `tuning`, `cases`, `response`, `elapsed_ms`, …) | under `spec`, unchanged |

Then run `scripts/jud.sh check`: a path-shaped id, a merge key or a tag an
old reader let through is refused now and must be fixed in the document.

## Reviewing an existing document

Run `jud.sh check` first; then read for what the checker cannot see:
a compound question, a Choice with no way out, criteria that restate the
question instead of defining the outcomes, a `tuning` block with no run
behind it, a label that contradicts its note. A document the reader refuses
by its envelope is in the old shape: move it forward (section above) before
reviewing its content.

## Output

Write the documents to the paths asked for (`.jud` extension), one document
per file, with a short header comment saying what the file is for. End with
the `jud check` summary and, for a rubric that will be pinned, its two
fingerprints as `jud check` printed them.
