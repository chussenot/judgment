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
and the two split files (with `holdout`). A what-if copy goes in a
temporary directory outside the project (`$TMPDIR`, else `/tmp`), never
beside the rubric.

1. **Resolve the arguments.** The rubric and the cases document are
   required. The recordings directory is the one given, else
   `recordings/<rubric metadata.name>/` beside the cases document when it
   exists. With no recordings, say so and offer `/jud:record`: `tune` never
   calls a model.
2. **Hold out** whenever the cases document has 40 or more cases (count
   before splitting: 48 gives 36 to tune and 12 held out). Skip it only
   for a what-if question about one bar, and then say the numbers are
   tuned on all cases.
   - Split as the skill says. Name the files after the cases file:
     `support-cases.jud` gives `support-cases-tune.jud` and
     `support-cases-holdout.jud`; their `metadata.name` is the cases
     document's name plus `-tune` and `-holdout`. With `holdout` in the
     arguments, write them beside the cases file; without it, in
     `$TMPDIR/jud-holdout/` (else `/tmp/jud-holdout/`).
   - Check both with the rubric. Then `eval --replay` both, so that a
     state copied wrong shows up as `no recording answers`.
   - Tune on the tuning set.
3. **Grade first.** Run `jud.sh eval <rubric> <cases> --replay <dir>
   --json`, on the held-out set too when there is one. This is the
   *before*. Triage every miss into the skill's table.
4. **Tune.** Run `jud.sh tune <rubric> <cases> --replay <dir>` with any
   `--target-accuracy` or `--min-covered` given. The tables and warnings
   come on stderr; the `policy:` and `tuning:` blocks come on stdout at
   column 0.
   - Exit 1 naming cases no recording answers: the recordings are stale.
     Explain it as `/jud:eval` does, and stop.
   - Exit 2 naming more than one model: the directory mixes models. Say
     so, and stop.
   - `nothing was proposed`: say why, from the per-gate lines.
5. **Decide each gate** as the skill says: accept, keep the written bar,
   or re-run. Give a one-line reason for each.
   - Whenever a proposed bar covers less than half the labelled cases, or
     you would call it too aggressive, run `tune` again with a lower
     `--target-accuracy` (it is free). Show both in one line, for example
     "at 0.95: 0.80, covers 15 of 48; at 0.90: 0.55, covers 45".
   - Before you say a miss (a wrong label, say) moved a bar, check it. Use
     the table: the bar moves only if the next lower row reaches the target
     once that case counts as right. Or run `tune` on a what-if copy of the
     cases (made with Read and Write, in the temporary directory) with the
     label fixed, at the same `--target-accuracy` and `--min-covered` as each
     proposal you weigh. Report what was measured, per target. When the bar
     does not move, name the misses that hold the next lower row under the
     target, and what would move it.
6. **Without `apply`**, stop here. Show the decision table and the exact
   edit as a unified diff against the rubric, with the comments around it
   as context. Then offer to apply it. Change no file.
7. **With `apply`:**
   - Edit the rubric in place as "Applying a proposal" says. Change the
     values of the gates whose value moves. A gate proposed at its current
     value keeps its value and its note, except a part the run makes stale
     ("a guess"), which becomes what `tune` printed. Only a gate whose
     proposal you declined gets a `kept at ...:` reason before its note. Give kept gates a reason prepended to their note. Add the
     `tuning` block exactly as `tune` printed it. Keep every comment.
   - `jud.sh check <rubric> <cases>` must say `0 refused`, with the same
     questions fingerprint as before.
   - Run the same `eval` again: that is the *after*. Show each gate's acts,
     defers and accuracy when acted, before and after, with the held-out
     rows first when there are any.
8. **Reply** as "What the reply contains" says, for `tune`.

Never write a `tuning` block, a fingerprint or a model that `tune` did not
print. Never use `--out` onto the rubric itself.
