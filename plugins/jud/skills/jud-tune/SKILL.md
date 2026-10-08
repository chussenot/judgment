---
name: jud-tune
description: Measure and tune a .jud rubric against its labelled cases. Record a model's answers once, read what jud eval says about accuracy, calibration and misses, and decide whether each miss is a wrong label, a weak question, a bar or the model. Then apply what jud tune proposes to the rubric in place and show the before and after. Use this whenever a task is about how well a rubric's questions or thresholds work, why a decision came out wrong, recordings, calibration, accuracy, thresholds that "feel off", or tuning or evaluating .jud files, even when the user does not say "jud", "eval" or "tune".
---

# Tuning a .jud rubric

A rubric's questions are asked of a model; its policy turns the answers into
actions at bars (`threshold`, `confidence`, `level_at_least`). The bars are
only as good as the cases they were measured on. Three `jud` commands make
the measurement; this guide is about reading what they print and deciding
what to change, which they do not do.

The commands run through the plugin's script, so they work wherever `jud` is
on PATH or a judgment checkout is at hand. Below, `$JUD` stands for
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`; when you run a command,
write the path out rather than setting a variable, so a permission rule for
the script matches it:

```sh
$JUD record RUBRIC CASES --out DIR        # spends one call per request; keeps the answers
$JUD eval   RUBRIC CASES --replay DIR     # grades the kept answers; no key, no network
$JUD tune   RUBRIC CASES --replay DIR     # proposes each gate's bar; never calls anything
```

The format and how to write a rubric or cases are the `jud` skill's
(`${CLAUDE_PLUGIN_ROOT}/skills/jud/SKILL.md`); read it before changing a
question or a label.

## What costs what, and what invalidates what

This decides the order of everything else, so know it first.

| You change | Recordings still answer? | Cost of the next measurement |
|---|---|---|
| A gate in `policy` (a bar, a fallback, a note), or the `tuning` block | yes: the policy is never sent | none: `eval` and `tune` replay |
| A label in a case's `expect`, or a case's `note` | yes: labels are never sent | none |
| A case's `state` or `options`, or a new case | no, for that case | one call per new request |
| Anything under `questions` (instructions, criteria, options, levels, `when`) | no, for every case that asks it | one call per case |
| The model (`model` in `jud config`) | the old answers still replay, but they are the old model's | a full `record --refresh` to measure the new one |

A recording is keyed by the request's fingerprint (the state and the
questions sent). When the questions change, `eval --replay` exits with
status 1 and `no recording answers N cases: ...; record them first`. That
message means the rubric and the recordings no longer match, not that
something is broken. `$JUD check RUBRIC CASES DIR/*.jud` shows which:
every recording whose request changed says `fingerprint differs`. All of
them means a question changed; one means that case's state or options
did. Never edit a recording to make it fit, and never revert a question
silently to make the old recordings work: say which change made them stale
and what re-recording costs.

So fix labels and bars freely; batch question changes, then re-record once.

## Before spending calls

`jud record` is the only step that costs money, and it asks the backend
`jud config` names. Before running it, find out and say:

- **How many requests.** One per case, except a conversation (an array
  state) whose cases label a Noul with `{from_turn: ...}`. Such a
  conversation is recorded once per turn, `len(state)` requests. Requests
  already in `--out DIR` are kept and not asked again, so a resumed or
  repeated run costs only what is new. `--refresh` asks everything again.
- **Which backend and model.** Run `$JUD config` and read `base_url` and
  `model`. It never prints a key; it says whether one is set.
- **Whether a key is set.** `TYPESAFE_API_KEY` or `api_key` in the
  configuration file; any non-blank word for a local server that ignores it.
  Never print or ask for the key's value.

If the user asked for this run in so many words, run it after saying the
count. Otherwise, stop at the count and ask. A run that fails part way keeps
what it wrote, so running the same command again resumes.

## Reading `jud eval`

Prefer `--replay DIR`: the same numbers, free and repeatable. Without it,
`eval` asks the backend for every case and keeps nothing, which spends the
calls a `record` would have kept. Add `--json` when you need the numbers
rather than the text.

Per question:

- **`labelled n, correct k, accuracy a (95% interval lo to hi)`.** This is the
  model's own answers against the labels, before any gate. The interval
  matters more than the point: it is the range the true accuracy could be in
  given only `n` cases. At about 90 % accuracy:

  | labelled | 95 % interval | what it supports |
  |---|---|---|
  | 7 | 0.49 to 0.97 | finding a broken document; nothing about a bar |
  | 30 | 0.74 to 0.97 | a first bar, flagged as provisional |
  | 100 | 0.83 to 0.94 | a bar worth pinning |
  | 400 | 0.87 to 0.93 | telling two close bars apart |

  `jud tune` warns when the interval is wider than 0.2, which is below
  about 35 cases at 90 %. Pass that warning on; do not soften it.
- **`brier`, `calibration error`, `confidence when right, when wrong`.** Read
  these as one question: does the model's confidence separate its right
  answers from its wrong ones? A bar can only defer what it can tell apart.
  - When right 0.85 and wrong 0.45: a bar between them will work.
  - When right 0.80 and wrong 0.78: no bar helps. The fix is in the question
    or the cases, not the policy.
  - `-` for "when wrong" means no misses, so nothing was learned about what a
    miss looks like.
- **`gate: acts on x of n, defers d, accuracy when acted p`.** This is the
  policy's operating point on these cases: how often it decides by itself,
  and how often that decision is right. Deferring sends the case to the
  gate's `fallback`, or to a person. Raising a bar trades acts for accuracy.
  Which side matters is the user's call: a refund sent wrongly costs more
  than a ticket routed to a human.
- **`model misses`.** Every case where the model's answer differs from the
  label, with its confidence. Triage every one (next section).

`--min-accuracy 0.9` (or `desk=0.95`) makes `eval` exit with status 3 when a
question's accuracy falls short. That is how a rubric is held in CI against
committed recordings (`docs/guides/run-in-ci.md` in the judgment
repository).

## Triage every miss

The numbers say how often; the misses say why. For each miss, read the
case's `state`, its `note`, the question's instructions and criteria, and
the confidence. Then put it in exactly one row:

| What you see | Cause | What to do |
|---|---|---|
| The state plainly supports the model's answer, or the case's own `note` argues for it | **The label is wrong** | Propose the corrected label to the user, with the reason. Never change a label just because the model disagreed: the model being confident is not evidence. |
| Confidence is low, at or under the gate's bar | **The bar's job** | Nothing. The gate defers it, which is what the bar is for. Say so. |
| Confidence is high, the label is right, and the criteria could be read the model's way | **The question is ambiguous** | Propose a sharper criterion: name the deciding detail. This changes the request, so re-record after. |
| Confidence is high, the label is right, and the deciding fact is not in the state | **The question asks what the model cannot see** | The fact belongs in the state, computed by the caller, or the question should not be asked (`when`). |
| Several misses swap the same two options or adjacent levels | **The outcomes overlap** | Sharpen both criteria against each other, merge the options, or use fewer levels. Re-record. |
| The miss's right answer is an outcome few or no other cases label | **A coverage gap** | Add cases for that outcome before reading anything into the bar. |
| None of the above: the label is right, the question is clear, and the model is confidently wrong once | **The model** | Record it as a known miss. One such case does not move a bar or justify rewriting a question. |

Report the triage as a table (case, question, expected, predicted,
confidence, cause, proposed action). Propose; do not apply. A label or a
question is the user's to change, and a question change costs a
re-recording.

## Coverage

`jud eval` does not count labels per outcome; count them from the cases
document. For each question, list how many cases label each option, each
level, `true` and `false` (or each `from_turn`). Flag:

- an outcome no case labels, so the model is never tested on it;
- a Noul with no `false` cases, so its threshold can never learn to say no;
- a way-out option (`none_of_these`) that is never the right answer, so the
  fallback was never measured;
- a question labelled on far fewer cases than the others (`when`, or labels
  left out), so its interval is wider than the table makes it look.

## Reading `jud tune`

`tune` writes its tables and warnings to stderr and its proposal to stdout:
a `policy:` block and a `tuning:` block at column 0. Each gate gets a line
like `propose confidence 0.45 (now 0.30): lowest bar at 95% accuracy; covers
6 of 7 labelled cases`.

How each bar is chosen:

- **A Noul's `threshold`:** the one with the best F1, the lower on a tie.
- **A Choice's or Score's `confidence`:** the lowest bar at which the cases
  it covers are at least `--target-accuracy` right (0.95 by default) and at
  least `--min-covered` cases are still covered (3 by default).
- **A Score's `level_at_least`**, when the gate has one: the one with the
  best F1.
- A gate with `bands` gets its table and no proposal.

Decide per gate, and say which you decided:

- **Accept** when the labelled count is enough for the use (the interval
  table above), the proposal moves in a direction the misses explain, and
  coverage stays acceptable to the user.
- **Keep the written bar** when the proposal is `0` ("the gate would defer
  nothing") on few cases, when the proposal sits on a single case's
  confidence, or when the cases are too few to trust. Give the kept gate a
  `note` that says so, for example `kept at 0.30: tune proposed 0.00 on 6
  cases, too few to drop the bar`.
- **Re-run with another `--target-accuracy`** when the proposed bar covers
  too few cases to be useful (say 2 of 40). Each run is free. Show the
  trade-off: at 0.95 the gate covers 12 of 40, at 0.90 it covers 31.
- **Pass on** the strict-gate note (an answer exactly at the bar is read the
  other way) and the Score note (the table reads the most probable level,
  the policy the level nearest the weighted score). Both say the table may
  differ slightly from what the policy will do. The before and after `eval`
  shows what it actually does.

Never chase 100 %. A bar that makes every labelled case right on a few
dozen cases has learned those cases.

## Applying a proposal

Edit the rubric in place. `--out` writes a new file but serialises it
again, so every comment and the layout are lost. Use it only when the user
wants a separate file.

1. For each accepted gate whose value moves, change only the tuned value
   (`threshold`, `confidence`, `level_at_least`) and its `note`. Keep
   `fallback`, `bands`, `strict` and every comment as they are. A gate whose
   proposal equals what is written (`propose 0.55 (now 0.55)`) stays exactly
   as written, its note included: the person who wrote that note knew
   something the generated one does not say.
2. Replace or add `spec.tuning` with the block `tune` printed, indented two
   spaces under `spec:`, values unchanged. `cases`, `model`, `server`,
   `tuned_at` and `labelled` describe the run and are never written by hand.
   If you kept some gates, the block stays: it records the run the other
   gates came from.
3. Check: `$JUD check RUBRIC CASES` must say `0 refused`. The questions
   fingerprint must be the one it was before your edit: you changed only
   the policy.
4. Show before and after: run `$JUD eval RUBRIC CASES --replay DIR` before
   the edit and again after it. Keep the outputs in the conversation; write
   no copy of the rubric and no output file into the user's project. Compare
   each gate's acts, defers and accuracy when acted. The model's accuracy
   does not move, since only the policy changed; say so if someone expects
   it to.

A rubric that already had a `tuning` block from other cases gets the new
block, not a merge. If the cases fingerprint in the new block differs from
the old one, say that the bars now rest on different cases.

## Do not grade on what you tuned on

A bar tuned on the same cases it is graded on looks better than it will do.
With about 40 labelled cases or more per question, hold some out:

1. Split the cases into two documents: a tuning set and a held-out set,
   every outcome in both. Take every fourth case into the held-out set
   unless the user has a better split. Give each document its own
   `metadata.name`, and name the files after the cases file
   (`support-cases.jud` gives `support-cases-tune.jud` and
   `support-cases-holdout.jud`). The states are unchanged, so the recordings already in
   DIR answer both: nothing is re-recorded.
2. Tune on the tuning set and apply.
3. Grade the tuned rubric on the held-out set with
   `eval --replay DIR`. Its gate numbers are the honest ones; report those
   as the expected performance.

Below about 40, say that there are too few to hold any out, and that the
numbers are optimistic.

## Iterating

One round is: triage the misses, fix the labels the user agrees are wrong
(free), batch the question changes (one re-recording), add cases where
coverage is thin (calls only for the new cases), then tune again. Stop when
the misses left are the model's own and the bars rest on enough cases.

## Never

- Write a `tuning` block, a fingerprint, a model or a time that `jud tune`
  did not print.
- Edit, delete or hand-write a recording to make `eval` pass. A recording is
  what a model said.
- Change a label to match the model without the user agreeing that the
  label was wrong.
- Spend calls without saying how many first: `record`, `eval` without
  `--replay`, or `record --refresh`.
- Mix two models' recordings in one directory. `tune` refuses them (`the
  recordings come from more than one model`); record each model into its own
  directory and compare the two `eval` reports.
- Present a bar read off a handful of cases as anything but a guess with a
  number on it.
