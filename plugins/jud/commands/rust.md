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
   `embed` in the arguments, embed the YAML instead of `include_str!`, as
   the skill's embedded form says (a retune then needs a regeneration, and
   a test holds the copy to the file).
3. Name, map and write the module exactly as the skill says. Overwrite an
   existing module only if its header says it was generated from this
   rubric (the same path, and the same `include_str!` target). Otherwise
   stop and ask. A regeneration writes the module afresh, not by patching.
4. Verify as the skill says: in a throwaway crate outside the project,
   written with the Write tool, that mounts the module with `#[path = ...]`.
   Run, one per Bash call, `cargo test --manifest-path <dir>/Cargo.toml`
   and `cargo clippy --manifest-path <dir>/Cargo.toml --all-targets -- -D warnings`
   (cargo's flags before `--`), and fix the module until both pass. Change
   no other file in the project unless asked: the `Cargo.toml` lines and
   the `mod` line go in the reply.
5. Reply as the skill's step 8 says, including what the caller owes and,
   when an existing module was replaced, what the regeneration changes.
