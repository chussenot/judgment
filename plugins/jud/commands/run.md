---
description: Evaluate one JSON state against a .jud rubric (jud RUBRIC) from a replay or the configured backend, and explain every verdict as the policy read it
argument-hint: "<rubric> <state: a JSON file, inline JSON, or a case id with --cases FILE> [--replay DIR] [--options JSON | --options-file FILE] [--asset REF=PATH ...] [--pixels WxH] [--long-context]"
---

Run this rubric on one state and explain what the policy decided:

$ARGUMENTS

Follow the jud skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud/SKILL.md`, in
particular "Running a rubric". Run every `jud` command through
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`, written out in full, one
command per Bash call, nothing chained before or after it. The state
reaches `jud RUBRIC` on stdin by input redirection from a file
(`jud.sh RUBRIC ... < PATH`), never through a pipe. This command writes
nothing into the project.

1. **Resolve the arguments.** The rubric is required. The state is a JSON
   file in the project, inline JSON, or a case id with `--cases FILE`
   (the case's `state`, and its `options` as `--options`, read from the
   document with the Read tool). Inline JSON or a case's state goes to a
   file of its own outside the project (the session's scratchpad or
   temporary directory, else `/tmp/jud-run-<rubric name>/state.json`),
   with the Write tool. If none is given, say so and stop.
2. **Check.** `jud.sh check <rubric>`, or `jud.sh check <rubric> <cases>`
   when `--cases` was given, must say `0 refused`; otherwise report it, offer `/jud:check`, stop.
3. **Show what is asked.** `jud.sh lower <rubric> --state-file <state>`
   (with `--options` when there are any): the questions the request
   carries, which ones a `when` left out for this state, which parts
   `part_when` dropped, and the options a Choice takes from the request.
   A refusal here is status 2 and names what the state lacks (options for
   a question that needs them, an image reference nothing declares): say
   what the caller must add, and stop.
4. **Announce a live call.** With `--replay DIR` (or `JUD_REPLAY`), say
   that the answer comes from the recordings with no key and no network,
   and that a state nobody recorded is status 1, never a guess. Without
   one, read `jud.sh config` and say, in the Bash call's description and
   as text just before it, `Asking <base_url>, model <model>, one request`
   (with the images and their sizes when `--asset` is given, since an
   image costs input tokens by its pixels). If the key is `missing`, stop
   and say how it is set (`/jud:record` step 6), never asking for it.
5. **Run.** `jud.sh <rubric> [--replay DIR] [--options ...] [--asset
   REF=PATH ...] [--pixels WxH] [--long-context] < <state>`. Stdout is the
   verdict JSON, one entry per question in the rubric's order; stderr may
   carry `jud: warning:` lines (an ignored `TYPESAFE_*` variable, a model
   or server other than the one the rubric's `tuning` names).
6. **Reply**, one line per question, in plain words, from the JSON:
   - `yes` or `no` with the probability, and the gate's `threshold` (and
     `strict`) it was read at;
   - `option` with its key, confidence and `band` when the gate has bands;
   - `level` with its label, index, weighted `value`, confidence, `band`,
     and `reached` when the gate has `level_at_least`;
   - `located` with the box in fractions and, when `--pixels` or the one
     image gave a size, in pixels; say that a Box answers with a box
     whether or not the thing is there, so the Noul beside it is read
     first;
   - `deferred` with `nearest` (the answer the model gave: an option key,
     a level's index, or a Box's edges as `top,bottom,left,right`), its
     confidence, the `bar` it was under and the `fallback` the policy
     names (`null` means nothing acts on it): the gate's job, not an
     error.
   Then every `jud: warning:` line in plain words, and a status-2
   refusal from the message (not one JSON value on stdin, an image
   outside the limits, options under a key the question already offers).
   End by offering `/jud:cases` when the user says the verdict was right
   or wrong: a state with its verdict is a labelled case.

Never edit the rubric, the cases or a recording; never write the state
into the project.
