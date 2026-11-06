---
description: Serve a directory of rubrics to Claude Code as MCP tools with jud mcp, after checking the binary has the command and every rubric reads; prints the claude mcp add line and runs it only when asked
argument-hint: "<rubrics dir> [--replay DIR] [--long-context] [add]"
---

Serve these rubrics as tools:

$ARGUMENTS

Follow the jud skill (`${CLAUDE_PLUGIN_ROOT}/skills/jud/SKILL.md`, "How
to run commands" of the Rust skill applies: one command per Bash call,
nothing chained). Run every `jud` command through
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`. The plugin starts no
server of its own: `jud mcp` is behind the crate's `mcp` feature, which
the release binaries do not carry, so an entry in the plugin would start
a server that half its users' `jud` cannot run.

1. **Does the binary have it?** `jud.sh mcp --help`. Clap's `unrecognized
   subcommand` with status 2 means it does not: say so, give
   `cargo install judgment --features mcp` (or a checkout built with
   `--features mcp`), and stop.
2. **Every rubric reads.** `jud.sh check <every .jud under the
   directory>` (Glob, recursively; cases and recordings in it are not
   tools and are checked with their rubric): `0 refused`. Two rubrics
   with one `metadata.name` refuse the server at start, since a tool is
   named by its rubric: name them and stop. A rubric with `assets` reads
   its images from its own directory on every call; say which do.
3. **The backend.** Without `--replay`, `jud mcp` asks the backend
   `jud.sh config` names and refuses to start with no key, so read the
   config and say which server and model every tool call will spend on;
   with `--replay DIR`, no key and no network, and a state nobody
   recorded is a tool error the agent reads, never a guess. Say which.
4. **The line.** Print, for the user to run:
   `claude mcp add jud -- jud mcp <dir> [--replay DIR] [--long-context]`
   (with the absolute path of `jud` when it is not on the user's PATH).
   With `add` in the arguments, run it, one command per Bash call, and
   say that the tools appear in a new session or after `/mcp`.
5. **Reply** with one line per tool: its name (the rubric's
   `metadata.name`), its input (`{state, options?}`: the JSON state, and
   per-request options for the Choices marked `options_from: request`),
   its result (the verdict map `jud RUBRIC` prints, as text) and what a
   failed call looks like (`isError` with `jud`'s own message). Then
   what `jud mcp` does not take: no `--asset` or `--pixels` per call.

Never write a `.mcp.json` or a settings file into the project; the
registration is the user's command.
