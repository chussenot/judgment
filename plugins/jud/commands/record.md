---
description: Record a model's answers to every labelled case of a .jud rubric (jud record), after saying how many calls it will spend and to which backend
argument-hint: "<rubric> <cases> [recordings dir] [--refresh]"
---

Record the configured model's answers to these cases, so they can be graded
and tuned on without asking again:

$ARGUMENTS

Follow the jud-tune skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`,
in particular "What costs what" and "Before spending calls".
Every `jud` command below runs through the plugin's script, written out in
full each time, never through a shell variable or a `cd`, so that a
permission rule for the script matches it.

1. Resolve the arguments. The rubric and the cases document are required;
   if either is missing, say so and stop. The recordings directory is the one
   given, else `recordings/<rubric metadata.name>/` beside the cases document.
   Say which.
2. Check before spending: `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh check <rubric> <cases>` must say `0 refused`.
   A refusal means the request cannot be built, so report it and stop (offer
   `/jud:check`).
3. Count the requests: one per case, or one per turn for a conversation
   whose cases label a Noul with `from_turn`. Count the recordings already in
   the directory (`*.jud` files); `record` keeps them unless `--refresh` was
   asked, so say how many are likely new. Run `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh config` and read
   `base_url`, `model` and whether a key is set. Never print or ask for the
   key itself.
4. If no key is set, or `base_url` is unreachable, stop: say what is
   missing, and that `TYPESAFE_API_KEY` (or `api_key` in the file `jud
   config` names) and `TYPESAFE_BASE_URL` set it. Any non-blank key does for
   a local server that ignores it.
5. Invoking this command is the user asking for the run. State the count,
   the backend and the model in one line, then run `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh record <rubric>
   <cases> --out <dir>` (with `--refresh` only when asked). If the run fails
   part way, say what failed. The recordings written so far are kept, and the
   same command resumes.
6. Reply with: the summary line `record` printed (`recorded N, kept M in
   DIR`), the model the recordings name, and the next step. That is
   `/jud:eval <rubric> <cases> <dir>`, which grades them free of charge.

Do not edit the rubric, the cases or any recording.
