---
description: Grade a .jud rubric's recorded answers against its labelled cases (jud eval), explain accuracy, calibration and the gates, and triage every miss into label, question, bar, coverage or model
argument-hint: "<rubric> <cases> [recordings dir] [--min-accuracy X]"
---

Grade this rubric on its cases and say what to change:

$ARGUMENTS

Follow the jud-tune skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`
first, in particular "Reading `jud eval`", "Triage every miss" and
"Coverage".
Every `jud` command below runs through the plugin's script, written out in
full each time, never through a shell variable or a `cd`, so that a
permission rule for the script matches it.

1. Resolve the arguments. The rubric and the cases document are required;
   if either is missing, say so and stop. The recordings directory is the one
   given, else `recordings/<rubric metadata.name>/` beside the cases document
   when it exists.
2. With no recordings directory, do not grade live unless the arguments say
   so (`live`). A live `eval` spends one call per case and keeps nothing.
   Say that, and offer `/jud:record` instead.
3. Run `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh eval <rubric> <cases> --replay <dir>`, adding any
   `--min-accuracy` given. Also run it with `--json` when you need exact
   numbers.
   - Exit 3 means a bar in `--min-accuracy` was not met. That is a finding,
     not an error.
   - Exit 1 with `no recording answers N cases: ...; record them first`
     means the recordings are stale. Run `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh check <rubric> <cases>
     <dir>/*.jud`: a recording whose request changed shows `fingerprint
     differs`. When every recording differs, a question changed (`git diff`
     on the rubric shows which); when one does, that case's state or options
     did. Say which change made them stale and how many calls re-recording
     costs. Then stop: do not edit recordings or revert the rubric.
4. Read the cases document and count, per question, how many cases label
   each outcome.
5. For every miss, read the case's state and note, and the question's
   criteria, and put the miss in one row of the skill's triage table.
6. Reply, in this order:
   - one line per question: accuracy with its interval, whether confidence
     separates right from wrong, and what the gate does (acts, defers,
     accuracy when acted);
   - whether the labelled count supports a bar, using the skill's interval
     table;
   - the triage table: case, question, expected, predicted, confidence,
     cause, proposed action;
   - coverage gaps;
   - the next steps, ranked. For example: fix these labels (free), sharpen
     this criterion (re-record N calls), add cases for these outcomes, then
     `/jud:tune`.

Do not change any file. Proposed label and question changes go in the
reply for the user to accept.
