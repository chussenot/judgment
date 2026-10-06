---
description: Check .jud documents with the judgment crate's reader (jud check), binding cases to their rubric and verifying recordings, and explain every refusal
argument-hint: "[files or directories; default: every .jud under the project]"
---

Check these `.jud` documents with the crate's reader:

$ARGUMENTS

If no path is given, find every `*.jud` under the project (skipping build
output) and check them together, so cases bind to the rubric they name and
recordings verify against the request they answer. Run
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh check <files>`; the script
uses a `jud` on PATH, else builds it from a judgment checkout (the project, or
`JUDGMENT_DIR`), and otherwise says how to install one.

Then report, in this order: the summary line; every `error` line with the
file, the field and, in one sentence each, what the reader refuses and why
(`${CLAUDE_PLUGIN_ROOT}/skills/jud/references/format.md` has the rules and
the checklist of refusals); every cases document that could not be bound
because its rubric was not among the files; and the names and fingerprints
of what passed. A document the reader refuses by its envelope is in an old
shape: say so and point at the skill's "Moving an old document forward"
section rather than editing it unasked.

Do not change any document unless asked. If asked to fix, fix each refusal
in the document rather than around it (never by dropping a label or widening
an option list to make a check pass), re-run the check, and show the
before and after summary lines.
