# Triage every miss

Part of the `jud-tune` skill (`${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`), which says when to read this page; its rules on running commands ("How to run them") and its reply checklist ("What the reply contains") apply here too.

## Triage every miss

The numbers say how often; the misses say why. For each miss, read the
case's `state`, its `note`, the question's instructions and criteria, and
the confidence. Then put it in exactly one row:

| What you see | Cause | What to do |
|---|---|---|
| The state plainly supports the model's answer, or the case's own `note` argues for it | **The label is wrong** | Propose the corrected label to the user, with the reason. Never change a label just because the model disagreed: the model being confident is not evidence, and a low-confidence miss on a label the criteria plainly support is not a label problem. |
| A Choice or Score answer under the `confidence` bar written in the rubric being graded (at the bar too, for a `strict` gate; a non-strict gate acts on an answer exactly at its bar). This comes first, even for a Score miss that swaps adjacent levels | **The bar's job** | Nothing for the gate: it defers this to its `fallback`. Say so. It is still a wrong answer: it counts in `accuracy` and against any `--min-accuracy` floor, and only a sharper question or another model removes it. |
| A Noul whose probability falls on the label's side of `threshold` (0.52 under a 0.55 threshold, labelled no) | **The bar's job** | Nothing: a threshold never defers, it decides, and here it decides the label. Say so. |
| Confidence is high, the label is right, and the criteria could be read the model's way | **The question is ambiguous** | Propose a sharper criterion: name the deciding detail. This changes the request, so re-record after. |
| Confidence is high, the label is right, and the deciding fact is not in the state | **The question asks what the model cannot see** | The fact belongs in the state, computed by the caller, or the question should not be asked: `when: <state path>` asks it only when that path is present in the state. `when` cannot name another question or depend on its answer. |
| Most of the model's answers to the question, right or wrong, are one option or level (the `outcomes` counts and the `collapsed` warning, `eval.md`) | **The question has collapsed** | Not a bar and not a label. Sharpen every outcome against the others, move what the state's machine-written fields already decide into code, split the question into narrower ones, or try another model. Re-record. |
| Several misses swap the same two options or adjacent levels | **The outcomes overlap** | Sharpen both criteria against each other, merge the options, or use fewer levels. Re-record. |
| The miss's right answer is an outcome few or no other cases label | **A coverage gap** | Add cases for that outcome before reading anything into the bar. |
| None of the above: the label is right, the question is clear, and the model is confidently wrong once | **The model** | Record it as a known miss. One such case does not move a bar or justify rewriting a question. |
| A Box under the `confidence` bar written in the rubric being graded | **The bar's job** | As for a Choice: the gate defers it, it still counts in `accuracy`. Say so. |
| A box on the right thing, overlap just under the label's bar (the box too loose or too tight on one side) | **The label's bar** | The bar is the label's `iou_at_least`, not the gate's, and `tune` never moves it. Say whether the task tolerates that box (a crop with a margin does; an alignment does not); the user raises or lowers the bar in the label, per case or set-wide, and says why in its `note`. A small target halves its overlap on a few pixels of offset: say so when the box is small. |
| A box on something else in the image (overlap near 0), confidently | **The question is ambiguous**, or **the model** | Name the distractor the box landed on; if the instructions could be read as naming it (a square where a circle was asked), sharpen them and re-record; if not, the model. |
| A box on an image where the thing is not there | **Not the Box's miss** | A Box always answers with a box; the case leaves it unlabelled and labels the Noul beside it `false`. Read the Noul's row for that case, and read the Box's confidence there: near zero is the model saying there was nothing, which the gate catches. |

A miss that only a proposed bar would defer is triaged on its own merits
(overlap, ambiguity, the model); the decision table then says the proposal
defers it.

Each miss has exactly one cause in the whole reply, in every sentence,
summary and next step: a miss triaged as a wrong label is not counted again
as overlap or a coverage gap later on, and "N misses sit on the border" counts
only those triaged so, taken from the triage table.

Report the triage as a table (case, question, expected, predicted,
confidence, cause, proposed action). Propose; do not apply. A label or a
question is the user's to change, and a question change costs a
re-recording.

What a proposed fix would do to a bar is measured, never guessed. A
relabel costs nothing to try. Read the cases document and Write it, with
only that label changed, to a directory of its own outside the project (see
"How to run them" in `SKILL.md`). Run `tune` on the copy against the same recordings,
with the same `--target-accuracy` and `--min-covered` as the proposal you
are weighing; if you weigh two targets, run the copy at both. Report the
bar it gives, per target. What a sharper question would do cannot be
measured from the recordings, which answered the old question: say so
instead of giving a number. It can be measured by recording again, which
is the user's call. When they ask for it (or the backend is a local server,
whose cost is only time, and they asked to investigate), measure on a
subset: Write an edited copy of the rubric and a cases document holding a
few cases per outcome to a directory of their own outside the project,
announce the requests as for `record` (`record.md`), record them into a recordings
directory of their own, and `eval` the copy. Report it as a subset of N
cases with the change named ("`content` moved last, 18 cases: still 3
right"), never as the rubric's accuracy, and leave the project's rubric,
cases and recordings untouched.
