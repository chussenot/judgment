---
description: Propose each gate's bar for a .jud rubric from recorded answers (jud tune), decide gate by gate whether to accept it, and with "apply" write it into the rubric in place, comments kept, with a before-and-after eval
argument-hint: "<rubric> <cases> <recordings dir> [apply] [holdout] [--target-accuracy X] [--min-covered N]"
---

Tune the bars of this rubric on its recorded answers:

$ARGUMENTS

Follow the jud-tune skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`
first, in particular "Reading `jud tune`", "Applying a proposal", "Do not
grade on what you tuned on" and "What the reply contains".

Run every `jud` command through the plugin's script,
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`, written out in full, one
command per Bash call: no shell variable, no `cd`, nothing chained before or
after it, no redirection into a file. The tool result shows stdout, stderr
and a non-zero exit status. Read files with the Read tool, edit them with
the Edit tool. Write nothing into the project but the rubric (with `apply`)
and the two split files (with `holdout`). A what-if copy or a split you
were not asked to keep goes in a temporary directory of its own outside the
project, as the skill says, never beside the rubric.

1. **A single what-if question** ("what would the bar be if ...") gets its
   answer, not the whole flow. Run `tune` as is, and on the what-if copy, at
   the targets the question needs, then reply with the measured bars and
   what holds them or would move them, and stop. Offer the full tuning
   only after that.
2. **Resolve the arguments.** The rubric and the cases document are
   required. The recordings directory is the one given, else
   `recordings/<rubric metadata.name>/` beside the cases document when it
   exists. With no recordings, say so and offer `/jud:record`: `tune` never
   calls a model.
3. **Hold out** whenever the cases document has 40 or more cases (count
   before splitting: 48 gives 36 to tune and 12 held out). Skip it only
   for a what-if question about one bar, and then say the numbers are
   tuned on all cases. That skip covers the what-if runs only: an edit you
   build in step 7 is tuned on the tuning set, or you say that `apply`
   would re-tune on it and may propose other bars.
   - With `apply`, keep the split: without `holdout` in the arguments, say
     that the `tuning` block will name a tuning set the project does not
     keep, and recommend `holdout` so it does.
   - Split with `jud.sh split <cases>`, as the skill says: with `holdout`
     in the arguments beside the cases file (no `--out`); without it,
     `--out` a temporary directory of its own. It writes
     `support-cases-tune.jud` and `support-cases-holdout.jud` for
     `support-cases.jud`, named after the cases document plus `-tune` and
     `-holdout`, and prints each half's labels; pass on any warning that a
     half lacks a label.
   - Check both with the rubric. Then `eval --replay` both.
   - Tune on the tuning set.
4. **Grade first.** Run `jud.sh eval <rubric> <cases> --replay <dir>
   --json`, on the held-out set too when there is one. This is the
   *before*. Triage every miss into the skill's table.
5. **Tune.** Run `jud.sh tune <rubric> <cases> --replay <dir>` with any
   `--target-accuracy` or `--min-covered` given. The tables and warnings
   come on stderr; the `policy:` and `tuning:` blocks come on stdout at
   column 0.
   - Exit 1 naming cases no recording answers: the recordings are stale.
     Explain it as `/jud:eval` does, and stop.
   - Exit 2 naming more than one model: the directory mixes models. Say
     so, and stop.
   - `nothing was proposed`: say why, from the per-gate lines.
6. **Decide each gate** as the skill says: accept, keep the written bar,
   or re-run. Give a one-line reason for each.
   - Whenever a proposed bar covers less than half the labelled cases, or
     you would call it too aggressive, run `tune` again with a lower
     `--target-accuracy` (it is free). Show both in one line, for example
     "at 0.95: 0.70, covers 9 of 30 tuning cases; at 0.90: 0.45, covers
     24 of 30". After a split, every count is of the tuning set.
   - Before you say a miss (a wrong label, say) moved a bar, check it. Use
     the table: the bar moves only if the next lower row reaches the target
     once that case counts as right. Or run `tune` on a what-if copy of the
     cases (made with Read and Write, in the temporary directory) with the
     label fixed, at the same `--target-accuracy` and `--min-covered` as each
     proposal you weigh. Report what was measured, per target. When the bar
     does not move, name the misses that hold the next lower row under the
     target in the what-if run's own table (the relabelled case now counts
     as right), and what would move it.
7. **Without `apply`**, change no file in the project. Build the edit step
   8 would make on a copy of the rubric (Read then Write, in a temporary
   directory of its own): the policy values and notes in place, and the
   `tuning` block indented two spaces under `spec:`, never at column 0 as
   `tune` prints it, and comments as step 8 says. `jud.sh check` the copy with the cases: `0 refused`,
   same questions fingerprint. Run `eval --replay` on the copy (held-out
   set first, when there is one): that is the *after* the edit would give.
   Show the decision table, the before and after, and the diff of the copy
   against the rubric, made with one Bash call, `git diff --no-index --
   <rubric> <copy>` (it exits 1 when the files differ, which is expected),
   pasted as it printed. Ask whether
   to apply it.
8. **With `apply`:**
   - Edit the rubric in place as "Applying a proposal" says. Change the
     values of the gates whose value moves. A gate proposed at its current
     value keeps its value and its note, except a part the run makes stale
     ("a guess"), which becomes what `tune` printed. Replace the `tuning`
     block with the one `tune` printed whenever any gate is accepted (one
     proposed at its current value counts); leave it alone only when every
     gate is kept. Only a gate whose
     proposal you declined gets a `kept at <value>: <reason>;` prefix before
     its note; a gate proposed at its current value is accepted, not kept. Add the
     `tuning` block exactly as `tune` printed it. Keep every comment,
     except words the run makes false (a header's "hand-written guesses"),
     which are reworded the way a note's stale part is.
   - `jud.sh check <rubric> <cases>` must say `0 refused`, with the same
     questions fingerprint as before.
   - Run the same `eval` again: that is the *after*. Show each gate's acts,
     defers and accuracy when acted, before and after, with the held-out
     rows first when there are any.
9. **Reply** as "What the reply contains" says, for `tune`.

Never write a `tuning` block, a fingerprint or a model that `tune` did not
print. Never use `--out` onto the rubric itself.
