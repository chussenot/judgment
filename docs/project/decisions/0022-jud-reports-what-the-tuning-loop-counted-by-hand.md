---
title: 0022 jud reports what the tuning loop counted by hand
description: Accepted and implemented; after a real-world run, three additions move into the jud command what the tuning skill and its users computed by hand. jud eval reports each question's answers by outcome, its majority label and its signals (no better than the majority, collapsed, an outcome never answered, a gate that defers most or nearly all) with the thresholds they were read with; jud record --dry-run says what a run would ask, of whom and for how long; jud split writes a tuning set and a held-out set, each case copied as read. Why the command and not the skill, the thresholds chosen, and what it costs.
status: accepted
date: 2026-10-09
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-09
tags: [decisions, jud, cli, evaluation, tuning, recordings]
---

# 0022 jud reports what the tuning loop counted by hand

**Status: accepted.** Implemented in the `jud` binary on 2026-10-09, with [issue #46](https://github.com/chussenot/judgment/issues/46).

## Context and problem statement

[Decision 0021](0021-record-eval-and-tune-from-the-command-line.md) put the loop from labelled cases to a tuned policy on the command line, and the Claude Code plugin's `jud-tune` skill drives it. The first run of both on real data outside the fixtures was a rubric over Wikimedia's recentchange stream: 93 captured cases, recorded against `tev1:0.8b` on Ollama, graded, split and tuned. It found what the commands did not say and the skill then had to compute by hand, with scripts the plugin cannot ship:

- **No baseline.** `harm_risk` was 0.63 accurate; always answering its commonest label, `none`, scores 0.66. Nothing in the report said so; the counts per label had to be taken from the cases document.
- **A collapse read as many misses.** `change_kind` answered `content` on 71 of 78 labelled cases. The report listed 49 separate misses; that they were one cause took a count of the predictions.
- **A gate that deferred everything.** `harm_risk`'s gate deferred 90 of 93 answers, so the policy handed the question to its fallback; the gate line showed the two numbers and nothing about what they meant.
- **The cost of a run.** Before `jud record` the skill counts the requests, matches the recordings in the directory with `jud check`, reads the backend from `jud config`, and estimates the time from nothing.
- **The held-out set.** The skill says, in so many words, "jud has no split command: copy each case block verbatim". An agent copying 93 cases by hand into two files is where a state gets edited on the way.

The plugin's own record of its battle test says the same: most of the claims graders found the output contradicting were "arithmetic done by hand on label counts, which `jud eval --json` does not yet report per outcome".

## Decision drivers

- **A count is the command's job.** A number the report could give is one an agent or a person should not compute: the command has the judgments, the reader has to rebuild them.
- **Data, not advice.** The command reports what the numbers are and names a pattern by a code; what to do about it stays with the skill and the person.
- **Nothing new to trust.** No new metric with a theory behind it: shares of counts already in the report, with the thresholds stated.
- **Additive.** No key, status or meaning changes; the stability page lists additions under Added.

## Considered options

1. Leave it to the skill, which can compute all of it from `--json` and the cases document.
2. Add the counts and signals to `jud eval`, a `--dry-run` to `jud record`, and a `jud split`.
3. A separate `jud report` that reads `eval --json` and adds the analysis.

## Decision outcome

Option 2.

### `jud eval`: outcomes, majority and signals

Each question's block gains, for every outcome the question offers (`yes` and `no`, a Choice's options in order, a Score's levels), how many labels name it, how often the model answered it on a labelled case, and how often rightly; the majority label and its share; and one `warning:` line per signal. `--json` carries them as `outcomes`, `majority` and `signals`, the last a list of codes, and the thresholds as `signal_rules`:

| Code | Raised when | Read from |
|---|---|---|
| `no_better_than_majority` | the accuracy's 95 % interval reaches down to the majority's share, and the labels name more than one outcome | ten labelled answers or more |
| `collapsed` | one answer takes at least 0.8 of the labelled answers and at least 0.2 more than the largest share any label has | ten labelled answers or more |
| `never_answered` | an outcome at least 3 labels name is never the model's answer | ten labelled answers or more |
| `defers_most` | the gate defers at least 0.5 of the answers it sees, and less than 0.9 | ten answers or more |
| `defers_nearly_all` | the gate defers at least 0.9 of the answers it sees | ten answers or more |

The margin in `collapsed` keeps a model that rightly answers a set that is 90 % `no` from being called collapsed. The minimum of ten keeps a share read off a handful of answers from raising any signal.

Three of these were changed in review, from what a live run against `jev-1.13.0` and the mock's profiles showed:

- **`no_better_than_majority` waits for ten labels.** Without a minimum it fired on nearly every small set, perfect scores included: on the seven triage cases `tone` scored 6 of 6 and was flagged, since a Wilson interval on six answers cannot clear a 0.67 majority. A perfect model needs ten cases to clear a 0.71 majority and thirty-five to clear 0.9. A question whose labels are all one outcome is never flagged: nothing can beat 1.0.
- **`never_answered` covers the collapse a skewed set hides.** A model that always said `yes` to a set that is 37 of 42 `yes` answered one outcome 0.12 more often than the labels named it, under the 0.2 margin, so `collapsed` stayed silent, and only `no 5/0/0` in the outcomes line showed it.
- **`defers_most` at 0.5.** On the live support recordings, the bar `jud tune` proposed for `urgency` (0.90) deferred 36 of 48 answers, and 0.97 deferred 38: a capable model's confident tail puts a bad bar between a half and nine tenths, which `defers_nearly_all` never sees. The Wikimedia gate, 90 of 93 on a small model always unsure, is what the 0.9 line was drawn from.

### `jud record --dry-run`

Reads what a run reads, the documents bound, every request lowered and the directory's recordings matched by fingerprint, and stops. It prints the requests, how many are recorded, which are to ask and which stale ones to replace, the backend and model with their source, whether a key is set, and the time at the median `elapsed_ms` of the recordings already in the directory. It asks nothing, creates nothing and needs no key.

### `jud split CASES [--every N] [--out DIR]`

Writes `STEM-tune.jud` and `STEM-holdout.jud`, the Nth, 2Nth, ... case held out (4 by default), each case copied as read (its values; comments are not kept) so the recordings made over the whole set answer both halves. It refuses an existing file and the input, so it writes over nothing, and warns when a label one half has is missing from the other. A `from_turn` label counts as the turns it labels; with `--rubric`, a Score level written as its index and as its text is one level. On the Wikimedia cases its halves have exactly the fingerprints of the split the skill had made by hand.

## Pros and cons of the options

**1. Leave it to the skill.** Nothing changes in the crate. But every agent recomputes, and gets wrong, numbers the command already holds; a person without the plugin gets none of it; and the split, the step most likely to alter a state, stays manual.

**2. In the commands.** The numbers come from the judgments the report is built from, so the counts add up by construction and the text and JSON cannot disagree. The cost is the report's length (two lines and the warnings per question), three thresholds that become part of the command's behaviour, and one more place that writes files.

**3. A separate report.** Keeps `jud eval` short, but splits one report in two, and the second must re-read what the first already had.

## More information

The thresholds are constants in `src/bin/jud/eval.rs`, named on [the command line reference](../../reference/cli.md#the-text-report) and written into every JSON report as `signal_rules`; a change to one changes when a code is raised, which is a change of meaning under [Stability](../../reference/stability.md), and two reports read under different ones say so in their own data. The skill that used to compute these, `plugins/jud/skills/jud-tune`, now reads them from the report.

Confirmed in `tests/jud_eval.rs` (the outcomes add up to `labelled` and `correct`, the documented keys, a collapse and a deferring gate raised against a server that always gives one answer), the signal unit tests in `src/bin/jud/eval.rs` (a skewed set answered rightly is no collapse, nothing raised under ten answers), `tests/jud_record.rs` (a dry run counts, names stale recordings, writes nothing and needs no key), and `tests/jud_split.rs` (the halves put back in order are the whole set, recordings over the whole set answer both, nothing is written over).
