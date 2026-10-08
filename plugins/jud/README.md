# jud

A Claude Code plugin for writing, measuring and tuning `.jud` documents, the file format of the
[judgment](https://github.com/chussenot/judgment) crate: a **Rubric** (the
typed questions a System One model is asked and the policy that reads its
answers), the **Cases** it is graded on, and a **Recording** of what a model
answered. The crate's own reader checks every document the plugin writes.

```sh
claude plugin marketplace add chussenot/judgment
claude plugin install jud@judgment
mise use -g github:chussenot/judgment@latest   # the `jud` command the plugin checks with
```

| What | Invoked as |
|---|---|
| The authoring skill: the workflow, the rules the reader enforces, how to write questions and cases that hold up, and which `jud` commands grade and tune them | loads when a task touches a `.jud` file, a rubric or labelled cases |
| Write a rubric from a brief | `/jud:rubric <brief>` |
| Write the cases a rubric is tuned on | `/jud:cases <rubric path> [brief]` |
| Check documents and explain every refusal | `/jud:check [files]` |
| The tuning skill: what each `jud eval` and `jud tune` number means, triaging misses into label, question, bar, coverage or model, applying a proposal in place | loads when a task is about how well a rubric works, its accuracy, its thresholds or its recordings |
| Record a model's answers to every case, after saying how many calls it spends | `/jud:record <rubric> <cases> [dir]` |
| Grade the recorded answers and triage every miss | `/jud:eval <rubric> <cases> [dir]` |
| Propose each gate's bar, decide gate by gate, and apply it with comments kept | `/jud:tune <rubric> <cases> <dir> [apply] [holdout]` |

[docs/guides/use-the-claude-code-plugin.md](https://github.com/chussenot/judgment/blob/main/docs/guides/use-the-claude-code-plugin.md)
says how it works and how it was tested; [docs/reference/jud-format.md](https://github.com/chussenot/judgment/blob/main/docs/reference/jud-format.md)
is the format's specification.
