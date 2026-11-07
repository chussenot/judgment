---
description: Write the Rust code for a decision a System One model answers through the judgment crate (typed questions, the call, the typed read, errors by group) with a Fake-backed test, compiled and linted before it is handed over
argument-hint: "<brief: the decision, the state the program has, where the code goes>"
---

Write the code for this decision with the judgment crate:

$ARGUMENTS

Follow the judgment skill: read `${CLAUDE_PLUGIN_ROOT}/skills/judgment/SKILL.md`
and `${CLAUDE_PLUGIN_ROOT}/skills/judgment/references/crate.md` first, and
the testing skill (`${CLAUDE_PLUGIN_ROOT}/skills/judgment-test/SKILL.md`)
before writing the test. One command per Bash call, nothing chained; files
with the Write and Edit tools.

1. **Read the project**: `Cargo.toml` (the `judgment` line, its features
   and minor, the edition, the `[lints]`), the module the code joins, and
   what the program already knows at the call site. A project with no
   `judgment` line gets the line in the reply, not in its manifest, unless
   asked.
2. **Split the decision** as the skill's step 2 says: what code computes
   goes in the state; each thing that needs judgment is one question. Say
   which is which, and what the caller must add to the state when a
   question's evidence is not there yet (then leave that question out).
3. **Write the module**: the `options!` enums, a `questions()` that builds
   them with their handles, one `async fn decide(backend: &impl SystemOne,
   state: &Value) -> judgment::Result<Decision>` (or the shape the
   project's code already has), every bar a named constant with a comment
   saying it is a guess until measured, a `match` that lists every
   variant, and the errors handled by group (step 6 of the skill).
4. **Write the test** beside it, with `Fake`: one per branch the code has
   (each option, below each bar, a missing answer), as the testing skill
   says.
5. **Verify**: `cargo test` and `cargo clippy --all-targets -- -D
   warnings` in the user's crate when it has a lock file and the module is
   mounted; otherwise in a throwaway crate outside the project (the
   session's scratchpad or `/tmp/judgment-integrate-<module>/`, written
   with the Write tool, its `Cargo.toml` with the user's `judgment` line
   or `judgment = "0.16"`, `serde_json`, `tokio` with `macros` and `rt`,
   and the user's `[lints]`, ending in an empty `[workspace]`), mounting
   the module with `#[path = "<absolute path>"]`. Fix until both pass.
6. **Reply** as the skill's "Reply" says, and offer `/jud:rubric` when the
   bars will be tuned by someone other than the code's author.
