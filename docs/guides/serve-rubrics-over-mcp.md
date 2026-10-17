---
title: Serve rubrics over MCP
description: How to give an agent your rubrics as tools with jud mcp, a Model Context Protocol server over stdio that turns every Rubric under a directory into a tool named by the rubric, takes the JSON state and optional per-request options, and returns the verdict text jud RUBRIC prints; how to build it (the mcp feature), register it with Claude Code, run it with no key from recordings, and what a failed call looks like to the agent.
status: current
last_reviewed: 2026-10-16
tags: [judgment, jud, cli, mcp, agents, how-to]
---

# Serve rubrics over MCP

**You will** let an agent call your rubrics as tools: it passes a JSON state and gets back the verdicts, the same text `jud RUBRIC` prints. **Prerequisites:** a directory of rubrics ([Write a rubric](write-a-rubric.md)), and the `jud` binary built with the `mcp` feature, which the release binaries do not carry:

```sh
cargo install judgment --features mcp
```

The feature is the binary's alone: the library links no MCP code, and a `jud` built with `cli` has no `mcp` command.

## Start the server

```sh
jud mcp rubrics/
```

`jud mcp` speaks the Model Context Protocol over stdio, so a host starts it rather than a person; run by hand it waits for JSON-RPC on stdin. Every `.jud` Rubric under the directory, searched recursively, becomes one tool named by the rubric's `metadata.name`. Cases and recordings under it are not tools; a file that is not a document is named on stderr and skipped, and two rubrics with one name refuse to start, because a tool is named by its rubric.

The backend is chosen exactly as for `jud RUBRIC` ([Configure a backend](configure-a-backend.md)): TypeSafe by default, and without a key `jud mcp` refuses to start with status 2 rather than serve tools that cannot answer. `--replay DIR` answers from recordings with no key and no network, which is the way to try it:

```sh
jud mcp rubrics/ --replay recordings/
```

## Register it with Claude Code

```sh
claude mcp add jud -- jud mcp rubrics/
```

The plugin does not register it for you: an entry in the plugin would start a server for everyone who installs it, including those whose `jud` has no `mcp` command.

## What a tool takes and returns

A tool's input is `{state, options?}`: `state` is any JSON value, as on stdin for `jud RUBRIC`; `options` maps a question id to its options (option key to description) for the Choices the rubric marks `options_from: request`, asked in the order the object is written. The description lists the questions and their kinds.

The result is one text block holding the verdict map, byte for byte what `jud RUBRIC` prints, less its closing newline. A call that cannot be answered (a state nobody recorded, a backend that failed, a state the rubric does not lower for) is a tool error (`isError`) whose text is the message `jud` would print, so the agent can read it and try again. A call naming no rubric, or without a `state`, is a protocol error.

## What is not here

A rubric's images come from its own `assets`, read from its directory on each call; there is no `--asset` or `--pixels` on a call. stdout is the protocol channel and nothing else is written to it; diagnostics go to stderr.
