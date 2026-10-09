---
description: Write a .jud Rubric from a brief (the questions a model is asked and the policy that reads its answers), checked with the crate's reader
argument-hint: "[brief, or the path of a rubric to revise]"
---

Write a `.jud` Rubric for this brief, or revise the one at the path given:

$ARGUMENTS

Follow the jud skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud/SKILL.md` and
its `references/format.md` first, then work through its workflow. In
particular:

1. Turn the brief into narrow, atomic questions, one per thing the
   application needs to know and cannot compute itself. Give each outcome a
   criterion in plain words, name the state keys the application will send,
   and give a Choice a way out (`none_of_these`, `other`) when the state might
   fit none of its options. Keep every threshold out of the instructions.
   When real states are at hand (a sample, a log, the brief's examples), read
   them first: for each question, name the field its answer is read from
   (none means the caller must add one; say which), and leave to code what a
   machine-written field already decides.
2. Write the policy beside the questions: one gate per question, fitting its
   primitive, each with a `note` saying why the bar is where it is. A
   hand-written rubric has no `tuning` block; never invent one.
3. If the brief asks for something the format or honesty refuses (an id that
   is not a name, a threshold in the instructions, a scale past ten levels, a
   one-option Choice, a tuning block for a run that never happened), write
   what the format allows and explain each deviation in the reply.
4. Check the document with the reader before handing it over:
   `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh check <rubric>` (with the
   cases document too, when one exists). Fix every refusal in the document.
5. Reply with the path written, the `jud check` summary line and the two
   fingerprints (questions and policy), and offer `/jud:cases` for the labelled
   cases the thresholds will be tuned on.

If the brief names no output path, write the rubric next to the files it
concerns, as `<name>.jud`, and say where.
