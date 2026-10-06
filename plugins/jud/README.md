# jud

A Claude Code plugin for writing `.jud` documents, the file format of the
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
| The skill: the workflow, the rules the reader enforces, how to write questions and cases that hold up | loads when a task touches a `.jud` file, a rubric or labelled cases |
| Write a rubric from a brief | `/jud:rubric <brief>` |
| Write the cases a rubric is tuned on | `/jud:cases <rubric path> [brief]` |
| Check documents and explain every refusal | `/jud:check [files]` |

[docs/skill.md](https://github.com/chussenot/judgment/blob/main/docs/skill.md)
says how it works and how it was tested; [docs/jud.md](https://github.com/chussenot/judgment/blob/main/docs/jud.md)
is the format's specification.
