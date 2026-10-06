---
description: Write a .jud Cases document for a Rubric (labelled states its thresholds are tuned on), bound to the rubric and checked with the crate's reader
argument-hint: "<rubric path> [brief, number of cases, or the path of a cases document to revise]"
---

Write a `.jud` Cases document for the rubric named here, or revise the cases
document given:

$ARGUMENTS

Follow the jud skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud/SKILL.md` and
its `references/format.md` first, in particular the sections on cases. Then:

1. Read the rubric. List its questions, each primitive, each Choice's options
   (and whether they come with the request), each Score's levels, and every
   `when` or `part_when`, because a label is only valid for a question the
   case's request actually asks.
2. Draft the states. Use the keys the application really sends, in their real
   shape; for a conversation, an array of turns. Cover every outcome at least
   once: every option of each Choice including the way out, every level of
   each Score including the lowest, both `true` and `false` of each Noul. Add
   the hard ones, the states a labeller would argue about, and give each a
   `note` saying why the label is what it is. If the brief gives no number,
   write enough to cover the outcomes and say that more are needed before a
   threshold is tuned.
3. Label only what the case is sure about: a question left out of `expect` is
   asked and not graded. Labels in the answer's own vocabulary: bare `true` or
   `false` for a Noul (`{from_turn: n}` or `{from_turn: null}` over a
   conversation), an offered option key for a Choice, a level's text or index
   for a Score. For a Choice with `options_from: request`, supply each case's
   options under `options`.
4. Name the rubric in `spec.rubric` (by name while the questions move, by
   fingerprint once a run must be reproducible) and give the document a
   `metadata.name`.
5. Bind and check: `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh check
   <rubric> <cases>`, and when the request depends on the state,
   `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh lower <rubric> --cases
   <cases>` to see what each case sends. Fix every refusal in the case, or
   raise it as a defect of the rubric when the label is right and the question
   is not.
6. Reply with the path written, a table of the cases and their labels, the
   `jud check` summary line and the cases fingerprint, and say how many cases
   label each outcome so the gaps are visible.

If the brief names no output path, write the cases beside the rubric as
`<rubric name>-cases.jud`, and say where.
