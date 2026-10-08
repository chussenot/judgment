---
title: Label cases
description: How to write the labelled cases a rubric is graded on and its thresholds are tuned from, bind them to the rubric, and check that every label fits the request its case lowers to.
status: current
last_reviewed: 2026-10-08
tags: [judgment, jud, cases, labels, how-to]
---

# Label cases

**You will** write a `.jud` Cases document for a rubric, bind it, and know which outcomes it covers. **Prerequisites:** a rubric ([Write a rubric](write-a-rubric.md)); the `jud` command. The field rules are in [`Cases`](../reference/jud-format.md#cases).

The example labels four messages for the `refund-screening` rubric; the finished file is [`examples/jud/screening-cases.jud`](../../examples/jud/screening-cases.jud).

## 1. Read the rubric first

List each question, its primitive, each Choice's options (and whether they come with the request), each Score's levels, and every `when`. A label is only valid for a question the case's request asks: a case whose state lacks the path a `when` names cannot label that question, and the reader refuses it.

For `refund-screening`: `refund_request` (Noul), `reason` (Choice: `duplicate_charge`, `unhappy`, `cancelled`, `none_of_these`; asked when `message` is present), `tone` (Score: `calm`, `annoyed`, `angry`).

## 2. Take real states

Use the keys the application really sends, in their real shape. An invented state tests an invented request. For a conversation, the state is an array of turns.

## 3. Label what you are sure of

A label is written in the answer's own vocabulary: `true` or `false` for a Noul, an offered option key for a Choice, a level's text or index for a Score. A question left out of `expect` is asked and not graded. A wrong label costs more than a missing one, because the bar moves to fit it.

```yaml
    - id: charged-twice
      state:
        message: I was charged twice for March. Please refund the second one.
      expect: {refund_request: true, reason: duplicate_charge, tone: annoyed}
```

Label what a careful colleague would say with the rubric in front of them, never what the model said. A label copied from a recording turns the test into a test of nothing.

## 4. Cover the outcomes, and keep the hard ones

Every option of a Choice and every level of a Score should be the right answer at least once, including the way out: the `none_of_these` option, the `calm` level, the `false` of a Noul. A set with no negative cases tunes a bar that never says no.

The cases that were argued about are the ones that define the line. Give each a `note` with the argument, and leave a question unlabelled on a case that is genuinely ambiguous about it:

```yaml
    - id: furious-outage
      state:
        message: Your service was down all day and I lost a client. This is unacceptable.
      expect: {refund_request: false, tone: angry}
      note: reason left unlabelled; a refund is implied, not asked for, and two labellers disagreed
```

Count honestly. Four cases find a document that is wrong and show the loop; they do not tune a bar, and the interval `jud eval` reports will say so.

## 5. Name the rubric and check the binding

`spec.rubric` names the rubric by name while the questions move, by fingerprint once a run must be reproducible. Check the two files together, so each label is checked against the request its case lowers to. The paths are those of this repository's example; give yours:

```sh
jud check examples/jud/screening.jud examples/jud/screening-cases.jud
```

<!-- transcript: jud check examples/jud/screening.jud examples/jud/screening-cases.jud -->
```text
rubric    examples/jud/screening.jud: name refund-screening, jud/v1.3, 3 questions, 3 gates (untuned)
          questions sha256:92a19d0948fdaa410401d8c22b43d1783ac291b40af210d4aa4c81e8275e0e17
          policy    sha256:2c4cfcb3d2b6f200242a8589f22ca64d0627a611860f5498be20424a73261766
cases     examples/jud/screening-cases.jud: name refund-screening-cases, jud/v1.3, 4 cases, bound to examples/jud/screening.jud
          cases     sha256:0e0d8b5cb14429de6ab723450402612fd326e7e6d98fe64f229e2235f62c6cc9
2 documents, 0 refused
```

`bound to` names the rubric the cases were matched to among the files; it does not say that the labels fit. A label that does not fit is an `error` line printed after it, with the case and the field named, and the last line counts it. Every label fit when no `error` line follows and the last line says `0 refused` (status 0). A label for a question the case's state does not trigger, an option the case was not offered, a level past the last one, a `from_turn` on a Choice: each is refused here, and nowhere later. `jud lower examples/jud/screening.jud --cases examples/jud/screening-cases.jud` prints what each case sends when a `when` or a `part_when` makes that unclear.

## Conversations

A state that is an array is a conversation, and a Noul over it takes `{from_turn: n}` for the zero-based turn at which it becomes true (assistant turns count), or `{from_turn: null}` for never. Every other label stands for the last turn. [`examples/jud/handoff-cases.jud`](../../examples/jud/handoff-cases.jud) is a labelled conversation; [Conversations](../reference/jud-format.md#conversations) has the rule.

## Per-request options

For a Choice with `options_from: request`, each case supplies its options under `options.<question id>`, as the application would that day, and the request still needs 2 to 255 in total. [`examples/jud/routing-cases.jud`](../../examples/jud/routing-cases.jud) is the shape.

## The whole file

<!-- file: examples/jud/screening-cases.jud -->
```yaml
# Labelled cases for the refund-screening rubric, written in
# docs/guides/label-cases.md. Each case labels the questions it is sure
# about; a question left out is asked and not graded.
apiVersion: jud/v1.3
kind: Cases
metadata:
  name: refund-screening-cases
  description: Four messages, labelled by hand while writing the guide.
spec:
  rubric: refund-screening
  cases:
    - id: charged-twice
      state:
        message: I was charged twice for March. Please refund the second one.
      expect: {refund_request: true, reason: duplicate_charge, tone: annoyed}
    - id: cancelled-last-week
      state:
        message: I cancelled last week and still got billed. I want that month back.
      expect: {refund_request: true, reason: cancelled, tone: annoyed}
      tags: [billing]
    - id: how-does-billing-work
      state:
        message: Quick question, when in the month do you charge the card?
      expect: {refund_request: false, reason: none_of_these, tone: calm}
      note: about money, asks for none back; the case the threshold is tuned against
    - id: furious-outage
      state:
        message: Your service was down all day and I lost a client. This is unacceptable.
      expect: {refund_request: false, tone: angry}
      note: reason left unlabelled; a refund is implied, not asked for, and two labellers disagreed
```

When the questions change, bind the cases again. A label that no longer fits is refused with the case and the field named, and that refusal is the point: the cases are the part of a decision that notices when the decision moved.

## Next

- [Record, replay and test](record-replay-and-test.md#record-a-rubrics-cases-from-the-shell): `jud record` answers the cases once, with a key, and keeps the answers.
- [Tune thresholds](tune-thresholds.md): `jud eval` grades the answers against the labels, and `jud tune` reads the bars off them.
