# judgment

A Claude Code plugin for using the [judgment](https://github.com/chussenot/judgment)
crate from Rust: asking a System One model typed questions and acting on
its calibrated answers, testing the code that does with no key and no
network, and observing and pacing its calls. Its reference pages are the
crate's own documentation, generated into the plugin. The `jud` plugin in
the same marketplace covers `.jud` rubrics and the `jud` command, the
`jud-rust` module included.

```sh
claude plugin marketplace add chussenot/judgment
claude plugin install judgment@judgment
```

| What | Invoked as |
|---|---|
| The integration skill: questions and typed handles, the answers, `Response::verify`, the error groups by what fixes them, the client and its retries, choosing a backend | loads when Rust code calls or should call the crate |
| Write the code for a decision, with its test, compiled and linted | `/judgment:integrate <brief>` |
| The testing skill: `Fake`, `Recorder` and `Replay`, wiremock for the client, no real API | loads when a task is about testing, mocking or replaying calls |
| Write and run the tests a call site needs | `/judgment:test <file> [record]` |
| The observability skill: `Observer`, `OtelObserver`, the spans, `Pacer` and `Paced` | loads when a task is about metrics, cost, tracing or many calls at once |
| Wire an observer and a pacer, and test the counts | `/judgment:observe <module> [otel] [pacer N]` |

[docs/guides/use-the-claude-code-plugin.md](https://github.com/chussenot/judgment/blob/main/docs/guides/use-the-claude-code-plugin.md)
says how both plugins were tested.
