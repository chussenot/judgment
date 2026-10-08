---
description: Propose each gate's bar for a .jud rubric from recorded answers (jud tune), decide gate by gate whether to accept it, and with "apply" write it into the rubric in place, comments kept, with a before-and-after eval
argument-hint: "<rubric> <cases> <recordings dir> [apply] [--target-accuracy X] [--min-covered N] [holdout]"
---

Tune the bars of this rubric on its recorded answers:

$ARGUMENTS

Follow the jud-tune skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`
first, in particular "Reading `jud tune`", "Applying a proposal" and "Do not
grade on what you tuned on".
Every `jud` command below runs through the plugin's script, written out in
full each time, never through a shell variable or a `cd`, so that a
permission rule for the script matches it.

1. Resolve the arguments. The rubric and the cases document are required.
   The recordings directory is the one given, else
   `recordings/<rubric metadata.name>/` beside the cases document when it
   exists. If there are no recordings, say so and offer `/jud:record`; `tune`
   never calls a model.
2. If the arguments ask for `holdout`, or there are 40 or more labelled
   cases and the user wants to know how the bars will do, split the cases as
   the skill says. Name the files after the cases file: `support-cases.jud`
   gives `support-cases-tune.jud` and `support-cases-holdout.jud`, beside it,
   with `metadata.name` the cases document's name plus `-tune` and
   `-holdout`. Check both with the rubric, and tune on the tuning set.
3. Run `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh tune <rubric> <cases> --replay <dir>` with any
   `--target-accuracy` or `--min-covered` given. Keep stderr (the tables and
   warnings) and stdout (the `policy:` and `tuning:` blocks) apart.
   - Exit 1 with a missing recording means the recordings are stale; explain
     it as `/jud:eval` does and stop.
   - Exit 2 naming more than one model means the directory mixes models; say
     so and stop.
   - `nothing was proposed` means there is nothing to apply; say why from
     the per-gate lines.
4. Decide each gate as the skill says (accept, keep the written bar, or
   re-run with another target) and give the reason in one line. Read the
   misses (`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh eval ... --replay <dir>`) before accepting a bar that moves
   a lot: a bar that moved to absorb a wrong label is the label's fault.
5. Without `apply` in the arguments, stop here. Show a table (gate,
   primitive, now, proposed, decision, reason) and the exact edit you would
   make to the rubric, then offer to apply it. Do not change any file.
6. With `apply`:
   - run `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh eval <rubric> <cases> --replay <dir>`
     before you edit: that is the before (on the held-out set too, when there
     is one). Write no copy of the rubric and no output file into the project;
     keep the numbers in the conversation;
   - edit the rubric in place as the skill says: the tuned values and notes
     of accepted gates, a `note` on each kept gate, and the `tuning` block
     exactly as `tune` printed it, under `spec:`. Keep every comment;
   - run `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh check <rubric> <cases>`. It must say `0 refused`, with the
     same questions fingerprint as before;
   - run the same `eval` on the edited rubric, and show each gate's acts,
     defers and accuracy when acted before and after. With a held-out set, also grade the edited rubric on
     it and report those as the numbers to expect.
7. Reply with the decision table, the before and after, every warning `tune`
   printed (thin data, a bar of 0, strict, Score) in plain words, and what
   would make the bars trustworthy: how many more cases, for which outcomes.

Never write a `tuning` block, a fingerprint or a model that `tune` did not
print, and never use `--out` onto the rubric itself.
