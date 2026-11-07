---
description: Test a Choice judge for position, tone and grader-injection bias (jud eval --swap, --tone-perturb, --injection-perturb), writing the perturbation files when none is given, recording the perturbed requests after announcing the cost, and reading the flip rates and the attack success
argument-hint: "<rubric> <cases> <recordings dir> [swap] [tone [FILE]] [injection [FILE]] [--confident-at TAU]"
---

Test this judge for bias, on the tests named (all three when none is):

$ARGUMENTS

Follow the jud-tune skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`
first ("How to run them", "What costs what"), then
`${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/references/bias.md` (what each
test perturbs, how to write a tone or injection file, how to read the
table) and `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/references/record.md`
(announcing a run's cost).

Run every `jud` command through `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`,
written out in full, one command per Bash call, nothing chained, no
redirection into a file. Read files with the Read tool (the shipped
examples under `${CLAUDE_PLUGIN_ROOT}/skills/jud/references/examples/`
too, never with `cat` or `head`), write with the Write tool.

1. **Resolve the arguments.** The rubric, the cases and the recordings
   directory are required; the originals must be recorded there already
   (`jud.sh check <rubric> <cases> <dir>/*.jud`; a case as written with
   no recording is recorded first, announced as `/jud:record` announces).
   The tests are `swap`, `tone` and `injection`; `tone FILE` and
   `injection FILE` name a file the user wrote.
2. **Check the shape before anything is written or asked.** Read the
   rubric: the test applies to a Choice whose options are the answers
   being compared. `swap` and `tone` take any Choice of two or more
   options that the cases label with an option; `injection` takes only a
   Choice of exactly two options, each case labelling one, whose losing
   option's description is text. When the rubric has no such question,
   say which rule it fails (`queue` offers five options; `tone` labels a
   Score) and stop: `jud` would refuse the file before any call, and a
   file written for it would land nowhere.
3. **Write the file the user did not give**, as `references/bias.md`
   says: `<cases stem>-tone.yaml` or `<cases stem>-injection.yaml` beside
   the cases document, one entry per labelled case for `tone` (the
   content kept, a tone around it, the four tones in turn), and for
   `injection` the three payloads of arXiv 2611.00505 in turn or the
   user's own, appended to nothing yet (the command appends). Show the
   file, or its first entries and the count, before any call.
4. **Plan and announce.** `jud.sh record <rubric> <cases> --out <dir>
   --dry-run` with the test flags (`--swap`, `--tone-perturb FILE`,
   `--injection-perturb FILE`): the requests to ask are the perturbed
   ones the directory does not hold. Announce the cost as `/jud:record`
   does, in the Bash call's description and as text before it:
   `Recording <n> perturbed requests (<swapped> swapped, <toned> toned,
   <injected> injected; <kept> kept, in <dir>) to <base_url>, model
   <model>.` Then record. A refusal (status 2 naming a case, a question
   or an empty note) is the file's: fix the entry, never the cases.
5. **Evaluate from the recordings.** `jud.sh eval <rubric> <cases>
   --replay <dir> --json` with the same test flags and `--confident-at`
   when given; without it, pass the gate's `confidence` bar for the
   question as `--confident-at` and say so, so the confident column says
   how often a verdict the gate would act on moves. Run once more without
   `--json` for the table's text.
6. **Reply**, from the JSON (`robustness`), per question and test:
   - the robustness table as `eval` printed it;
   - the flip rate with its interval and counts, and the cases that
     flipped by name; a flip that corrected a wrong verdict counted as
     such;
   - the confident flip rate at the bar used, in one sentence about the
     gate ("of the 9 verdicts the gate at 0.6 would act on, 2 moved under
     the tone");
   - the mean |Δp|;
   - the accuracy as written and perturbed, with intervals;
   - under `injection`, the attack-success line apart from the flip rate
     (of the verdicts right as written, those the note turned wrong),
     with the interval, and that it measures the payload supplied, not
     the model's robustness to every note;
   - what the perturbed recordings are named (`CASE.swapped.jud`,
     `.toned.jud`, `.injected.jud`) and that `eval` replays them with the
     same flags at no cost;
   - ranked next steps: an instruction that says order and tone are not
     evidence (one re-recording), a higher bar (free, defers), more pairs
     (the interval at twelve pairs reaches from 0 to 0.26 on no flip).

Never edit a recording, the rubric or the cases; never write a file into
the project but the perturbation file of step 3.
