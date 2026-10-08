---
description: Record a model's answers to every labelled case of a .jud rubric (jud record), after telling the user how many calls it will spend, to which backend and model
argument-hint: "<rubric> <cases> [recordings dir] [--refresh]"
---

Record the configured model's answers to these cases, so they can be graded
and tuned on without asking again:

$ARGUMENTS

Follow the jud-tune skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`,
in particular "What costs what" and "Before spending calls".

Run every `jud` command through the plugin's script,
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`, written out in full, one
command per Bash call: no shell variable, no `cd`, nothing chained before or
after it (`;`, `&&`, `|`, `echo $?`), no redirection into a file. The tool
result shows the output and a non-zero exit status. Read files with the Read
tool.

1. **Resolve the arguments.** The rubric and the cases document are
   required; if either is missing, say so and stop. The recordings directory
   is the one given, else `recordings/<rubric metadata.name>/` beside the
   cases document.
2. **Check.** `jud.sh check <rubric> <cases>` must say `0 refused`. A refusal
   means the requests cannot be built: report it, offer `/jud:check`, stop.
3. **Count the requests.** One per case. A conversation whose cases label a
   Noul with `{from_turn: ...}` is recorded once per entry of its state
   array (`len(state)`, assistant entries included); sum over the cases.
4. **Count what is kept.** If the directory has recordings, run
   `jud.sh check <rubric> <cases> <dir>/*.jud`. A recording with
   `fingerprint matches` is kept; one with `fingerprint differs` is stale:
   `record` asks that case again and replaces the file. So the new requests
   are the total minus the matching recordings (the total with
   `--refresh`). When any is stale, find what changed: `git diff --
   <rubric> <cases>` as written (no `-C`, no `cd`), or `git log -p -1 --
   <rubric>` for a committed change. Name it in the announcement and the
   reply, for example "the `queue` question's `account` criterion now says
   plan changes and trials". All stale means a question changed; some, those
   cases did.
5. **Read the backend.** `jud.sh config` prints `base_url`, `model`,
   `model_from` and where the key comes from. It never prints the key, and
   neither do you. `model_from: default` means nothing set the model:
   `jev-latest` is an alias, and the recordings will name the version that
   answered. `jud config` does not contact the backend; do not probe it
   yourself.
6. **Stop if there is no key.** If `api_key` shows none, stop and report
   everything steps 1 to 5 found (directory, requests, kept, new, base_url,
   model) in one reply. Say that the key is set outside this conversation:
   `export TYPESAFE_API_KEY=...` in the shell (or a gitignored `.env` that
   mise or direnv loads; `jud` reads no `.env` itself), or `api_key` in the
   file `jud config` names, and that any non-blank word does for a local
   server that ignores it. Then re-run the command. Never
   ask for the key here.
7. **Announce, then record.** Before the `record` call, send the user this
   line as a message of its own: `Recording <new> requests (<kept> kept,
   <stale> stale and replaced, in <dir>) to <base_url>, model <model>.`
   Leave out the parts that are zero, and add `, one per turn for <n>
   conversations` when that applies. When any are stale, follow it with the
   change step 4 found: `Stale because the queue question's account
   criterion now says plan changes and trials.` Invoking this command is the user
   asking for the run, so then run `jud.sh record <rubric> <cases> --out
   <dir>`, with `--refresh` only when asked. If it fails part way, say what
   failed: what was written is kept, and the same command resumes.
8. **Reply** with the announcement line again, then `record`'s summary line
   (`recorded N, kept M in DIR`). If N differs from what you announced, say
   why. Give the model the recordings name, read from all of them with
   `grep -h "^    model:" <dir>/*.jud` and counted. If there is more than
   one model, say so: `tune` refuses a directory that mixes models. If the
   recorded model differs from `jud config`'s, say both. End with the next
   step, `/jud:eval <rubric> <cases> <dir>`, which grades them at no cost.

Do not edit the rubric, the cases or any recording.
