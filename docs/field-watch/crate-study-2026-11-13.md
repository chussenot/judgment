---
title: A study of the other Rust crates in the field, 2026-11-13
description: A reading, on 2026-11-13, of the source of thirty-eight Rust crates that work on typed decisions from System One models (the thirty-four the field watch listed and four published that day), through three lenses (the crate, the jud command, the Claude Code plugins); for each crate what it does better than judgment, what judgment does better and its licence; what the reading showed to be wrong or untested in judgment; and a ranked list of opportunities, each with its evidence, lens, effort and a label, SMALL or ASK.
status: current
last_reviewed: 2026-11-13
tags: [research, judgment, jud, plugins, comparison, field-watch]
---

# A study of the other Rust crates in the field, 2026-11-13

About thirty Rust crates now work in this field, and some may solve a problem better than `judgment`, `jud` or the plugins do. This page reads each of them for that, and ends with what is worth taking.

**In short.** No crate does the whole job better, and most do less: fourteen of them send questions or options in sorted or random order, where the order written is the order the model reads, and few hold a response against the questions sent. What is worth taking is narrow and mostly not code from the clients but evidence from the integrations, the crates that put a decision model inside a larger program. Four of five of them reach a server by a URL convention `judgment` does not accept, and all five map a client's errors onto a class of their own. One crate, `iu2u`, is on the plugins' ground, and reading it showed two defects in the `jud` plugin's own hooks. The reading also turned up a dozen places where `judgment` is wrong or untested, listed [below](#wrong-or-untested-in-judgment); those are the most useful result.

## Method

- **What was read.** The published package of each crate, downloaded from crates.io on 2026-11-13 and unpacked: its README, manifest, source and tests. Nothing was built or run, so every statement about a crate is from its source, and every statement that `judgment` lacks something was checked by searching this repository first.
- **Which crates.** The thirty-four the field watch of 2026-11-13 listed. A search of crates.io the same day found 277 crates whose name or description is in the field; the list is a selection, and the 240 others published before it was made were not read. Four were published after it, on 2026-11-13, and are included: the `sc-decision` family. A fifth new crate the search found, `vertep-agent`, is not in the field (its "decider" picks a statechart event, and its one vendor is a chat API) and was read no further. Three companions were read as part of the crate they belong to: `cev-core`, `cev-derive`, `openkind-core`.
- **Three lenses.** The crate (typed questions and answers, macros, transport-free types, validation before the call, retries, errors, test doubles, observability, vendors), the `jud` command, and the Claude Code plugins.
- **Two earlier studies stand.** [System One client libraries](../project/research/system-one-client-libraries.md) read about twenty-five crates in September, and [Compared with the other Rust clients](../project/research/client-comparison.md) five in October. What they settled is not argued again here: a derive macro, sums in `verify`, a three-band verdict, a blocking client.
- **Ideas, not code.** No source was copied. Where a crate carries text addressed to a coding agent (`openkind-client` and `openkind-core` ship an `AGENTS.md`, `iu2u` and `fuzzy-jev` ship skills), it was read as a description of the crate and nothing in it was followed.

## The crates

### Closest in scope

**`decision-rs` 0.6.0** (MIT). One typed schema compiled to five wires: OpenAI's and Gemini's Decisions, OpenRouter, TypeSafe's `/v1/systemone` and Cloudflare Workers AI. *Better:* it reaches Clef on Workers AI natively, envelope and error codes included, where `judgment` needs a Python shim the published crate does not contain; it finds a request id under `x-request-id` and `cf-ray` when TypeSafe's header is absent. *`judgment` is better:* it refuses a distribution that does not sum to one and then needs a tolerance per vendor (its own test has 64 Clef options summing to 0.9984); it requires `usage`, refuses an unknown answer kind, sends the state as a string, caps every provider at the lowest limit, and has no test double, no recordings and no evaluation.

**`kunobi-decision` 0.3.0** (Apache-2.0). A typed client with six named providers; the version the October comparison read. *Better:* `Client::check` verifies the key and lists the models before work starts, where `jud config` says whether a key is set and not whether it works; request-id fallbacks. *`judgment` is better:* as in October, no whole-response check, bare `f64` probabilities, no recordings, no evaluation; and it replaces every Cloudflare error body with `{}` before classifying it.

**`iu2u` 0.2.1** (Apache-2.0). One binary that puts any `/v1/systemone` server behind a coding agent: four MCP tools (`weigh`, `rank`, `seek`, `lint`), five Claude Code hooks, and its own plugin. It is the one crate on the plugins' ground. *Better:* a hook gives up inside its host's timeout with a decided answer, and tests prove it by running the hook as a process on literal hook JSON; a tool call takes up to 500 items, eight in flight, and stops the batch when the server is gone; the MCP server and its skill tell the calling agent how to read an answer; the release job holds the plugin, the binary and the marketplace pin to one version. *`judgment` is better:* `iu2u` checks only that every question id came back, so an option never offered or a probability of 1.7 is accepted; it keeps options in a sorted map; every bar in it is a constant nobody measured, and its README tells the user to evaluate them and gives no way to; it has no retries, no pacing and no recordings. Its questions are improvised per call, where a rubric is a reviewed file with tuned bars.

**`cev-rs` 0.3.0**, with `cev-core` and `cev-derive` (Apache-2.0). The typed SDK of the `cev` server, with a derive for Choice enums. *Better:* an enum can be a Score's levels, so the answer comes back as a variant. *`judgment` is better:* its derive makes the wire key from the identifier, so renaming a variant changes the wire; its Score confidence formula differs from the one `judgment` has pinned to a hosted answer; a Noul becomes a `bool` at a fixed 0.5 inside the SDK; no retry, timeout, validation or response check; its mock answers anything by keyword overlap.

**`cev-server` 0.3.0** (Apache-2.0). A server of the wire on open checkpoints, with feedback endpoints and online learning. *Better:* a paired gate (an adapter is served only once its loss beats the base model's on the same labels by a set margin), where `jud eval --against` prints two separate intervals; two position tests that move every option, where `--swap` reverses and so never moves the middle option of an odd Choice; a correction by a person becomes a labelled row. *`judgment` is better:* labels, answers and thresholds are files in git with fingerprints, and a model's answer is never rewritten after the fact.

**`systemone` 0.2.0** (MIT). A small async client with the SDKs' retry defaults; 0.2.0 added a switch to OpenAI's `/v1/decisions`. *Better:* a connection failure's message carries its whole cause chain, where `jud` prints reqwest's top line, which names the URL and not the cause; it sends `x-typesafe-retry-count` on a retry. *`judgment` is better:* questions and options are sorted maps, there is no response check, and its OpenAI dialect writes an absent confidence as `0.0`.

**`systemone-core` 0.2.0** (MIT). The `no_std` half of that client: wire types, builders and the retry arithmetic as a pure function. *Better:* a host with its own socket and timer can reuse it. `judgment` builds without `http`, on `std`, but its request body type, its retry policy and its status classifier are all behind `http`, though the reference says that build is "everything a project with its own transport needs". *`judgment` is better:* insertion order, limits checked before sending, a jitter that cannot panic.

**`systemone-macro` 0.2.0** (MIT). A derive that takes option labels from an enum and reads `serde(rename)`. *Better:* an enum the application already serialises is also the option set. *`judgment` is better:* `options!` keeps key and description beside the variant and survives a rename; no page says that `options!` passes attributes through or that `Options` can be implemented by hand.

**`systemone-facet` 0.2.0** (MIT). The same labels by runtime reflection. Nothing worth taking: it panics on a type that is not an enum and leaks a string per variant.

### Published on 2026-11-13

**`sc-decision-schema` 0.1.0** (MIT). The contract crate of an agent framework: wire types, a sans-IO backend trait, and one digest per question. *Better:* a per-question digest says which question changed, where a recording holds one fingerprint for the whole request; a parse failure names its path (`answers.q: missing field`); every example of TypeSafe's guides is a fixture that must round-trip. *`judgment` is better:* its fingerprint is canonical JSON another tool can reproduce, with published vectors; nothing in this family holds a response against the questions.

**`sc-decision-systemone` 0.1.0** (MIT). The encode and decode of the wire as two pure functions. *Better:* it removes the client's own key from every server body it quotes; it does not send a key over plain http to a host that is not loopback; it refuses a base URL with a query, where `judgment` drops the query without a word. *`judgment` is better:* its decode never looks at the request, and only 400 and 422 are classified.

**`sc-decision-local` 0.1.0** (MIT). The same backend for a local daemon, with a probe per daemon kind. *Better:* loading a cold model is kept out of the request timeout, where `judgment`'s answer is three full retries or a larger timeout for every call. *`judgment` is better:* what it knows of each server is a dated run on record, not four hard-coded shapes.

**`sc-decision-llm` 0.1.0** (MIT). The same request asked of a chat model, the probabilities being the model's own report. Nothing worth taking: it is off the wire, and a self-reported number is what `judgment` exists not to trust.

### Clients

**`typesafe-client` 0.2.0** (MIT OR Apache-2.0). The crate closest to `judgment`: typed keys, a trait, a verified response, a fake. *Better:* its fake can fail, can answer differently on the next call, and keeps the whole request; its image-reference check looks only where the server substitutes; its contract test holds the OpenAPI document's named example pairs; one call says whether an error is worth retrying. *`judgment` is better:* an unknown answer kind or a missing `usage` fails its whole response, its `verify` compares neither the legend nor the score's range, it has no response cap, and an unscripted question gets a made-up 0.5 from its fake, where `Fake` fails the call.

**`typesafe-rs` 0.1.0** (MIT OR Apache-2.0). A port of the SDKs' transport. *Better:* it sets the connection pool's idle time and keep-alive; a successful response says how many attempts it took. *`judgment` is better:* no response verification, no budget, and an error message cut with a byte slice that panics inside a multi-byte character.

**`typesafe-rs-mock` 0.1.0** (MIT OR Apache-2.0). A scripted HTTP server for tests. It drives the real HTTP path (statuses, headers, latency), which `Fake`, `Recorder` and `Replay` never see; wiremock does the same job, and its helpers ignore the question asked. Worth one line in the test skill: a wiremock body can be built from a `Fake`'s response.

**`typesafe-system-one` 0.1.1** (MIT). A careful port hardened after an adversarial review. *Better:* it refuses a state that is a number, a boolean or null before any I/O; a 2xx that does not fit names the place (`answers.tone.confidence`); a base URL with a query is refused at build. *`judgment` is better:* questions travel sorted, `extra_body` may replace `questions`, and request bodies are logged at debug.

**`typesafe-systemone` 0.2.0** (MIT OR Apache-2.0). A small client with a chained builder. Nothing worth taking: sorted maps, a `Retry-After` conversion that panics on a huge value, no request id.

**`typesafe-io` 0.1.0** (MIT). A thin binding of the two endpoints. Nothing worth taking: its `Debug` prints the key and its maps send questions in random order.

**`jev` 0.1.2** (MIT OR Apache-2.0). A blocking client with one-question helpers. *Better:* a client that sends no `Authorization` header for a server with no key, where `judgment` refuses a blank key and its guide teaches the word `unused`; a TLS feature without a C build. *`judgment` is better:* option order, verification, retries.

**`jevlin` 0.2.1** (MIT; the manifest names a licence file, which is why crates.io says non-standard). A struct of answer fields is the question set. *Better:* a request with no question is refused before the call; a caller's own `reqwest::Client` is accepted. *`judgment` is better:* answers are bare `f64`, a `Retry-After` is slept uncapped, a duplicate question id silently replaces.

**`kunobi-jev` 0.3.0** (Apache-2.0). Now a three-line re-export of `kunobi-decision`. Nothing to take.

**`jev-driver` 0.1.0** and **`jev-driver-macros` 0.1.0** (MIT OR Apache-2.0). One type per question, built by six derives. *Better:* a backticked reference in a question's instructions that names nothing in the state is an error before sending, the one mistake every check `judgment` has lets through. *`judgment` is better:* questions and options go out alphabetically whatever the declaration order, a legend must be strings, 5xx and a timeout and a refused connection are one error, and the library loads `.env` by default. The derives buy little: the count and duplicate-key errors they raise at compile time `judgment` raises on the first call, before any request, and the price is a second crate pinned with `=` and `syn` in every build.

**`system2` 0.1.0** (MIT OR Apache-2.0). The largest SDK in the field, two thirds of it tests. *Better:* it was run live against OpenRouter and reads its wrapped error body and its `x-generation-id`, which `judgment` does not, so `Error::is_request_too_large` is false there; it probes its cache for writability before a request is sent, so a paid answer is never lost to an unwritable file; a set with no questions sends no request. *`judgment` is better:* a replay key that survives serialisation, a legend held against the levels, errors by remedy, a contract test, properties and snapshots, none of which it has.

**`system-one-client` 0.1.0** (Apache-2.0). One careful POST built around what a fail-open caller does next. *Better:* a 2xx that does not decode carries the first 512 bytes; the base URL may be the origin or the full endpoint; no token means no header; its error kind is read from a nested `error.code`, which `judgment` never reads. *`judgment` is better:* the SDKs' retry set, verification, request ids; its golden tests pass when their fixtures are absent.

**`openkind-client` 0.5.1**, with `openkind-core` (MIT). A port of the Python SDK's client over validation shared with a server. *Better:* a test-by-test map to the official SDK's own suite at a named commit, where `judgment` states parity in prose against versions older than the ones it cites elsewhere; a missing key can be allowed. *`judgment` is better:* its questions are a randomly seeded map, structured Score levels cannot be represented, an unknown answer kind fails the response, and its sum tolerance of 0.001 would refuse three options at 0.33 each.

**`fuzzy-jev` 0.7.6** (MIT). A command and a library for OpenRouter's decisions endpoint, with fuzzy-logic rules over the answers. *Better:* every call ends with tokens and dollars; `jev add skill` writes its skill to any agent's directory with careful write rules; plain text is a state. *`judgment` is better:* everything after the call. It tells its users to check their rules against reviewed cases and gives them nothing to do it with; it has no retries, cache, recordings or evaluation.

### Integrations

None of these depends on `judgment`; `polyc-judgment-systemone` shares a word with it and nothing else.

**`agentscope-extensions-judge` 0.1.1** (Apache-2.0). A client for the wire, a second for OpenAI's Decisions, and 29,000 lines of agent applications on the first. *Better:* the base URL may be the root, end in `/v1` or be the full endpoint; its test doubles script a failed call and one that never returns; a wrapper runs any decision as off, shadow or enforce under a time budget. *`judgment` is better:* sorted questions with a comment that order "is not semantically meaningful", no `Retry-After`, and a sum check whose first bound rejected a real 34-option response. "Calibrated" in its name means the wire's own probabilities and nothing more.

**`gaise-provider-typesafe` 5.7.0** (MIT OR Apache-2.0). The TypeSafe side of a multi-provider router. *Better:* it removes its own key from a server's error body, and tests that with a server that echoes it. *`judgment` is better:* its retries skip transport failures, its errors are strings, and its image references carry a random value no recording could replay.

**`polyc-judgment-systemone` 2026.11.1** (Apache-2.0 OR MIT). One implementation of its host's provider trait. *Better:* it keeps no response body in any error, because a gateway's 422 can quote the state. *`judgment` is better:* retries, `verify`, order, the gateway's request id. It joins `/systemone` onto a base that ends in `/v1`, a convention `judgment` cannot be pointed at.

**`allama-systemone` 0.3.0** (MIT OR Apache-2.0). One provider with six presets and three dialects; the best-informed crate here about how servers differ. *Better:* a path setting and a Workers AI dialect reach servers `judgment` needs a shim for; a 404 that is a missing route says so; it can be built with no credential; its encoder and decoder are callable without HTTP; it records server limits and shapes this repository's compatible-servers page lacks (LiteLLM, DigitalOcean, Ollama's caps, Clef's image shape). *`judgment` is better:* it sends once, drops the Score legend, and refuses images on TypeSafe's endpoints, which stopped being true on 2026-10-14.

**`ontomark-jev` 0.7.1** (MIT OR Apache-2.0). Three hooks of a note-ingest engine filled with Noul and Choice questions, off by default. *Better:* every failure becomes one reason word and the engine returns what it returns without the model, and a table of twelve causes tests exactly that: a pattern `judgment` neither documents nor makes easy, since `Fake` cannot fail. *`judgment` is better:* its options go out sorted (`c0, c1, c10, c2`), and it finds a timeout by matching text in an error string.

**`llm-wires` 0.7.0** (MIT). Wire adapters for several APIs, the System One wire and OpenAI's Decisions among them. *Better:* a closed port reads "refused" in its error, and a test pins it; a 429 keeps the provider's words, where `judgment` drops a 429's body; image references are looked for in string values only. *`judgment` is better:* order, verification, typed probabilities, and everything unknown kept; it drops the legend, the extras and the cost.

**`yoagent-workers` 0.4.0** (MIT). A Cloudflare Worker's AI binding as a decision backend: no HTTP, no token, a future that is not `Send`. It shows what a host with its own transport needs and `judgment` does not give: the `SystemOne` trait cannot be implemented over the binding, and no build for `wasm32` is checked anywhere. It also separates a spent quota from a capacity limit, which `judgment`'s shim passes through as one retried 429.

**`cerno-core` 0.1.4**, **`cerno-sdk` 0.1.4**, **`cerno-server` 0.1.4** (MIT). A service that answers the three kinds of question from an ordinary chat model's logprobs. Nothing for a client to take from the engine. Two facts matter. Its server answers `POST /v1/systemone` with another body (questions as an array, the state a string, a Score counted from one), so `judgment` pointed at it gets a 400 that reads as the caller's mistake: serving the path is not speaking the wire. And one conformance file is run by its Rust, Python and TypeScript clients, where the `.jud` format's refusal cases exist only as Rust source.

## Wrong or untested in judgment

Found while checking what `judgment` lacks. Each was read from source. The first three were then confirmed by running: both hooks with hook input on stdin, and a request serialised with a `NaN` in its state.

1. **The recording guard fails open on some paths.** `plugins/jud/hooks/guard-recording.sh` writes the file path into its deny JSON unescaped, so a quote or a backslash in the path makes the decision unreadable and the edit goes through. No test runs either hook script.
2. **The post-write hook can report a compiler's errors as a refused document.** With no `jud` on `PATH` it falls through to `cargo run` inside a hook the host stops at 20 seconds, and any output with `error` lines is reported as "refused the document".
3. **A non-finite number in the state is sent as `null`**, with no error on either side. TypeSafe's request rules refuse one, and its Python SDK sends it as a string since 0.7.4.
4. **Two plugin pages say what the crate does not do.** The `judgment` skill says `Client::from_env()` reads `TYPESAFE_BASE_URL`; it reads the key only, so code written from the skill in an OpenRouter environment sends that key to the hosted API. The test skill says `Fake::calls()` shows the questions and options; a `Call` holds question ids. And `/judgment:integrate` pins `judgment = "0.16"`, a line the release script does not cover.
5. **`jud RUBRIC` and `jud mcp` send a request with no questions** when every `when` fails for a state, where `jud record`, `eval` and `tune` refuse it before any call with status 2.
6. **An error's `kind` never reads a nested `error.code`**, though one verified server's envelope has one.
7. **A decode error does not name the answer it failed on**: serde's line and column, no question id.
8. **A transport failure's message omits its cause**, in the library's text and on `jud`'s stderr.
9. **"The key is quoted by no error" holds only while no server echoes it**: a 400, 402, 403, 404 or 5xx body is quoted, and no test has a server echo the key.
10. **The reference says the build without `http` is "everything a project with its own transport needs"**; the request body type, the retry policy and the status classifier are behind `http`.
11. **The image-reference check passes on a match in a question id or a wire key name**, where the `.jud` reader applies the stricter rule.
12. **A repeated option key in `--options` JSON is merged silently**, against the rule the builder states.
13. **A 429 that names a wait above the cap pauses the pacer for the backoff, not the wait**, against what two comments say.
14. **`--swap` never moves the middle option of an odd Choice**, and the guide does not say.
15. **Smaller ones.** An empty string passes for instructions. The error table calls `Http` transient, though it also holds a 404. The configuration page says the client sends to the whole base URL, though a query is dropped. The parity heading names older SDK versions than the crate cites elsewhere. A comment in the contract test says the document has no named examples, and it now has four that no test reads.

## Opportunities, ranked

Ranked by what a user of `judgment` gains. **SMALL** fits the field watch's rules; **ASK** would change public API, the command's behaviour or output, the format, a dependency or the scope, or needs more than half a day, so it is a question for the maintainer and not a change.

| # | Opportunity | Evidence | Lens | Effort | Label |
|---|---|---|---|---|---|
| 1 | Harden the `jud` plugin's two hooks and test them as processes: escape the path in the guard's JSON; never build inside the post-write hook | `iu2u` (hooks tested as processes, a decided answer inside the host timeout); items 1 and 2 above | plugins | 3 h | SMALL |
| 2 | Refuse a request with no questions in `jud RUBRIC` and `jud mcp`, as the case commands do | `system2`, `jevlin`, `openkind-core` refuse it before the call; item 5 | jud | 1 h | ASK: an exit status changes |
| 3 | Check the state before sending: non-finite numbers, nesting past 64 levels, a scalar state | TypeSafe's request rules; `typesafe-system-one` and `system2` refuse a scalar state; item 3 | crate, jud | 6 to 8 h | ASK: a call that went out is refused |
| 4 | Name the question in a decode error | `typesafe-system-one`, `sc-decision-schema` (a dotted path); item 7 | crate, jud | 2 h | SMALL |
| 5 | Read a nested `error.code` as the error's kind | `system-one-client`; one verified server's envelope; item 6 | crate | 1 h | SMALL |
| 6 | Correct the plugin pages and hold the pinned version to the crate's | all five other clients read `TYPESAFE_BASE_URL` in the library; item 4 | plugins | 1 h | SMALL |
| 7 | Reach the server the host was configured with: an exact endpoint, and a client with no key | `jev`, `jev-driver`, `system-one-client`, `openkind-client`, `allama-systemone`, `polyc-judgment-systemone`, `ontomark-jev`, `agentscope-extensions-judge` | crate, jud | 6 h | ASK: new builder methods and a configuration key |
| 8 | A `Fake` that can fail and answer differently on the next call; an error's group as a value | `typesafe-client`, `agentscope-extensions-judge`, `ontomark-jev`, `polyc-judgment-systemone` | crate, plugins | 5 h | ASK: new public items |
| 9 | Many states per call in `jud mcp` and `jud RUBRIC` | `iu2u` (`weigh`: 500 items, eight in flight) | jud, plugins | 4 to 8 h | ASK: new input, may pass half a day |
| 10 | Tell the calling agent how to read a verdict: MCP server instructions, one sentence per tool | `iu2u` | plugins | 1 to 2 h | ASK: changes what `jud mcp` returns |
| 11 | Paired counts and a sign test under `jud eval --against` | `cev-server`'s paired gate | jud | 3 to 4 h | ASK: new `--json` keys |
| 12 | A transport failure that says why | `llm-wires`, `systemone`; item 8 | crate, jud | 1 to 2 h | SMALL, and open as bead `judgment-7yf` |
| 13 | Probe the recordings directory before the first call | `system2`; a paid answer is lost to an unwritable directory | crate, jud | 1 to 2 h | SMALL |
| 14 | Redact the client's own key from quoted server text | `gaise-provider-typesafe`, `sc-decision-systemone`, `polyc-judgment-systemone`; item 9 | crate | 2 h | ASK: an option of the open decision `judgment-bsz.21.11` |
| 15 | Warn when a question refers to something the state does not have | `jev-driver` | crate, jud, plugins | 4 h | ASK: a new function and a new line of output |
| 16 | A documented path for a host with its own transport: a guide, a `wasm32` check, then the body type outside `http` | `yoagent-workers`, `allama-systemone`, `ontomark-jev`, `systemone-core`; item 10 | crate | 3 h, then over half a day | SMALL for the guide and the check, ASK for the API |
| 17 | Read OpenRouter's wrapped errors and its `x-generation-id` | `system2`'s live captures | crate | 2 h and a live run | SMALL, blocked on a `live:openrouter` run |
| 18 | `jud config --check`: is the server reachable, the key accepted, the model listed | `kunobi-decision` | jud | 3 to 4 h | ASK: a new flag |
| 19 | Per-question fingerprints in a recording | `sc-decision-schema` | jud, format | 1 to 2 days | ASK: a `.jud` minor |
| 20 | Tokens and cost in `jud record`'s tally and dry run | `fuzzy-jev` | jud | 3 h | ASK: new output |
| 21 | One rule for "is this image referenced": string values only | `typesafe-client`, `llm-wires`; item 11 | crate | 2 h | ASK: a request accepted today is refused |
| 22 | Documentation corrections found on the way (items 10, 14 and 15, the Decisions note's "in question order", a server that serves the path and not the wire, five servers the compatible-servers page is behind on) | `cerno-server`, `llm-wires`, `allama-systemone` | docs | 3 h | SMALL |
| 23 | Request-id fallbacks (`x-request-id`, `cf-ray`), the retry-count header, a 429's body kept | `decision-rs`, `kunobi-decision`, `openkind-client`, `systemone`, `llm-wires` | crate | 2 h each | ASK: what the crate sends or reads on the wire |
| 24 | A Noul gate that can defer; typed Score levels; a load allowance for a cold local model; `.jud` refusal vectors as data; a TLS provider the consumer chooses | `agentscope-extensions-judge` and `ontomark-jev`; `cev-rs`; `sc-decision-local`; `cerno-sdk`; `jev` and `llm-wires` | format, crate, jud | from 3 h to days | ASK |

**Not recommended.** A derive macro (`cev-derive`, `systemone-macro`, `jev-driver-macros`): the September decision stands, and this reading adds the cost side. Arithmetic in `verify` (`decision-rs`, `agentscope-extensions-judge`, `openkind-core`, `fuzzy-jev`): four crates check sums and each needed a tolerance its own data broke. A backend off the wire (`systemone`'s and `llm-wires`' OpenAI dialects, `sc-decision-llm`): both lose what the research page said they would. Recalibration and online learning (`cev-server`, `cerno-core`): they rewrite a model's answer.

**Nothing worth taking:** `systemone-facet`, `sc-decision-llm`, `typesafe-systemone`, `typesafe-io`, `kunobi-jev`, `cerno-core`.
