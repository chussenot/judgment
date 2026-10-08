---
description: Turn a .jud rubric into a small typed Rust module (enums for options and levels, one decide call), compiled and tested before it is handed over
argument-hint: "<rubric path> [output .rs path] [embed]"
---

Write the Rust module for this rubric:

$ARGUMENTS

Follow the jud-rust skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-rust/SKILL.md`
and the template it names (`references/triage.rs`, or `references/routing.rs`
for a rubric with `options_from: request` or `when`) before writing anything.

Run every `jud` command through `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`,
written out in full, one command per Bash call, with nothing chained to it.

1. `jud.sh check <rubric>` must say `0 refused`; copy the questions
   fingerprint from it. A refusal stops here: report it and offer
   `/jud:check`.
2. Choose the output path. If one is given, use it. Otherwise, in a Rust
   crate (a `Cargo.toml` above the rubric or in the working directory), use
   `src/<module>.rs` of that crate; else write `<module>.rs` beside the
   rubric. The `include_str!` path is relative to the `.rs` file. With
   `embed` in the arguments, embed the YAML instead of `include_str!`, and
   say what that costs.
3. Name, map and write the module exactly as the skill says. Overwrite an
   existing module only if its header says it was generated from this rubric.
   Otherwise stop and ask.
4. Verify as the skill says: `cargo check`, `cargo test <module>::tests` and
   `cargo clippy --all-targets` in the user's crate, or in a throwaway crate
   in a temporary directory outside the project, deleted afterwards. Fix the
   module until all three pass. Change no other file in the project unless
   asked: the `Cargo.toml` lines and the `mod` line go in the reply.
5. Reply with:
   - the file written and the `include_str!` path;
   - every name you had to change, and why;
   - the `Cargo.toml` lines and the `mod` line;
   - a usage example with the module's real names;
   - the checks that passed.
