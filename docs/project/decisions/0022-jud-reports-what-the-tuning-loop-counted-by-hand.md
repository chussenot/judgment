---
title: 0022 jud reports what the tuning loop counted by hand
description: Accepted and implemented; after a real-world run, three additions move into the jud command what the tuning skill and its users computed by hand. jud eval reports each question's answers by outcome, its majority label and three signals (no better than the majority, collapsed, a gate that defers nearly all); jud record --dry-run says what a run would ask, of whom and for how long; jud split writes a tuning set and a held-out set verbatim. Why the command and not the skill, the thresholds chosen, and what it costs.
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

Each question's block gains, for every outcome the question offers (`yes` and `no`, a Choice's options in order, a Score's levels), how many labels name it, how often the model answered it on a labelled case, and how often rightly; the majority label and its share; and one `warning:` line per signal. `--json` carries them as `outcomes`, `majority` and `signals`, the last a list of codes:

| Code | Raised when | Read from |
|---|---|---|
| `no_better_than_majority` | the accuracy's 95 % interval reaches down to the majority's share | any labelled count |
| `collapsed` | one answer takes at least 0.8 of the labelled answers and at least 0.2 more than the largest share any label has | ten labelled answers or more |
| `defers_nearly_all` | the gate defers at least 0.9 of the answers it sees | ten answers or more |

The margin in `collapsed` keeps a model that rightly answers a set that is 90 % `no` from being called collapsed. The minimum of ten keeps a share read off a handful of answers from raising either share-based signal; `no_better_than_majority` needs no minimum, because a wide interval is what it reports.

### `jud record --dry-run`

Reads what a run reads, the documents bound, every request lowered and the directory's recordings matched by fingerprint, and stops. It prints the requests, how many are recorded, which are to ask and which stale ones to replace, the backend and model with their source, whether a key is set, and the time at the median `elapsed_ms` of the recordings already in the directory. It asks nothing, creates nothing and needs no key.

### `jud split CASES [--every N] [--out DIR]`

Writes `STEM-tune.jud` and `STEM-holdout.jud`, the Nth, 2Nth, ... case held out (4 by default), each case copied as read so the recordings made over the whole set answer both halves. It refuses an existing file and the input, so it writes over nothing, and warns when a label one half has is missing from the other. On the Wikimedia cases its halves have exactly the fingerprints of the split the skill had made by hand.

## Pros and cons of the options

**1. Leave it to the skill.** Nothing changes in the crate. But every agent recomputes, and gets wrong, numbers the command already holds; a person without the plugin gets none of it; and the split, the step most likely to alter a state, stays manual.

**2. In the commands.** The numbers come from the judgments the report is built from, so the counts add up by construction and the text and JSON cannot disagree. The cost is the report's length (two lines and the warnings per question), three thresholds that become part of the command's behaviour, and one more place that writes files.

**3. A separate report.** Keeps `jud eval` short, but splits one report in two, and the second must re-read what the first already had.

## More information

The thresholds are constants in `src/bin/jud/eval.rs` and named on [the command line reference](../../reference/cli.md#the-text-report); a change to one changes when a code is raised, which is a change of meaning under [Stability](../../reference/stability.md). The skill that used to compute these, `plugins/jud/skills/jud-tune`, now reads them from the report.

Confirmed in `tests/jud_eval.rs` (the outcomes add up to `labelled` and `correct`, the documented keys, a collapse and a deferring gate raised against a server that always gives one answer), the signal unit tests in `src/bin/jud/eval.rs` (a skewed set answered rightly is no collapse, nothing raised under ten answers), `tests/jud_record.rs` (a dry run counts, names stale recordings, writes nothing and needs no key), and `tests/jud_split.rs` (the halves put back in order are the whole set, recordings over the whole set answer both, nothing is written over).
