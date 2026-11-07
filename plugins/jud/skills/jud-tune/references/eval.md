# Reading `jud eval`, and coverage

Part of the `jud-tune` skill (`${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`), which says when to read this page; its rules on running commands ("How to run them") and its reply checklist ("What the reply contains") apply here too.

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
- **A Box question's line.** `labelled` counts the cases with a box label:
  an image without the thing labels the Noul beside it `false` and leaves
  the Box unlabelled, so it is in the Noul's counts and the gate line, not
  in the Box's accuracy. `correct` is a box whose intersection over union
  with the label reaches the label's bar (`iou_at_least`, 0.5 when the
  label does not say); `mean IoU` stands where a Brier score would (a Box
  has no distribution to score; `brier -` is right, not missing);
  `calibration error` and the confidence when right and wrong read as for
  any question, the confidence against the hits. `outcomes` has the one
  outcome `box` and `majority box, L of L (1.00)` says nothing: do not
  compare the accuracy with it. The gate line covers every answer, the
  unlabelled empty images included: `acts on 23 of 40, defers 17` with 30
  labelled means the ten empties were deferred, which is what a confidence
  bar on a Box is for. Each miss prints the overlap it reached (`a box at
  IoU 0.24 with the label, confidence 0.70`), the number the triage reads.
- **`majority OUTCOME, M of L (SHARE)`.** What a model that always gave
  the commonest label would score (`majority.share` in the JSON). Put it
  beside the accuracy. A `not shown to beat always answering ...` warning
  (`no_better_than_majority`) means the interval reaches down to it: say
  so before anything about bars. It is never raised under ten labels, where
  even a perfect score cannot clear a common majority: on fewer, compare the
  two numbers yourself and say the count is too small to tell them apart. With 71 of 84 labels `false`, 0.96 is 0.11
  better than always answering no, not 0.96 better than nothing; at 0.63
  against 0.66, the model does worse than always answering the commonest
  level.
- **Tuned on these cases?** If the rubric's `spec.tuning.cases` equals the
  `cases.fingerprint` in the JSON, the bars were tuned on the cases being
  graded, so the gate numbers are optimistic (`tune.md`, "Do not grade on what
  you tuned on"). It says nothing about the recordings: a replay that exits 0 or
  3 is what shows they still answer.
- **`brier`, `calibration error`, `confidence when right, when wrong`.** Read
  these as one question: does the model's confidence separate its right
  answers from its wrong ones? A bar can only defer what it can tell apart.
  `calibration error` is printed twice, and the JSON has two keys: `ece`,
  over the wire's `confidence`, the number the gates compare, and
  `ece_probability`, over the probability of the predicted answer (a
  Noul's `max(p, 1 - p)`, a Choice's or Score's probability of its pick;
  `null` for a Box). Quote `ece` for a question about a bar; quote
  `ece_probability` when comparing models or servers, since it means the
  same thing across primitives and across servers whose `confidence` does
  not (Laya, Ollama). A gap between the two on one server says its
  `confidence` is not the pick's probability.
  - When right 0.85 and wrong 0.45: a bar between them will work.
  - When right 0.80 and wrong 0.78: no bar helps. The fix is in the question
    or the cases, not the policy.
  - `-` for "when wrong" means no misses, so nothing was learned about what a
    miss looks like.
  - In the JSON these are `confidence_when_right` and
    `confidence_when_wrong`, `null` where the text prints `-`.
- **`gate: acts on x of n, defers d, accuracy when acted p`.** This is the
  policy's operating point on these cases: how often it decides by itself,
  and how often that decision is right. Deferring sends the case to the
  gate's `fallback`, or to a person. Raising a bar trades acts for accuracy.
  Which side matters is the user's call: a refund sent wrongly costs more
  than a ticket routed to a human.

  A `the gate defers D of T` warning is not a bar doing its job. At nine in
  ten or more (`defers_nearly_all`) the policy hands the whole question to
  its `fallback`, which the warning names; at half or more
  (`defers_most`) it hands over most of it. Say what that does in production
  in one line ("every answer falls back to `possible`, so every change goes
  to a patroller"), and look at confidence when right and when wrong before
  proposing any other bar. A bar `jud tune` proposes can land here: before
  accepting one, check what share it defers, and refuse a bar that would
  raise either code unless the user asked for that coverage.
- **`model misses`.** Every case where the model's answer differs from the
  label, with its confidence. A Noul's is the probability of its answer; a
  Score's levels are printed as indices, 0 being the first level in
  `criteria`, so map them to names first. Triage every one (`triage.md`).
- **`outcomes, labelled/answered/right`.** Per outcome (`outcomes` in the
  JSON): how many labels name it, how often the model gave it on a labelled
  case, how often rightly. An outcome answered far more often than it is
  labelled is where the misses come from; one labelled `0` is never tested.
  A `collapsed` warning means one answer takes most of them while the labels
  are spread (a Choice answering `content` 71 times in 78, a Score staying
  at level 0): the model is not telling the outcomes apart, and the misses
  are one cause, not many. Say so first, with the counts the warning gives,
  then triage the rows under it. No bar and no relabel fixes a collapse; a
  sharper question, outcomes the state can tell apart, or another model can.
  A `never answered: ...` warning (`never_answered`) names outcomes three
  labels or more ask for that the model never gives. On a skewed set it is
  the collapse `collapsed` cannot see (always `yes` on a set that is 88 %
  `yes`); read it the same way, and name the outcomes it lists.

`--min-accuracy 0.9` (or `desk=0.95`) makes `eval` exit with status 3 when a
question's accuracy falls short. That is how a rubric is held in CI: the
recordings are committed, and a CI job runs the same `eval --replay ...
--min-accuracy` and fails the build on status 3 (`docs/guides/run-in-ci.md`
in the judgment repository). The floor holds `accuracy` (every labelled
case), never accuracy when acted: a deferred miss still counts. The policy is
never sent, so no bar or fallback can raise it; only a fixed label, a
sharper question or another model can. When a question falls short, say what
would raise it, with the arithmetic (with 50 of 60 right, relabelling one
case gives 51 of 60, 0.85, still short of 0.9, which needs 54).

`--fail-if-moved` is the second CI gate: status 3 when the model or the
server that answered is not the one the rubric's `tuning` block names.
Without the flag the report only says so, as a `warning:` line in the
text and a `moved` key in the JSON (`tuned_model`, `tuned_server`, and the
`models` and `servers` that answered), present only then. Read it before
any number: the bars were tuned on another model's answers, so the gate
numbers describe a policy nobody measured on these recordings. The way
out is `/jud:tune` on these recordings (the block then names this model)
or a recording on the tuned model; never an edit to the block.

Under a replay `eval` prints one `jud: warning:` line per request that two
files in the directory record, naming both, as `record` and `tune` do: the
report answers from one of them by name order, so pass the warning on and
name the stale file (`record --refresh` leaves an old file under another
name beside the new one).

A Choice whose options are the answers being compared (a pairwise judge)
can also be tested for bias: `eval --swap` asks each case again with the
options reversed, and `--tone-perturb FILE` with the labelled answer
rewritten in another tone (FILE: case id, question id, rewrite), and
`--injection-perturb FILE` with a note appended to the losing answer (FILE:
case id, question id, note), which adds an attack-success line below the
table: of the verdicts right as written, those the note turned wrong. The table
at the end of the report gives the flip rate, the flip rate among verdicts
at `--confident-at` or above, the mean change in probability and the
accuracy under each form. A confident flip is a verdict a gate would act on
that the perturbation moved: say so, and name the cases. The perturbed
requests are recorded with the same flags (`record --swap ...`), as
`CASE.swapped.jud`, `CASE.toned.jud` and `CASE.injected.jud` (`docs/guides/test-a-judge-for-position-and-tone-bias.md`
in the judgment repository). `references/bias.md` is the page for running
and reading them.

The thresholds a signal is raised at are in the JSON's `signal_rules`
(`min_answers` 10, `collapse_share` 0.8, `collapse_margin` 0.2,
`never_answered_min` 3, `defers_most` 0.5, `defers_nearly_all` 0.9).
Quote them when you explain why a signal was or was not raised, and never
raise one yourself on fewer answers than `min_answers`.

## Comparing two models

`eval --replay NEW --against OLD` sets every answer beside the one `OLD`
recorded for the same request (another model's, recorded into a
directory of its own) and adds an `agreement` block to the JSON and a
table to the text, per question: the pairs, how often the two picks
differ, the same among the answers at `--confident-at` or above (this
run's confidence), the mean |Δp| between the probability each put on this
run's pick, and each side's accuracy over the labelled pairs (`-` with no
label). A Box has no pick and is left out.

- **Agreement is not accuracy.** Two models can agree and both be wrong,
  so `--against` never gates and never changes the status, and it needs
  no label: it is the check to run when the model changes, before any
  case is labelled. The cases that differ are the ones to label first.
- **Every case as written needs a recording in `OLD`**, as under
  `--replay`; a missing one stops the run, naming the directory and the
  cases, before anything is asked. A recording is found by its request's
  fingerprint, which leaves the model out, so another model's recordings
  of the same rubric and cases are found.
- **Calibration across servers**: quote `ece_probability` for both sides
  (a second `eval --replay OLD --json`), not `ece`, whose `confidence`
  may mean different things on two servers.
- **The bars rest on one of them.** The rubric's `tuning` names the model
  its bars were read from; `moved` says which side that is not. After a
  switch: label the differing cases, `/jud:tune` on the new directory,
  then `--fail-if-moved` in CI once the block names it. `/jud:compare`
  runs this.

## Coverage

`jud eval` counts the labels per outcome: the first number of each
`outcomes` entry, every outcome the question offers listed, a `0` included.
Quote those counts; they add up to `labelled` by construction. Two things
it does not give, which come from the cases document: the names of the
cases behind every count of 3 or fewer (`none_of_these 2: thanks,
receipt`), and, for a conversation labelled with `from_turn`, which turn
each case names (`eval` counts that question per turn, as `yes` and `no`).
After a proposed relabel, move exactly the relabelled cases from one
outcome to the other. After a split, `jud split` prints each half's label
counts, or `eval` each half. Say nothing about what an outcome's cases are
like that the named cases do not show. Flag:

- an outcome no case labels, so the model is never tested on it;
- a Noul with no `false` cases, so its threshold can never learn to say no;
- a way-out option (`none_of_these`) that is never the right answer, so the
  fallback was never measured;
- a question labelled on under about two thirds as many cases as the
  others (through `when`, or labels left out). Its own
  `accuracy_interval95` already reflects its count: quote that. Say nothing
  when the counts differ by one or two. A Box is labelled only on the images
  that hold the thing, so its count is lower by design: say how many images
  do, and how many do not, since the second number is the Noul's `false`
  coverage and the Box's test of what it answers over nothing.
