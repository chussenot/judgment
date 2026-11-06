---
description: Review a .jud rubric and its cases for what the checker cannot see (a compound question, a Choice with no way out, a threshold moved into the instructions, an invented tuning block, labels copied from a recording, thin outcomes), ranked with each fix and its cost; read-only
argument-hint: "<rubric> [cases] [recordings dir]"
---

Review these documents for substance, after the reader's check:

$ARGUMENTS

Follow the jud skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud/SKILL.md`, in
particular "The rules writers trip on", "Writing questions that hold up",
"Writing cases a threshold can rest on" and "Reviewing an existing
document". Run every `jud` command through
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`, one per Bash call. This
command changes no file: it proposes.

1. **The reader first.** `jud.sh check <everything given>`: refusals are
   findings of their own, reported as `/jud:check` reports them, and a
   document in the old envelope is said to be so and left alone. Keep
   the fingerprints the check printed.
2. **The rubric, question by question**, against "Writing questions that
   hold up": one finding per rule broken, quoting the line. Among them:
   a compound question (two things, one probability); criteria that
   restate the question instead of defining each outcome; a Choice with
   no way out when the state might fit none; a threshold or a
   confidence asked for in the instructions, where it cannot be tuned;
   a question whose deciding fact is not in the state the instructions
   name, or that a machine-written field already decides; a Box with no
   Noul beside it; a Score with more levels than the state can tell
   apart; a `when` that names another question. For each, the fix and
   its cost: a question change re-records every case that asks it; a
   policy change is free.
3. **The policy.** Every gate with a `note` that says where its bar came
   from; a `tuning` block whose `cases` fingerprint matches the cases
   given (else the bars rest on other cases: say so), whose `model` is
   named by the recordings when a directory is given (else `moved`), and
   never invented (a block with no `labelled` map, a round `tuned_at`, a
   fingerprint that matches nothing); a `fallback` the application can
   live with; a bar of 0 or 1 that acts on everything or nothing.
4. **The cases**, when given, against "Writing cases a threshold can rest
   on": an outcome no case labels; a Noul with no `false`; a way-out
   option never labelled; labels in a case whose `note` argues the other
   way; states that are not the application's shape (invented keys, a
   summary where the application sends a record); `jud.sh lower <rubric>
   --cases <cases>` for a label on a question a `when` leaves out (the
   reader refuses it) and for options a case supplies under a key the
   question already offers. With a recordings directory, compare every
   label with the recorded answer: a cases document whose every label
   equals the model's answer was labelled from the recordings and tests
   nothing; say so with the count.
5. **With recordings**, `jud.sh eval <rubric> <cases> --replay <dir>
   --json` once, for the `signals` only (`collapsed`, `never_answered`,
   `no_better_than_majority`, `defers_most`, `defers_nearly_all`): each
   is a finding pointing at `/jud:eval` for the triage. Quote no other
   number.
6. **Reply** with a ranked table (finding, where, why it matters, fix,
   cost: free, calls for new cases, or a re-recording), the refusals
   first, then what passed in one line each (the rules read and met), and
   the fingerprints. Rank by what the finding costs the user: a question
   the model cannot answer from the state, then a label that moves a bar
   wrongly, then coverage, then wording.

Edit nothing, not even a typo. Offer `/jud:rubric <path>` or `/jud:cases
<rubric> <path>` to apply what the user accepts, and `/jud:eval` for the
numbers.
