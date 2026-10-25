# Reading `jud tune`, applying a proposal, holding cases out

Part of the `jud-tune` skill (`${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`), which says when to read this page; its rules on running commands ("How to run them") and its reply checklist ("What the reply contains") apply here too.

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
- **A Box's `confidence`:** as a Choice's, over the labelled cases alone
  (`covered` counts cases with a box label; the empty images are not in
  the table, so a bar cannot be read off them there). The overlap bar a
  box is right at is the label's `iou_at_least`, never proposed.
- A gate with `bands` gets its table and no proposal.

Decide per gate, and say which you decided:

- **Accept** when the labelled count is enough for the use (the interval
  table in `eval.md`), the proposal moves in a direction the misses explain, and
  coverage stays acceptable to the user.
- **Keep the written bar** when the proposal is `0` on few cases (say why
  in these words: a bar of 0 defers nothing, so every low-confidence answer
  acts, and N cases with no miss cannot show the model never needs
  deferring), when it sits on a single case's confidence, when
  the cases are too few to trust, when it was set by misses you triaged as
  wrong labels or overlapping outcomes *and* the step up defers right
  answers too, or when it waits on a label the user has not confirmed.
  Compare the written bar's row with the proposed one: if correct drops,
  the raise costs right answers to exclude misses a bar cannot fix, so
  keep it. If correct stays the same and only covered falls, the raise
  defers misses and nothing else: accept it, whatever caused the misses. Prepend the reason to the gate's note, for example
  `kept at 0.30: tune proposed 0.00 on 6 cases, too few to drop the bar;`
  followed by what the note said before. The reason must agree with what
  you measured: put a bar on a label only if the what-if copy moved it at
  that target; otherwise name the cases that hold the next lower row.
- **Check a Box's bar against the empty images.** The table does not see
  them. Before accepting a bar, read `eval`'s gate line on the full cases
  (`acts on x of n`): a bar under the confidence the model gives over
  nothing (the study's empties sat at 0.02 to 0.04) would let a box over
  nothing through on every empty image, which the Noul beside it must then
  catch; say which of the two gates stops them.
- **Re-run with another `--target-accuracy`** whenever the proposed bar
  covers less than about half the labelled cases, or you would call it too
  aggressive. Each run is free. Show the trade-off in one line: at 0.95 the
  bar is 0.70 and covers 9 of 30; at 0.90 it is 0.45 and covers 24. A bar at
  another target is the bar a `tune` run at that target printed, on the
  cases the decision rests on (the tuning set after a split).
- **Say what set a bar only after checking.** A Choice or Score bar is the
  lowest row of the table that reaches the target. A miss moved it only if
  the next lower row reaches the target once that case counts as right;
  read the row, or measure with a what-if copy. When the bar does not move,
  read the next lower row (the one just below the proposed bar: 0.65 under
  0.70), in the run you are reporting: in a what-if run the relabelled case
  counts as right. Its misses are covered minus correct. Name those whose confidence
  lies between that row's bar and the proposed one (case, expected,
  predicted, confidence): they are what the step down adds. Then say how
  many more right answers the row needs to reach the target
  (target x covered, rounded up, minus correct), and which of its misses
  could supply them. Then say what would move it: those cases answered right,
  after a relabel if the label is wrong, after a re-record if the question
  is.
- **A bar that only a relabel would move** was set by a wrong label: keep
  the written bar until the user confirms the relabel, or propose the
  relabel and the bar together. Mind the direction: a miss on a wrong label
  can only push a confidence bar up, so it never makes a proposal looser
  than it should be. When the proposal and the written bar fall on the same
  table row (same covered, same correct), say they are equivalent on these
  cases, and keep the written value for that reason, not because of a
  label. Lead with the reason for the target you rejected ("at 0.95: 0.80,
  covers 10 of 36").
- **The reason cell holds the reason**, in one line, never a pointer to
  another section.
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
2. Run the accept, keep and re-run rules gate by gate, and run the re-run
   and the what-if even for a gate you expect to keep: the reason quotes
   them. A Choice or Score bar raised to exclude confident misses you
   triaged as wrong labels or overlapping outcomes, at the cost of right
   answers, is kept, whatever the user asked: a bar cannot fix either. Say
   so, and propose the label fix or the sharper criterion instead. A raise
   that defers only misses is accepted.

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
   `strict` and every comment as they are, except words in a comment the
   run makes false ("the bars below are hand-written guesses"), which are
   reworded the way a note's stale part is. A gate whose proposal equals
   what is written (`propose 0.55 (now 0.55)`) keeps its value. Its note
   keeps what it says, except a part the run makes stale ("a guess", "no
   run behind it"), which becomes what `tune` printed. Otherwise the note
   would contradict the `tuning` block.
2. Replace or add `spec.tuning` with the block `tune` printed, indented two
   spaces under `spec:`, values unchanged. `cases`, `model`, `server`,
   `tuned_at` and `labelled` describe the run and are never written by hand.
   If you kept some gates, the block stays: it records the run the other
   gates came from. Replace the block whenever you accept at least one
   proposal, even one equal to the written value: the accepted bars now
   rest on this run. Write no new block only when you keep every gate; then
   leave the existing block and the comments alone.
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

1. Split with `jud`, which holds out every fourth case and copies each one
   as read:

   ```sh
   ${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh split CASES --out DIR
   ```

   `DIR` is the cases file's own directory when the user asked for split
   files (leave `--out` out), else a temporary directory of its own. It
   writes `STEM-tune.jud` and `STEM-holdout.jud` (`support-cases.jud` gives
   `support-cases-tune.jud` and `support-cases-holdout.jud`), each with its
   own `metadata.name`, prints each half's count, fingerprint and labels per
   question, and warns on stderr when one half lacks a label the other has:
   pass that warning on. It refuses to write over a file; never delete one
   to make room without the user's say. `--every N` holds out every Nth
   case when the user wants another share. The states are unchanged, so
   the recordings already in DIR answer both halves and nothing is
   re-recorded. A `jud` that refuses `split` as unknown predates it: then
   write the two documents by copying each case block verbatim, every
   fourth into the held-out set, and check that the counts add up and that
   `eval --replay` answers each.
2. Tune on the tuning set and apply.
3. Grade the tuned rubric on the held-out set with
   `eval --replay DIR`. Its gate numbers are the honest ones: put them
   first, one sentence per gate ("expect about 10 of 12 acted, 2 deferred,
   1.00 when acted, on 12 held-out cases"), and label the tuning-set numbers
   as optimistic.

Below 40, say that there are too few to hold any out, and that the numbers
are optimistic.
