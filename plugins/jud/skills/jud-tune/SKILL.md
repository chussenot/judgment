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
on PATH or a judgment checkout is at hand:

```sh
${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh record RUBRIC CASES --out DIR     # spends one call per request; keeps the answers
${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh eval RUBRIC CASES --replay DIR    # grades the kept answers; no key, no network
${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh tune RUBRIC CASES --replay DIR    # proposes each gate's bar; never calls anything
```

How to run them, because a user's permission rules match the command as
written:

- Write the script's path out in full every time. Do not set a shell
  variable or `cd` first. Below, `jud.sh` is short for that path.
- Run one `jud` command per Bash call, with nothing chained before or after
  it (`;`, `&&`, `|`, `echo $?`) and no redirection into a file. The tool
  result shows stdout, stderr and a non-zero exit status.
- The same holds for every other command: run `git diff -- support.jud`
  as written, from the working directory, with no `-C`, no `cd`, no
  absolute path.
- Read files with the Read tool and edit them with the Edit tool. Make a
  copy with Read and Write, not `cp` or `mkdir`: the Write tool creates the
  directory.
- Write nothing into the user's project that the task did not ask for: no
  copy of the rubric, no saved output. A what-if copy (a cases document with
  one label changed, to measure what the change would do) or a split you
  were not asked to keep goes in a directory of its own outside the project:
  the session's scratchpad or temporary directory if the system names one,
  else `/tmp/jud-<rubric name>-<what>/` (for example
  `/tmp/jud-support-triage-relabel-refund-request/`). Do not run a command
  to find `$TMPDIR`. Build the copy with Read then Write only, no `sed`,
  `cp`, `mkdir` or redirection. If the file already exists, Read it in full
  before overwriting it, and never run `jud` on a copy this conversation
  did not write. The reply says it was a copy.

The format and how to write a rubric or cases are the `jud` skill's
(`${CLAUDE_PLUGIN_ROOT}/skills/jud/SKILL.md`); read it before changing a
question or a label.

Every reply after `eval` or `tune` ends with the items in "What the reply
contains" (the last section): one line per question, the shortfall in cases,
the triage table, coverage counts, and for `tune` the decision table, the
warnings as printed, and the before and after. Check them off before you
send it.

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
something is broken. `jud.sh check RUBRIC CASES DIR/*.jud` shows which:
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
  conversation is recorded once per entry of the array, `len(state)`
  requests, assistant entries included.
- **How many are new.** A recording in `--out DIR` that still answers its
  request is kept and not asked again; a stale one (its request changed) is
  asked again and its file replaced. `jud.sh check RUBRIC CASES DIR/*.jud`
  tells them apart (`fingerprint matches` or `differs`). The cost is the
  requests minus the matching recordings, or all of them with `--refresh`.
  When some are stale, find what changed (`git diff -- RUBRIC CASES`, or
  `git log -p -1 -- RUBRIC` for a committed change) and name it.
- **Which backend and model.** Run `jud.sh config` and read `base_url`,
  `model` and `model_from`. `model_from: default` means nothing chose the
  model; `jev-latest` is an alias, and the recordings will name the version
  that answered. `config` never prints a key, and does not contact the
  backend.
- **Whether a key is set.** `TYPESAFE_API_KEY` or `api_key` in the
  configuration file; any non-blank word for a local server that ignores it.
  The user sets it outside the conversation: `export TYPESAFE_API_KEY=...`
  in their shell (or a gitignored `.env` that mise or direnv loads; `jud`
  itself reads no `.env`), or `api_key` in the configuration file `jud
  config` names. Never print it or ask for it.

Say the cost to the user in a message of its own before the `record` call,
for example `Recording 18 requests (30 already in recordings/support) to
https://api.typesafe.ai, model jev-latest.` A final reply after the run is
too late. If the user asked for this run in so many words (`/jud:record`
does), run it after that message; otherwise stop at the count and ask. A
run that fails part way keeps what it wrote, so the same command resumes.

## Reading `jud eval`

Prefer `--replay DIR`: the same numbers, free and repeatable. Without it,
`eval` asks the backend for every case and keeps nothing, which spends the
calls a `record` would have kept. Quote numbers from `--json`, not from the
rounded text, and only numbers the output gives: a count, a width or a
prediction you worked out is labelled as yours.

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
  about 35 cases at 90 %. Judge the width from `accuracy_interval95` in the
  JSON; a width that rounds to 0.20 may not trigger it. Pass the warning on
  when `tune` prints it; do not soften it, and do not predict it.

  Say what each question is short of: about 35 labelled cases for a first
  bar and 100 for a pinned one, minus its own `labelled`. One new case can
  label every question, so the total to add is the largest shortfall, not
  the sum. Then name the outcomes the new cases should label, from the
  coverage count.
- **Tuned on these cases?** If the rubric's `spec.tuning.cases` equals the
  `cases.fingerprint` in the JSON, the bars were tuned on the cases being
  graded, so the gate numbers are optimistic (see "Do not grade on what you
  tuned on"). It says nothing about the recordings: a replay that exits 0 or
  3 is what shows they still answer.
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
  label, with its confidence. A Noul's is the probability of its answer; a
  Score's levels are printed as indices, 0 being the first level in
  `criteria`, so map them to names first. Triage every one (next section).

`--min-accuracy 0.9` (or `desk=0.95`) makes `eval` exit with status 3 when a
question's accuracy falls short. That is how a rubric is held in CI: the
recordings are committed, and a CI job runs the same `eval --replay ...
--min-accuracy` and fails the build on status 3 (`docs/guides/run-in-ci.md`
in the judgment repository). The floor holds `accuracy` (every labelled
case), never accuracy when acted: a deferred miss still counts. The policy is
never sent, so no bar or fallback can raise it; only a fixed label, a
sharper question or another model can. When a question falls short, say what
would raise it, with the arithmetic (relabelling one case gives 42 of 48,
0.875, still short of 0.9).

## Triage every miss

The numbers say how often; the misses say why. For each miss, read the
case's `state`, its `note`, the question's instructions and criteria, and
the confidence. Then put it in exactly one row:

| What you see | Cause | What to do |
|---|---|---|
| The state plainly supports the model's answer, or the case's own `note` argues for it | **The label is wrong** | Propose the corrected label to the user, with the reason. Never change a label just because the model disagreed: the model being confident is not evidence, and a low-confidence miss on a label the criteria plainly support is not a label problem. |
| A Choice or Score answer at or under the gate's `confidence` bar | **The bar's job** | Nothing for the gate: it defers this to its `fallback`. Say so. It is still a wrong answer: it counts in `accuracy` and against any `--min-accuracy` floor, and only a sharper question or another model removes it. |
| A Noul whose probability falls on the label's side of `threshold` (0.52 under a 0.55 threshold, labelled no) | **The bar's job** | Nothing: a threshold never defers, it decides, and here it decides the label. Say so. |
| Confidence is high, the label is right, and the criteria could be read the model's way | **The question is ambiguous** | Propose a sharper criterion: name the deciding detail. This changes the request, so re-record after. |
| Confidence is high, the label is right, and the deciding fact is not in the state | **The question asks what the model cannot see** | The fact belongs in the state, computed by the caller, or the question should not be asked (`when`). |
| Several misses swap the same two options or adjacent levels | **The outcomes overlap** | Sharpen both criteria against each other, merge the options, or use fewer levels. Re-record. |
| The miss's right answer is an outcome few or no other cases label | **A coverage gap** | Add cases for that outcome before reading anything into the bar. |
| None of the above: the label is right, the question is clear, and the model is confidently wrong once | **The model** | Record it as a known miss. One such case does not move a bar or justify rewriting a question. |

Each miss has exactly one cause in the whole reply: a miss triaged as a
wrong label is not counted again as overlap or a coverage gap later on.

Report the triage as a table (case, question, expected, predicted,
confidence, cause, proposed action). Propose; do not apply. A label or a
question is the user's to change, and a question change costs a
re-recording.

What a proposed fix would do to a bar is measured, never guessed. A
relabel costs nothing to try. Read the cases document and Write it, with
only that label changed, to a directory of its own outside the project (see
"How to run them"). Run `tune` on the copy against the same recordings,
with the same `--target-accuracy` and `--min-covered` as the proposal you
are weighing; if you weigh two targets, run the copy at both. Report the
bar it gives, per target. What a sharper question would do cannot be
measured without re-recording: say so instead of giving a number.

## Coverage

`jud eval` does not count labels per outcome; count them from the cases
document. For each question, list how many cases label each option, each
level, `true` and `false` (or each `from_turn`). Name the cases behind every
count of 3 or fewer (`none_of_these 2: thanks, receipt`). Each question's
counts add up to its `labelled` in the eval JSON; if they do not, recount
before reporting. After a proposed relabel, move exactly the relabelled
cases from one outcome to the other. After a split, count the tuning set
and the held-out set apart. Say nothing about what an outcome's cases are
like that the named cases do not show. Flag:

- an outcome no case labels, so the model is never tested on it;
- a Noul with no `false` cases, so its threshold can never learn to say no;
- a way-out option (`none_of_these`) that is never the right answer, so the
  fallback was never measured;
- a question labelled on under about two thirds as many cases as the
  others (through `when`, or labels left out). Its own
  `accuracy_interval95` already reflects its count: quote that. Say nothing
  when the counts differ by one or two.

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
- **Keep the written bar** when the proposal is `0` on few cases (say why
  in these words: a bar of 0 defers nothing, so every low-confidence answer
  acts, and N cases with no miss cannot show the model never needs
  deferring), when it sits on a single case's confidence, when
  the cases are too few to trust, when it was set by misses you triaged as
  wrong labels or overlapping outcomes, or when it waits on a label the user
  has not confirmed. Prepend the reason to the gate's note, for example
  `kept at 0.30: tune proposed 0.00 on 6 cases, too few to drop the bar;`
  followed by what the note said before.
- **Re-run with another `--target-accuracy`** whenever the proposed bar
  covers less than about half the labelled cases, or you would call it too
  aggressive. Each run is free. Show the trade-off in one line: at 0.95 the
  bar is 0.80 and covers 15 of 48; at 0.90 it is 0.55 and covers 45.
- **Say what set a bar only after checking.** A Choice or Score bar is the
  lowest row of the table that reaches the target. A miss moved it only if
  the next lower row reaches the target once that case counts as right;
  read the row, or measure with a what-if copy. When the bar does not move,
  read the next lower row (the one just below the proposed bar: 0.75 under
  0.80). Its misses are covered minus correct; name exactly that many
  cases, each with a confidence at or above that row's bar and below the
  proposed one (case, expected, predicted, confidence). They hold that row
  under the target. Then say what would move it: those cases answered right,
  after a relabel if the label is wrong, after a re-record if the question
  is.
- **A bar that only a relabel would move** was set by a wrong label: keep
  the written bar until the user confirms the relabel, or propose the
  relabel and the bar together.
- **Pass on** the warnings `tune` printed, and only those: `jud: warning:`
  lines, the bar of 0, the strict-gate note (an answer exactly at the bar is read the
  other way) and the Score note (the table reads the most probable level,
  the policy the level nearest the weighted score). Both say the table may
  differ slightly from what the policy will do. The before and after `eval`
  shows what it actually does.

Never chase 100 %. A bar that makes every labelled case right on a few
dozen cases has learned those cases.

## Applying a proposal

Apply when the user asked for the bars to be changed: `/jud:tune ...
apply`, or in so many words ("fix the thresholds", "tune it"). That is not a
request to accept every proposal. Before applying:

1. With 40 or more labelled cases, split first (see "Do not grade on what
   you tuned on"), tune on the tuning set, and report the held-out numbers.
2. Run the accept, keep and re-run rules gate by gate. A Choice or Score
   bar set by misses you triaged as wrong labels or overlapping outcomes is
   kept, whatever the user asked: a bar cannot fix either. Say so, and
   propose the label fix or the sharper criterion instead.

Then apply every gate you accept, keep the others with a reason, and show
the before and after. A label or a question is never applied with the bars; propose it
separately. When the user only asked what the bars should be, show the edit
as a unified diff and offer to apply it.

Edit the rubric in place. `--out` writes a new file but serialises it
again, so every comment and the layout are lost. Use it only when the user
wants a separate file.

1. For each accepted gate whose value moves, change only the tuned value
   (`threshold`, `confidence`, `level_at_least`) and its `note`. In the
   note, replace what the run makes stale ("a guess", "no run behind it")
   with what `tune` printed, and keep what says what the gate does or whom
   it affects: `lowest bar at 95% accuracy; covers 29 of 36 labelled cases;
   high and critical page the on-call agent`. Keep `fallback`, `bands`,
   `strict` and every comment as they are. A gate whose proposal equals
   what is written (`propose 0.55 (now 0.55)`) keeps its value. Its note
   keeps what it says, except a part the run makes stale ("a guess", "no
   run behind it"), which becomes what `tune` printed. Otherwise the note
   would contradict the `tuning` block.
2. Replace or add `spec.tuning` with the block `tune` printed, indented two
   spaces under `spec:`, values unchanged. `cases`, `model`, `server`,
   `tuned_at` and `labelled` describe the run and are never written by hand.
   If you kept some gates, the block stays: it records the run the other
   gates came from.
3. Check: `jud.sh check RUBRIC CASES` must say `0 refused`. The questions
   fingerprint must be the one it was before your edit: you changed only
   the policy.
4. Show before and after: run `jud.sh eval RUBRIC CASES --replay DIR`
   before the edit and again after it. Keep the outputs in the conversation; write
   no copy of the rubric and no output file into the user's project. Compare
   each gate's acts, defers and accuracy when acted. The model's accuracy
   does not move, since only the policy changed; say so if someone expects
   it to.

A rubric that already had a `tuning` block from other cases gets the new
block, not a merge. If the cases fingerprint in the new block differs from
the old one, say that the bars now rest on different cases.

## Do not grade on what you tuned on

A bar tuned on the same cases it is graded on looks better than it will do.
Count the labelled cases before splitting. At 40 or more, hold some out:
48 gives 36 to tune on and 12 held out. Never say there are too few at 40 or
more. When the user did not ask for split files (`/jud:tune ... holdout`),
write the two documents to a temporary directory of their own, not the
project, and still report the held-out numbers. If bars tuned that way are
applied, the `tuning` block names a tuning set the project does not keep,
so a later `eval` on the full cases cannot tell that three quarters of them
were tuned on: say so, and recommend keeping the split files. Count each question's outcomes in both
halves, and name the cases behind every held-out count of 3 or fewer: a
held-out number on 2 billing cases says little about billing.

1. Split the cases into two documents: a tuning set and a held-out set,
   every outcome in both. Take every fourth case into the held-out set
   unless the user has a better split. Give each document its own
   `metadata.name`, and name the files after the cases file
   (`support-cases.jud` gives `support-cases-tune.jud` and
   `support-cases-holdout.jud`). `jud` has no split command: copy each case
   block verbatim. Then check that the two counts add up to the original,
   `jud.sh check` each with the rubric, and `eval --replay` each. The
   states are unchanged, so the recordings already in DIR answer both and
   nothing is re-recorded; a state copied wrong shows as `no recording
   answers`.
2. Tune on the tuning set and apply.
3. Grade the tuned rubric on the held-out set with
   `eval --replay DIR`. Its gate numbers are the honest ones: put them
   first, one sentence per gate ("expect about 10 of 12 acted, 2 deferred,
   1.00 when acted, on 12 held-out cases"), and label the tuning-set numbers
   as optimistic.

Below 40, say that there are too few to hold any out, and that the numbers
are optimistic.

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
- Spend calls without saying how many first, in a message before the call:
  `record`, `eval` without `--replay`, or `record --refresh`.
- Mix two models' recordings in one directory. `tune` refuses them (`the
  recordings come from more than one model`); record each model into its own
  directory and compare the two `eval` reports.
- Present a bar read off a handful of cases as anything but a guess with a
  number on it.

## What the reply contains

A reply that ran `eval` or `tune` has items 2 to 6, always, for every
question in the rubric, even when the user asked about one gate: answer
their question first, then give the items. One triage row per line under
`model misses` (with more than 10, group the rows by cause, but name every
case), and counts for every question. Items 1, 6
(for `eval` alone) and 7 depend on what was done. In this order:

1. The cost, before anything was spent (`record`), and what was spent.
2. One line per question: accuracy with its interval, whether confidence
   separates right from wrong, what the gate does (acts, defers, accuracy
   when acted).
3. Whether the labelled count supports a bar, and how many more cases each
   question needs, for which outcomes.
4. The triage table, a row for every miss.
5. Coverage: the thin outcomes, with the cases behind them.
6. For `tune`: the decision table (gate, primitive, now, proposed,
   decision, reason) and the warnings `tune` printed, in plain words, apart
   from your own observations.
7. What changed: the before and after per gate, held-out first; or the
   exact edit as a unified diff when nothing was applied. If no file
   changed, say so.
8. The next steps, ranked, each with its cost: free (labels, bars),
   calls for new cases, or a full re-recording (questions).

Before sending it, check every number against the output it came from: a
count adds up to its total, a width is `high - low` from the JSON, and a
statement of what a change would do names the run that measured it and the
target it ran at ("at 0.95: still 0.80"), and for every other target you
discussed, gives the measured bar or says "unmeasured at 0.90". Never write
that a change "won't move any bar" unless the copy ran at every target you
weighed. An announced number repeated in the reply is the number that was
announced.
