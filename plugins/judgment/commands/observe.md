---
description: Wire an Observer (or OtelObserver) and a Pacer into Rust code that calls the judgment crate, remove what they make redundant, and test the counts
argument-hint: "<file or module that builds the client or fans out calls> [otel] [pacer N]"
---

Make these judgment calls observable, and bounded when they fan out:

$ARGUMENTS

Follow the judgment-observe skill: read
`${CLAUDE_PLUGIN_ROOT}/skills/judgment-observe/SKILL.md` and, for `otel`,
`${CLAUDE_PLUGIN_ROOT}/skills/judgment-observe/references/export-to-opentelemetry.md`;
for a pacer, `references/bound-a-stream.md`. One command per Bash call;
files with the Write and Edit tools.

1. **Read the code**: where clients are built, what the application uses
   for metrics and tracing, whether calls fan out (a spawn per item, a
   `join_all`, a stream), and any hand count of tokens or cost.
2. **Wire** the observer at the client's construction, `OtelObserver`
   with `otel` (the `otel` feature on the `judgment` line, the
   application's own meter provider), and a shared `Pacer` (the size
   given, else one the reply justifies) on the client and a `Paced`
   backend where calls fan out.
3. **Remove** what is now duplicated (a hand count of tokens, a retry
   loop around the call, a fixed sleep between calls), and say each.
4. **Test** the observer's counts after a success, a 429 and an unfit
   answer against wiremock and a real `Client`, and the pacer with a
   paused tokio clock, as the skill says.
5. **Verify** with `cargo test` and `cargo clippy --all-targets -- -D
   warnings` (the user's crate, or a throwaway one as `/judgment:integrate`
   step 5 builds it). Reply as the skill's "Reply" says.
