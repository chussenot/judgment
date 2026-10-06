---
name: jud
description: Write, review or fix .jud documents (a rubric with its policy, labelled cases, a recording) in version 1.2 of the format the judgment crate reads, and check them with the crate's own reader. Use this whenever a task touches a .jud file, a rubric, typed questions with thresholds, labelled cases for tuning a threshold, or turning questions written in code into a document another tool can read, even when the user does not say "jud" or "rubric".
---

# Writing .jud documents (version 1.2)

A `.jud` file is a decision written down so a person can review it and a
program can run it: the typed questions a System One model is asked about a
state, and the thresholds at which its answers become actions. Three kinds
share one YAML envelope: a **rubric** (questions + policy), **cases** (labelled
states the rubric is graded on) and a **recording** (what a model answered
once). `docs/jud.md` in the judgment repository is the specification;
`docs/rubric.md` is the reasoning. This skill is the working guide.

What makes a document good is not the syntax, which `jud check` enforces, but
two things the checker cannot see: the questions are narrow and atomic, with
criteria that mean the same thing to every reader and to the model; and the
policy is honest about where its numbers come from.

## Workflow

1. **Decide the kind.** A decision needs a rubric. Thresholds need cases to
   be tuned on, so a rubric usually comes with a cases document. A recording
   is written by hand only for a test fixture.
2. **Read `references/format.md`** before writing: every field per kind,
   the gate fields per primitive, the label shapes, and the checklist of what
   the reader refuses. It is 300 lines and saves a round of refusals.
3. **Write the document** with `jud: 1.2` on the first line. Start from the
   nearest example in `examples/jud/` of the judgment repository when one
   is at hand (`triage.jud` and `triage-cases.jud` for one message with a
   tuned policy; `handoff.jud` and `handoff-cases.jud` for a conversation;
   `routing.jud` and `routing-cases.jud` for per-request options, `when`,
   `part_when` and bands).
4. **Check it** with the crate's reader, rubric and cases together so the
   labels are checked against the requests they grade:

   ```sh
   scripts/jud.sh check path/to/rubric.jud path/to/cases.jud
   ```

   The script finds `jud` on PATH, else builds it from the judgment
   checkout. Fix every `error` line; it names the field. A refusal is never
   worked around by loosening the document (dropping a label, widening an
   option list); it is a defect in the document, or in the question.
5. **When the request depends on the state** (`when`, `part_when`,
   `options_from: request`), look at what each case actually sends:

   ```sh
   scripts/jud.sh lower path/to/rubric.jud --cases path/to/cases.jud
   scripts/jud.sh lower path/to/rubric.jud --state '{"message": "..."}'
   ```

   A question that is missing from a case's request cannot be labelled for
   that case; a part that is missing was left out by `part_when`.
6. **Report** the files written and the `jud check` summary line, with the
   fingerprints when the rubric will be pinned or the cases named by `tuning`.

## The rules writers trip on

Each of these is a refusal `jud check` will name; knowing them first saves the
round trip.

- `jud: 1.2` is a number. `"1.2"` is a string and refused.
- Every id is a **name**: letters, digits, `.`, `_`, `-`, starting with a
  letter or digit. `inbox-triage` yes; `inbox triage`, `./triage`, `.hidden` no.
- Only `true` and `false` are booleans. `yes` and `no` are strings: fine as
  option keys, refused as Noul labels.
- A **Choice** offers 2 to 255 options, each key non-empty, in the order the
  model sees them. Give it a `none_of_these` (or `other`) option when the state
  might fit none, so the model has somewhere to put its doubt.
- A **Score** has 2 to 10 levels, lowest first, none null.
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
  or a map value is fine for sharing one sentence between questions.
- An unknown field is refused with its path, so a typo in a field name is a
  refusal, not a silently ignored key. `tuning` and a recording's `response`
  are the only open maps.
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
- **Label what the case is sure about.** A question left out of `expect` is
  asked and not graded. Label the obvious cases and the ones that were argued
  about; a `note` on a hard case says why the label is what it is.

## Turning questions written in code into a rubric

The builder calls map one to one, because a question in a rubric is the
wire's own shape: `questions.choice::<T>(id, instructions)` with an
`options!` enum is a Choice whose `criteria` are the enum's keys and
descriptions in declaration order; `questions.noul(id, instructions, None)`
is a Noul with no `criteria` (leave the key out; `None` and absent are the
same on the wire); `questions.score(id, instructions, levels)` is a Score
with those levels. What the code does with an answer is the policy:
`confidence.at_least(0.7)` is `confidence: 0.7`, `is_yes(0.6)` is
`threshold: 0.6`, a strict `>` is `strict: true`; a comparison of the chosen
option (`chosen == Billing`) stays in the caller and gets no gate. Add
nothing the code does not send (a `none_of_these` option changes the
request), and say so in the reply. To prove parity, `scripts/jud.sh lower`
prints the request the rubric sends; compare it with what the code sends
(a `Fake` or a `Recorder` shows that), and the request fingerprint `jud
check` prints for a recording is the one a `Replay` keys by. No probe crate
is needed.

## Reviewing an existing document

Run `scripts/jud.sh check` first; then read for what the checker cannot see:
a compound question, a Choice with no way out, criteria that restate the
question instead of defining the outcomes, a `tuning` block with no run
behind it, a label that contradicts its note. Move a document from `jud: 1`
or `1.1` to `1.2` by changing the declaration; 1.2 adds no field, and any
path-shaped id, merge key or tag the old reader let through is now refused
and must be fixed.

## Output

Write the documents to the paths asked for (`.jud` extension), one document
per file, with a short header comment saying what the file is for. End with
the `jud check` summary and, for a rubric that will be pinned, its two
fingerprints as `jud check` printed them.
