---
title: Write a rubric
description: How to write a .jud Rubric from a brief, question by question, with a policy that is honest about its numbers, and check it with the crate's reader before it is used.
status: current
last_reviewed: 2026-10-08
tags: [judgment, jud, rubric, how-to]
---

# Write a rubric

**You will** turn a decision into a `.jud` Rubric that `jud check` accepts, with narrow questions, criteria the model and a reviewer read the same way, and a policy that says what it rests on. **Prerequisites:** the `jud` command ([Install](../start/install.md)); what a rubric is ([Rubrics, cases and recordings](../concepts/rubrics-cases-recordings.md)). The field rules are in [The .jud format](../reference/jud-format.md).

The example screens a support message for a refund request. The finished file is [`examples/jud/screening.jud`](../../examples/jud/screening.jud).

## 1. Start from the envelope

Every document has the same four top-level fields. Name the rubric; the name is what cases and recordings will refer to, so keep it stable.

```yaml
apiVersion: jud/v1.3
kind: Rubric
metadata:
  name: refund-screening
  description: Screen a support message for a refund request before a person reads it.
spec:
  questions: {}
```

A name is letters, digits, `.`, `_` and `-`, starting with a letter or a digit ([Names](../reference/jud-format.md#names)).

## 2. Write one question per thing

List what the application needs to know and cannot compute itself. Each becomes one question of one primitive ([System One](../concepts/system-one.md)). "Does it ask for a refund, and why, and how angry are they?" is three questions.

Give each outcome a criterion in plain words. `Does the message ask for money back?` reads differently to every grader; `true: a refund, a chargeback, a credit` reads the same to all of them, and to the model. Name the state key the question is about (`message`): the state is JSON and the model sees all of it.

```yaml
  questions:
    refund_request:
      type: noul
      instructions: Does `message` ask for money back?
      criteria:
        true: a refund, a chargeback, a credit, a cancelled charge
        false: a question about a charge, a complaint with no money asked for, anything else
```

For a Choice, offer a way out. A Choice with no option for "none of these" forces a wrong answer at high confidence.

```yaml
    reason:
      type: choice
      instructions: Why does the writer of `message` want money back?
      criteria:
        duplicate_charge: Charged twice, or charged after cancelling
        unhappy: The product or the service did not do what was expected
        cancelled: The writer cancelled and wants the remaining time refunded
        none_of_these: No refund asked for, or a reason not listed
      when: message
```

`when: message` asks the question only when the state carries that path; a state with no message has nothing to ask about ([State paths](../reference/jud-format.md#state-paths)). The options are sent in the order written.

For a Score, write the levels lowest first, as plain strings:

```yaml
    tone:
      type: score
      instructions: How upset is the writer of `message`?
      criteria: [calm, annoyed, angry]
```

Keep facts the application has out of the questions. Whether the account is on an annual plan, whether the charge is older than thirty days: compute them in code and put the result in the state.

## 3. Write the policy, and say it is a guess

One gate per question, fitting its primitive ([Policy](../reference/jud-format.md#policy)). A hand-written rubric has no `tuning` block; a `note` on each gate says why the bar is where it is, and "a guess" is an honest note. Never invent a `tuning` block: `jud tune` writes it from a real run ([Tune thresholds](tune-thresholds.md)).

```yaml
  policy:
    refund_request:
      threshold: 0.6
      note: a guess; a false positive costs a person a minute, a miss costs a customer
    reason:
      confidence: 0.5
      fallback: none_of_these
      note: a guess; below it a person reads the message
    tone:
      level_at_least: angry
      note: angry messages are read first
```

Keep the threshold out of the instructions. A question that says "answer yes only if you are quite sure" has moved the bar into the model, where it cannot be tuned.

## 4. Check it

```sh
jud check screening.jud
```

<!-- transcript: jud check examples/jud/screening.jud -->
```text
rubric    examples/jud/screening.jud: name refund-screening, jud/v1.3, 3 questions, 3 gates (untuned)
          questions sha256:92a19d0948fdaa410401d8c22b43d1783ac291b40af210d4aa4c81e8275e0e17
          policy    sha256:2c4cfcb3d2b6f200242a8589f22ca64d0627a611860f5498be20424a73261766
1 documents, 0 refused
```

A refusal names the file and the field. Fix it in the document, never around it: a label dropped or an option list widened to pass a check has changed the decision. The two fingerprints are what another document pins the rubric by; `(untuned)` says the policy carries no `tuning` block.

## 5. See what a state is asked

```sh
jud lower screening.jud --state '{"message": "I was charged twice for March."}'
```

prints the request the rubric lowers to for that state: the questions whose `when` holds, as the wire carries them. A question missing from the output was left out by its `when`; that is how a declaration is seen rather than guessed.

## The whole file

<!-- file: examples/jud/screening.jud -->
```yaml
# The rubric built step by step in docs/guides/write-a-rubric.md: whether a
# support message asks for money back, why, and how upset the writer is.
# Hand-written, so it carries no tuning block: the bars are stated guesses.
apiVersion: jud/v1.3
kind: Rubric
metadata:
  name: refund-screening
  description: Screen a support message for a refund request before a person reads it.
spec:
  questions:
    refund_request:
      type: noul
      instructions: Does `message` ask for money back?
      criteria:
        true: a refund, a chargeback, a credit, a cancelled charge
        false: a question about a charge, a complaint with no money asked for, anything else
    reason:
      type: choice
      instructions: Why does the writer of `message` want money back?
      criteria:
        duplicate_charge: Charged twice, or charged after cancelling
        unhappy: The product or the service did not do what was expected
        cancelled: The writer cancelled and wants the remaining time refunded
        none_of_these: No refund asked for, or a reason not listed
      when: message
    tone:
      type: score
      instructions: How upset is the writer of `message`?
      criteria: [calm, annoyed, angry]
  policy:
    refund_request:
      threshold: 0.6
      note: a guess; a false positive costs a person a minute, a miss costs a customer
    reason:
      confidence: 0.5
      fallback: none_of_these
      note: a guess; below it a person reads the message
    tone:
      level_at_least: angry
      note: angry messages are read first
```

## From questions written in code

The builder calls map one to one, because a question in a rubric is the wire's own shape: `questions.choice::<T>(id, instructions)` with an `options!` enum is a Choice whose `criteria` are the enum's keys and descriptions in declaration order; `questions.noul(id, instructions, None)` is a Noul with no `criteria`; `questions.score(id, instructions, levels)` is a Score with those levels. What the code does with an answer is the policy: `confidence.at_least(0.7)` is `confidence: 0.7`, `is_yes(0.6)` is `threshold: 0.6`, a strict `>` is `strict: true`. To prove parity, compare `jud lower`'s output with the request the code sends; the request fingerprint `jud check` prints for a recording is the one a `Replay` keys by.

## Next

- [Label cases](label-cases.md): the examples the bars will be tuned on. The loop after them is `jud record`, `jud eval` and `jud tune` ([Tune thresholds](tune-thresholds.md)).
- [Use the Claude Code plugin](use-the-claude-code-plugin.md), which writes and checks these files from a brief.
