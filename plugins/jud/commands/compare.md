---
description: Compare two models' recorded answers on one rubric (jud eval --against): where their picks differ, the confident share, how far the probabilities move, each side's accuracy and calibration, whether the rubric's bars were tuned on either, and the model-switch workflow that follows
argument-hint: "<rubric> <cases> <recordings dir A> <recordings dir B> [--confident-at TAU]"
---

Compare the two models' answers to this rubric's cases:

$ARGUMENTS

Follow the jud-tune skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`
("How to run them", "What costs what") and
`${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/references/eval.md` ("Comparing
two models"). Run every `jud` command through
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`, one per Bash call.
Read files with the Read tool. This command changes no file and makes no
call.

1. **Resolve the arguments.** All four are required; `A` is the side
   whose answers are graded as this run's, `B` the one set beside them.
   Say so.
2. **Check both directories** with the documents: `jud.sh check <rubric>
   <cases> <A>/*.jud` and the same for `B`: `0 refused`, every recording
   `fingerprint matches`. A case as written with no recording on either
   side stops here (status 1 under `--against` names the directory and
   the cases): offer `/jud:record` into that directory, with its cost.
3. **Name the models.** Grep `^\s+model:` in each directory and tally.
   A directory that mixes models stops here, as `tune` refuses it: say
   which cases each model answered.
4. **Compare.** `jud.sh eval <rubric> <cases> --replay <A> --against <B>
   --json`, with `--confident-at` when given. The flag takes one value
   for the whole run: without one, pass the `confidence` bar of the gate
   the user cares about (the Choice whose picks differ most, when they
   name none) and say which; with several gates at different bars, say
   that the confident column reads that one bar for every question.
   Once more without `--json` for the table's text.
5. **Reply**, from the JSON, per question:
   - the agreement table as printed: pairs, the share that differs with
     its interval and counts, the confident share (verdicts a gate would
     act on that the other model answers otherwise), mean |Δp|, and each
     side's accuracy over the labelled pairs when there are labels (`-`
     without);
   - the cases that differ, by name, with both answers and both
     confidences from the recordings, and the label when there is one:
     these are the cases to label first when there are none, since
     agreement is not accuracy (two models can agree and both be wrong);
   - `ece` and `ece_probability` for `A` (from `questions[]`), and for
     `B` from a second `eval --replay <B> --json`: say that
     `ece_probability` is the one that compares across servers whose
     `confidence` means different things;
   - the `moved` key: which of the two, if either, the rubric's `tuning`
     names; the bars were tuned on that one's answers, and `--min-accuracy`
     reads `A` only;
   - what the two `--min-accuracy`-free accuracies say when labels exist,
     with intervals, and that at these counts the intervals overlap or
     do not.
   End with the model-switch workflow, ranked by cost: label the cases
   that differ (`/jud:cases`, free of calls), `/jud:tune` on the new
   model's directory (free), then `--fail-if-moved` in CI once the rubric
   names it (`/jud:ci`).

Never a gate: `--against` changes no status. Never edit a recording, the
rubric or the cases.
