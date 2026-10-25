# Before spending calls: counting a `jud record` run

Part of the `jud-tune` skill (`${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`), which says when to read this page; its rules on running commands ("How to run them") and its reply checklist ("What the reply contains") apply here too.

## Before spending calls

`jud record` is the only step that costs money, and it asks the backend
`jud config` names. Before running it, ask `jud` what the run would do:

```sh
${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh record RUBRIC CASES --out DIR --dry-run
```

It asks nothing, writes nothing and needs no key, and prints the requests
(a conversation labelled with `from_turn` counts once per turn), how many
DIR already answers, the names to ask and the stale ones to replace, the
backend and model with where the model came from, whether a key is set,
and the time at the median pace of the recordings already in DIR. To know
which recordings answer, it loads every image the documents declare
(`assets`, by paths relative to each file), so a file that is missing, is
not a PNG or a JPEG, or is outside the API's limits fails the plan by the
entry's name before any call; `check` reads the documents without their
files, so this is where a bad path first shows. Read the plan and say:

- **How many requests, and how many are new.** The run asks the `to ask`
  and `stale` names and keeps the rest; with `--refresh`, all of them.
  When some are stale, find what changed (`git diff -- RUBRIC CASES`, or
  `git log -p -1 -- RUBRIC` for a committed change) and name it.
- **Which backend and model.** `(from default)` means nothing chose the
  model; `jev-latest` is an alias, and the recordings will name the version
  that answered. A rubric with a Box needs Jev 1.14 or later: on the
  hosted API `jev-latest` and `jev-preview` both point to `jev-1.14.0`
  (as of 2026-10-25), and a model pinned to `jev-1.13.0` refuses every
  such request, one failed call per case (observed on 2026-10-15 as a
  400 with a bare `Invalid request.`, while `jev-latest` was 1.13).
  Bars tuned on one release stay meaningful only on it, so pin the
  version (`model: jev-1.14.0`) rather than an alias that moves when the
  next release ships; `jev-preview` can move ahead of `jev-latest`. Images
  alone (a Noul, a Choice or a Score about one) are answered by Jev 1.13
  and later.
- **Whether a key is set.** A missing one stops the run before the first
  call; any non-blank word does for a local server that ignores it. The
  user sets it outside the conversation: `export TYPESAFE_API_KEY=...` in
  their shell (or a gitignored `.env` that mise or direnv loads; `jud`
  itself reads no `.env`), or `api_key` in the configuration file `jud
  config` names. Never print it or ask for it.
- **How long.** A server on the user's machine (Ollama, laya-serve, a
  `base_url` on `127.0.0.1` or `localhost`) costs no money but wall time:
  quote the `time:` line. A hosted backend's cost is the calls.

A `jud` that refuses `--dry-run` as unknown predates it: count by hand
then. The requests are one per case, `len(state)` for a conversation
labelled with `from_turn`; `jud.sh check RUBRIC CASES DIR/*.jud` says which
recordings still answer (`fingerprint matches`) and which are stale
(`differs`); `jud.sh config` names the backend, the model and the key's
source; `spec.elapsed_ms` in a recording gives the pace.

Say the cost to the user before the `record` call: as the Bash call's
description, which shows on the call, and as text just before it,
for example `Recording 18 requests (30 already in recordings/support) to
https://api.typesafe.ai, model jev-latest.`, or, on a local server,
`Recording 93 requests to http://127.0.0.1:11434, model tev1:0.8b, about
9 s each (14 min).` A final reply after the run is
too late. If the user asked for this run in so many words (`/jud:record`
does), run it after that message; otherwise stop at the count and ask. A
run that fails part way keeps what it wrote, so the same command resumes.
