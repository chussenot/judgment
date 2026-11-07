# jud

A Claude Code plugin for writing, measuring and tuning `.jud` documents, and for using a rubric from Rust, the file format of the
[judgment](https://github.com/chussenot/judgment) crate: a **Rubric** (the
typed questions a System One model is asked and the policy that reads its
answers), the **Cases** it is graded on, and a **Recording** of what a model
answered. The crate's own reader checks every document the plugin writes.
Since 0.5.0 the plugin also runs a rubric on one state, tests a judge for
position, tone and grader-injection bias, compares two models, writes the
CI job that holds the labels, reviews a document's substance, serves
rubrics over MCP, ships the example documents it starts from, and checks
every `.jud` it writes through a hook. 0.4.0 taught it Jev 1.14: a Box
question, the images a document declares, labels graded by intersection
over union, and a typed module whose `decide` takes the images.

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
| Review a rubric and its cases for what the reader cannot see, ranked with each fix's cost | `/jud:review <rubric> [cases] [dir]` |
| Run a rubric on one state and explain every verdict | `/jud:run <rubric> <state> [--replay DIR]` |
| The tuning skill: what each `jud eval` and `jud tune` number means, triaging misses into label, question, bar, coverage or model, applying a proposal in place | loads when a task is about how well a rubric works, its accuracy, its thresholds or its recordings |
| Record a model's answers to every case, after saying how many calls it spends | `/jud:record <rubric> <cases> [dir]` |
| Grade the recorded answers and triage every miss | `/jud:eval <rubric> <cases> [dir]` |
| Propose each gate's bar, decide gate by gate, and apply it with comments kept | `/jud:tune <rubric> <cases> <dir> [apply] [holdout]` |
| Test a judge for position, tone and grader-injection bias | `/jud:bias <rubric> <cases> <dir> [swap] [tone] [injection]` |
| Compare two models' answers before labelling | `/jud:compare <rubric> <cases> <dir A> <dir B>` |
| Write the CI job that holds the labels | `/jud:ci [documents] [github \| sh]` |
| Serve a directory of rubrics as MCP tools (a `jud` built with `mcp`) | `/jud:serve <dir> [--replay DIR] [add]` |
| The Rust skill: a rubric as a small module a program wires in with `mod` and one `decide` call, enums for options and levels, a test that fails when the rubric drifts | loads when a task is about using a rubric from Rust |
| Write that module, compiled and tested before it is handed over | `/jud:rust <rubric> [out.rs] [embed]` |
| Hooks: `jud check` on every `.jud` written, and no edit to a file that reads `kind: Recording` | on by default; `disableAllHooks` in settings turns them off |

[docs/guides/use-the-claude-code-plugin.md](https://github.com/chussenot/judgment/blob/main/docs/guides/use-the-claude-code-plugin.md)
says how it works and how it was tested; [docs/reference/jud-format.md](https://github.com/chussenot/judgment/blob/main/docs/reference/jud-format.md)
is the format's specification.
