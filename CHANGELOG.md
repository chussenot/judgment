# Changelog

All notable changes to the `judgment` crate. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the crate follows
[Semantic Versioning](https://semver.org/) (0.x: a minor bump may break).

## [Unreleased]

## [0.12.0] - 2026-10-09

### Added

- `jud eval` reports, per question, its answers by outcome (how many labels
  name each, how often the model gave it, how often rightly), the majority
  label with its share, and five signals: `no_better_than_majority`,
  `collapsed`, `never_answered`, `defers_most` and `defers_nearly_all`, each a
  `warning:` line in the text and a code in the new `outcomes`, `majority`
  and `signals` keys of `--json`, none read from fewer than ten answers. The
  thresholds they were read with are the new `signal_rules` key, so a report
  says which it used (decision 0022). They are what the tuning skill
  computed by hand on the first real-world run (#46).
- `jud record --dry-run` says what a run would do and does nothing: the
  requests, how many the directory answers, which are to ask or replace,
  the backend and model, whether a key is set, and the time at the median
  pace of the recordings already there. No key, no call, no file. It counts
  two cases that lower to one request once, and refuses what the run would
  refuse before its first call.
- `jud split CASES [--every N] [--out DIR]` writes a tuning set and a
  held-out set, every Nth case held out, each case copied as read so the
  recordings over the whole set answer both. It writes over nothing and
  warns when a label one half has is missing from the other. A `from_turn`
  label counts as the turns it labels, and with `--rubric RUBRIC` a Score
  level written as its index and as its text is one level.

### Changed

- The Claude Code plugin (now 0.3.1) carries what a first real-world run
  taught it (#46): a rubric over Wikimedia's recentchange stream, 93 cases
  captured from it, recorded and tuned against `tev1:0.8b` on Ollama, and a
  Rust loop judging the live stream. `jud-tune` now reports each question
  against always giving its commonest answer, names a question that has
  collapsed onto one answer, flags a gate that defers nearly every case,
  states a local server's cost as time, and allows an announced subset
  experiment on a copy to measure a question change. The `jud` skill and
  `/jud:rubric` ask for the field each answer is read from and leave to code
  what machine-written fields decide; `/jud:cases` keeps captured states
  verbatim and labels from the best evidence, an outcome known later
  included. `jud-rust` gives the `use` path for a module mounted from
  `lib.rs` and says that every `decide` is one request a stream caller must
  bound. The mock System One server gains `collapse` and `confidence`
  profile keys, and a new `jud-tune` scenario uses them.

### Fixed

- `/jud:rust` no longer writes a `Cargo.lock` (or a `target/`) into the
  user's crate when that crate already mounts the module: it runs `cargo
  test` there only when a lock file exists, with a target directory outside
  the project.

### Added

- The Claude Code plugin (now 0.3.0) turns a rubric into a typed Rust module:
  the `jud-rust` skill and `/jud:rust <rubric>`. The module reads the rubric
  with `include_str!` and lowers the request with `Rubric::lower`, so it sends
  what `jud lower` sends and a retune needs no regeneration. It gives each
  Choice and Score an enum and the program one call, `decide(backend, model,
  state)`, and its own test fails when the rubric's questions drift from it.
  `examples/jud_typed.rs` wires the two template modules
  (`examples/jud/triage.rs`, `examples/jud/routing.rs`) into a program and
  tests them; `scripts/gen-plugin-rust.sh` keeps the plugin's copies equal
  to them.

### Added

- The Claude Code plugin (`plugins/jud/`, now 0.2.0) measures and tunes
  rubrics as well as writing them: a `jud-tune` skill and three commands.
  `/jud:record` says how many calls a run spends, to which backend and model,
  before running `jud record`, and resumes what a directory already holds.
  `/jud:eval` grades the recordings and triages every miss into a wrong
  label, an ambiguous question, the bar's job, a coverage gap or the model,
  proposing each fix without applying it. `/jud:tune` decides gate by gate
  whether to take what `jud tune` proposes (never a bar of 0 on a handful of
  cases, never a bar that absorbs a wrong label), and with `apply` writes it
  into the rubric in place with its comments kept, then shows each gate
  before and after; with `holdout` it tunes on three quarters of the cases and
  reports the rest. The commands were battle-tested headless (`claude -p
  --plugin-dir`) against a mock System One server that knows the labels
  (`plugins/jud/skills/jud-tune/evals/`); no test calls a real API.

## [0.11.1] - 2026-10-08

### Fixed

- `jud completion zsh` completes a subcommand's own arguments. The script
  clap generates declared the optional `RUBRIC` before the subcommand, so in
  `jud eval <TAB>` zsh read `eval` as the rubric and offered the subcommand
  names again; `check`, `lower`, `record`, `eval` and `tune` completed
  neither their files nor their flags. The first word now offers a
  subcommand or a file, and each subcommand completes its own arguments.
  Bash, fish, elvish and PowerShell were not affected. Regenerate the
  installed script (`jud completion zsh > ...`, or `mise run install` in a
  checkout) to pick it up.

## [0.11.0] - 2026-10-08

### Added

- `jud record`, `jud eval` and `jud tune` run a rubric over its labelled
  cases from the shell (decision 0021; `docs/reference/cli.md`). `jud record
  RUBRIC CASES --out DIR` asks the configured backend once per case, or once
  per turn for a conversation labelled with `from_turn`, and writes each
  answer as a `.jud` recording. It keeps what `DIR` already answers, so an
  interrupted run resumes, and `--refresh` asks again. The server a
  recording names is the base URL (`TYPESAFE_BASE_URL`, or `base_url` in the
  configuration file) without its userinfo, query and fragment, so a
  credential kept in the URL is never written to a recording, and `jud tune`
  strips a recording's server the same way for its `tuning` block. `jud eval
  RUBRIC CASES [--replay DIR]` grades the answers against the labels and
  reports, per question, the model's accuracy with its 95 % interval, its
  Brier score and calibration error, how often the policy's gate acts and how
  often its verdict is right, and every miss, as text or as one JSON object
  (`--json`). `--min-accuracy [QUESTION=]ACCURACY` holds the model's
  accuracy per question and makes `jud eval` exit with the new status 3 when
  a bar is not met. `jud tune RUBRIC CASES --replay DIR` reads recordings
  only, prints each gate's sweep on stderr and proposes the bars as the
  `policy` and `tuning` blocks on stdout; `--out PATH` writes the whole
  rubric instead.

  Contract: exit status 3 (`jud eval` alone), the keys of `jud eval --json`
  (`rubric`, `cases`, `requests`, `models`, `questions`, `min_accuracy`) and
  the two places that write files, `jud record --out` and `jud tune --out`,
  join the command's contract (`docs/reference/stability.md`). Neither
  command writes over a rubric or a cases document it was given, and `jud
  tune --out` never writes among the recordings. Every other command stays
  read-only. stdout is the result and nothing else; progress, tables and
  failures go to stderr. Statuses 0, 1 and 2 keep their meaning.

### Changed

- The documentation is rebuilt in Diátaxis form under `docs/`: tutorials
  (`start/`), guides (`guides/`), reference (`reference/`), concepts
  (`concepts/`) and the project's own pages (`project/`, where the
  decision, verification and research records moved unchanged). The `.jud`
  specification is `docs/reference/jud-format.md`, in BCP 14 wording, and
  the plugin's `references/format.md` is generated from it; the command
  line's reference is generated from the binary's help; the README is a
  front door. Every Rust block, whole `.jud` document and marked transcript
  on a page is held to the code by a test, and every internal link is checked
  (`mise run docs:links`). The rebuild itself changed no behaviour, API or
  format rule.
- `Replay::open` reads a directory in name order, so that when two files
  record one request the same one answers on every file system, and a
  `.json` recording that does not parse is `Error::InvalidRecording`, which
  names the file, instead of `Error::Decode`. **Breaking** for a caller that
  matched `Error::Decode` there.
- `jud RUBRIC`, `jud check`, `jud lower` and `jud config` no longer panic
  (status 101) when stdout is closed (`jud check FILE | head -0`): a reader
  that has gone is not an error, and the status is the command's own, as it
  already was for `jud completion` and is for `jud record`, `jud eval` and
  `jud tune`.
- Messages of the existing paths, each with its status unchanged:
  - A failure of `jud RUBRIC` names the backend as `scheme://host/path`,
    without the userinfo, query or fragment of its base URL, so a credential
    kept in the URL is not printed.
  - A misplaced `--replay` gets a hint. `jud --replay DIR eval ...` says the
    flag belongs after the subcommand, and `jud --replay DIR` alone says a
    rubric is required; both are status 2 as before.
  - A mistyped bare subcommand (`jud evaluate`) reads as a rubric path that is
    not there, and the message now adds `(not a subcommand either: ...)` with
    the subcommands' names.
  - Under `jud RUBRIC --replay DIR`, a recording that matches the state and
    the rubric but no longer fits the questions is reported as `a recording
    under DIR matches this state and rubric but cannot answer it`, where the
    message said that no recording answers; status 1 as before.
  - `jud check` points a document that is not `jud/v1.3` at
    `docs/reference/jud-format.md`, where it named the page's old path,
    `docs/jud.md`.

## [0.10.4] - 2026-10-07

### Added

- A container image, `ghcr.io/chussenot/jud`, for `linux/amd64` and
  `linux/arm64`: `cat event.json | docker run -i --rm -v "$PWD:/work"
  ghcr.io/chussenot/jud triage.jud`. `FROM scratch` with the release
  tarball's binary, a CA bundle for the client's TLS and a non-root user,
  about 10 MB; tagged `X.Y.Z`, `X.Y` and `latest`, with a build-provenance
  attestation. The release workflow's new `image` job fills the build
  context from the tarballs it just built (`scripts/image-context.sh`),
  runs the image on the runner and pushes it after the publish
  ([The jud command line](docs/reference/cli.md),
  [Releasing](docs/project/releasing.md)).

## [0.10.3] - 2026-10-07

### Added

- `cargo binstall judgment` installs the release's `jud` tarball instead of
  compiling: `[package.metadata.binstall]` in `Cargo.toml` names the asset
  and the binary's path inside it, and the release workflow checks, on every
  platform it packages, that the templates name the tarball it just built
  ([The jud command line](docs/reference/cli.md)).
- A Homebrew formula for `jud`: `brew install chussenot/tap/jud` on Apple
  silicon and Linux, from [chussenot/homebrew-tap](https://github.com/chussenot/homebrew-tap).
  The formula installs the release tarballs by their checksums and generates
  the shell completions from the binary. It is rendered from each release's
  `SHA256SUMS` by `scripts/homebrew-formula.sh` and committed to the tap by
  the release workflow's new `homebrew` job, so the tap's version is the
  release's ([The jud command line](docs/reference/cli.md),
  [Releasing](docs/project/releasing.md)).

## [0.10.2] - 2026-10-07

### Changed

- A kind's reader (`Rubric::parse`, `Cases::parse`, `jud::parse_recording`)
  handed a document of another known kind refuses it with
  `jud::Error::OtherKind { found, expected }`, "a Cases document, not a
  Rubric; this reads `kind: Rubric`", instead of `Error::Kind`, which said
  the kind was not a document kind at all. `Error::Kind` still names a kind
  the crate does not know.

### Fixed

- `jud lower` and `jud check` name the file in every refusal and say what
  was expected of it: `cannot read rubric PATH`, `PATH is not a valid
  Rubric: ...`, `--state is not JSON`, and, for a file that is not a `.jud`
  document at all, the pointer to the envelope that `jud RUBRIC` already
  gave. `jud lower examples/jud/triage-cases.jud` used to print "`kind:
  Cases` is not a document kind" with no path.

## [0.10.1] - 2026-10-07

### Added

- `jud --replay DIR RUBRIC`, or `JUD_REPLAY=DIR`, answers from the recordings
  under a directory through the crate's `Replay` backend instead of a server:
  no key, no network, a state nobody recorded is an error. The README's
  command-line transcript runs this way over `examples/recordings/` and
  `tests/jud_cli.rs` holds it to what the binary prints.
- `docs/reference/stability.md`: what is promised across versions. `jud/v1` is stable, a
  minor version only adds, readers accept every `v1` document for at least
  twelve months after a later minor is published, fingerprints exclude the
  envelope and never change for a document's `spec`; the crate's 0.x rule and
  the command's contract.
- Decision 0020, proposed and not applied: whether a `v1` minor stays in the
  `apiVersion` string or moves to a `minor` field under `apiVersion: jud/v1`,
  each option priced.
- `scripts/record_demo.sh` (`mise run demo`) records the command-line demo as
  an asciinema cast, `docs/demo.cast`, from the recordings, with no network;
  regenerated by the release bump so the version it shows is the release's.
- `mise run ci` is an alias of `mise run check`, the gates in CI order.
### Changed

- The README opens with the command line for 0.10: a one-line pitch, the mise
  install, the `cat event.json | jud triage.jud` transcript with the verdicts
  the binary prints for `examples/jud/triage.jud`, then the Rust API; one
  comparison table of the four properties where the clients differ, the full
  grids linked; the live example's capture trimmed to the command and its
  output.
## [0.10.0] - 2026-10-07

### Added

- `jud completion <shell>` prints a completion script for bash, zsh, fish,
  elvish or PowerShell, generated from the command tree the binary parses
  with, so it cannot drift from the binary; `docs/reference/cli.md` says where each
  shell wants it, and `mise run install` refreshes the scripts a shell
  already has. The binary's arguments are now parsed by `clap`: `jud --help`
  and every subcommand's `--help` are derived from the same tree, a usage
  error names the argument and exits 2 as before, and `jud lower` refuses
  `--state`, `--state-file` and `--cases` together instead of taking the
  last.

### Changed

- The `jud` binary builds behind its own `cli` feature (`jud`, `http`,
  `clap`, `clap_complete`): `cargo install judgment --features cli`. The
  `jud` feature is the format alone again, so a library build with it
  compiles no command-line parser.

## [0.9.0] - 2026-10-06

### Added

- `jud RUBRIC` evaluates the JSON state on stdin against a `.jud` Rubric and
  prints one verdict per question as JSON (`cat input.json | jud rubric.jud`,
  `jq '.customer' event.json | jud customer.jud`), through the crate's
  client: TypeSafe by default, any System One server by `TYPESAFE_BASE_URL`
  or `base_url` in `~/.config/jud/config.yaml` (`api_key`, `model`,
  `timeout_secs` too; the environment wins). `jud config` prints the
  resolved backend, `jud --version` the version. Exit status 0 on verdicts,
  1 when the backend call failed, 2 when the file, the input or the
  configuration is wrong. The binary now needs the `http` feature beside
  `jud` (both on for `cargo install judgment --features jud`), and `tokio`
  gains `rt` under `http`.
- Release binaries: a pushed version tag builds, checks and packages `jud`
  for `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` and
  `aarch64-apple-darwin`, attaches the tarballs and `SHA256SUMS` with a
  build-provenance attestation to the GitHub release, so
  `mise use -g github:chussenot/judgment@latest` installs it without a Rust
  toolchain (decision 0019; `docs/reference/cli.md`). `mise run install` installs it
  from a checkout. A `workflow_dispatch` of the release workflow is a dry
  run that builds every leg and publishes nothing.

## [0.8.0] - 2026-10-06

### Added

- The `jud` binary (feature `jud`; `cargo install judgment --features jud`):
  `jud check FILE...` reads documents as the crate does, binds cases to the
  rubric they name and verifies a recording against the request it answers,
  printing names, the `apiVersion` and fingerprints and exiting 1 on a
  refusal; `jud lower RUBRIC` prints the request a state or every case
  lowers to. `mise run jud:check` runs it over the examples or the files
  given.
- A Claude Code plugin, `plugins/jud/`, served by this repository as a
  marketplace (`.claude-plugin/marketplace.json`; `claude plugin
  marketplace add chussenot/judgment`, `claude plugin install
  jud@judgment`): the `jud` skill for writing, reviewing and fixing `.jud`
  documents, with a one-page field reference, the checker as a script and
  the six tasks it was tested on, and three commands, `/jud:rubric`,
  `/jud:cases` and `/jud:check`. `docs/guides/use-the-claude-code-plugin.md` says how it works and how
  to install it.

### Changed

- **Breaking:** the `.jud` envelope is a manifest's (decision 0018;
  `docs/reference/jud-format.md`): `apiVersion: jud/v1.3`, `kind: Rubric`, `Cases` or
  `Recording`, `metadata` (`name`, required on every kind; `version`,
  `description`, `labels`, `annotations`) and `spec`, which holds every
  field a kind had at the top level. A rubric's or a cases document's `id`
  and a recording's `case` are `metadata.name`, a cases document's
  `rubric` is `spec.rubric`, and the top-level `x-` keys are
  `metadata.annotations`. The reader reads exactly `jud/v1.3` and refuses
  any other `apiVersion` by name: there is no compatibility with the
  `jud: 1`, `1.1` and `1.2` envelopes, and a document is moved forward by
  rewriting its envelope. In the crate, `jud::API_VERSION` replaces
  `jud::VERSION` and `jud::MINOR`; `Rubric::id` is `name` and
  `Rubric::extensions` is `labels` and `annotations`
  (`IndexMap<String, String>`); `Cases::id` is `name`, now a required
  `String`, and `Cases::extensions` likewise; `Rubric::new(name, questions)`.
  Every document under `examples/jud/` and
  `examples/recordings/jud_calibration/`, the rubric in
  `examples/jud_quickstart.rs` and the README, and the JSON Schemas carry
  the new envelope. No fingerprint changes: every fingerprint is over a
  value inside `spec`, and the envelope is part of none.

## [0.7.0] - 2026-10-06

### Added

- `.jud` version 1.2 (decision 0017; `docs/reference/jud-format.md`): no new field. Every
  id is a name (letters, digits, `.`, `_`, `-`, starting with a letter or
  a digit), so a case id never reaches the file system as a path; a merge
  key and a tag the core schema does not define are refused with their
  position, and a `!!binary` scalar is its text, never decoded; a syntax
  error names a line and a column and quotes nothing. `Rubric::policy_fingerprint`
  is the fingerprint of the gates alone, so a moved threshold is as visible
  as a changed question. `eval::is_name` states the grammar.

### Changed

- Every document under `examples/jud/` and `examples/recordings/jud_calibration/`,
  and the rubric in `examples/jud_quickstart.rs` and the README, declares
  `jud: 1.2`; no fingerprint changes, the version being part of none.
  `examples/jud_calibration.rs` prints the policy fingerprint beside the
  questions'.
- The `.jud` reader applies the 1.2 rules to every version it reads: a
  document that used a merge key, a foreign tag or a path-shaped id is
  refused, naming the position or the field. `eval::write_recording` and
  `eval::read_recording` refuse a case id that is not a name
  (`eval::Error::NotAName`). `Replay::open` skips symlinks and
  subdirectories, and a replay looks a request up by its SHA-256
  fingerprint before the FNV request hash.

## [0.6.1] - 2026-10-06

### Changed

- The examples' recordings live under `examples/recordings/<example>/`,
  one directory per example named after it, instead of beside each
  example under its own name; the pattern examples record and replay
  through one shared module, `examples/common/mod.rs`, so every example
  writes its recordings the same way to the same place.

### Added

- Two more README examples, each held to its file by `tests/readme.rs`:
  `examples/jud_quickstart.rs`, the quickstart's two questions as a `.jud`
  rubric with their thresholds, answered by the `Fake` backend and read
  through the rubric's policy, which CI runs; and `examples/jud_live.rs`,
  the rubric in `examples/jud/triage.jud` read from the file and asked of
  the hosted API with `TYPESAFE_API_KEY`.

## [0.6.0] - 2026-10-06

### Added

- `docs/project/internals.md`, the rules the code applies and why: the path
  of one call through the modules, the tolerant decoder, what the response
  check leaves unchecked, how an error body is read, what a per-call option
  replaces, the retry loop's edge rules, how recordings are keyed, the
  metric definitions and the `.jud` reader's mechanics, with diagrams.

### Changed

- The comments in `src/` say why and no longer what, once each: dated
  observations, run numbers, server version histories and restatements of
  the code moved to the documentation or went; `src/` is about 1,800 lines
  shorter with no code changed, every public item still documented and
  every intra-doc link resolving.

### Added

- `tools/systemone/serve.py`, the shim that was `examples/laya/serve_laya.py`,
  serves the wire over a choice of open-weight decision models: Laya in
  this process, the request's `model` choosing among several checkpoints,
  and Cloudflare's Clef and Clef-flash on Workers AI, which take the System
  One body at one exact URL per model inside Cloudflare's envelope and so
  cannot be reached with a `base_url` alone. `mise run live:clef` runs the
  live tests through it (`CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_API_TOKEN`
  in `.env`); `OLLAMA_MODEL=clef mise run live:ollama` runs them against
  the same weights on Ollama 0.35.1.

### Changed

- The Laya shim and the benchmark export moved out of `examples/`, which
  now holds only the Rust examples and the recordings and documents they
  read: `tools/systemone/serve.py` and `tools/typed-decisions/export.py`.
  `tools/` is excluded from the package, as `scripts/` is.

## [0.5.1] - 2026-10-04

### Fixed

- A `jud: 1` document could carry a 1.1 field whose value was empty or
  null (`strict: false`, `part_when: {}`, `bands: []`, a case's
  `options: {}` or `options: null`) and be read; a 1.1 field now counts by
  its presence, whatever its value, and `null` is refused as the value of
  one in either version. `bands: []` is refused.
- The JSON Schemas accepted 1.1 features under `jud: 1`, where the reader
  refuses them; they now refuse them too, and a blank band verdict, so the
  schemas and the reader refuse the same documents (`tests/jud.rs`).
- A state path indexed an array with `01` or `+0`; only a canonical decimal
  indexes one now, as in a JSON Pointer and as the specification says.
- A Choice or Score gate with `strict: true` and no bar deferred an answer
  of confidence 0; with no bar nothing is deferred, strict or not.
- `part_when` could leave a Noul's instructions as `{}`, asking the model
  nothing; instructions emptied by `part_when` are `null`, so a Noul with
  no criteria is refused.
- `Rubric::apply` read a question the rubric does not have, or one of
  another primitive, at default gates; it refuses a request this rubric
  did not lower.
- `Rubric::to_yaml` wrote, and `Rubric::lower` lowered, a rubric built in
  code that a reader would refuse (a malformed `when`, `options_from` on a
  Noul); both now check it as a parsed rubric is checked.
- A supplied option with an empty key or a number or boolean description
  was read; it is refused when the cases are parsed, as the schema says.

### Changed

- The 1.1 examples (`examples/jud/routing.jud`, `routing-cases.jud`), the
  specification's examples and the test fixtures use a support inbox
  instead of an operational domain, as the crate's own rule on staying
  generic requires.
- `docs/reference/jud-format.md` states what the review found unstated: instruction parts
  are a JSON object whose key order carries no meaning, an `x-` value is
  JSON-representable, and a fingerprint does not witness the order of a
  Choice's options. `eval::tuning::level_sweep` says where it can read a
  level differently from a gate.

## [0.5.0] - 2026-10-04

### Added

- `jud: 1.1` (decision 0016, issues #8, #9, #10), a minor version of the
  format that only adds: every `jud: 1` document reads as before, keeps its
  fingerprint and is still written as `jud: 1`. A document that uses a 1.1
  feature must say `jud: 1.1`; a writer declares it only when it has to.
  - Requests that depend on the state: a question's `when` (asked only
    when a state path is present), `part_when` (an instruction part sent
    only when one is), and a Choice's `options_from: request` (options
    supplied per request, before the static ones, which may then number
    fewer than two). `Rubric::lower(state, supplied)` builds the request
    through the builder's checks; `jud::present` is the state-path test.
  - A case's `options`, so a case is a complete request; `Case::request`
    lowers it, and binding refuses a label on a question its state does
    not ask.
  - Gates with `bands` (named confidence bars, highest first, generalising
    `confidence`), `level_at_least` on a Score and `strict` (`>` for `≥`);
    `Verdict::Option` and `Verdict::Level` carry the band, and a Score's
    verdict whether it reached the level.
  - Top-level `x-` keys on every kind, ignored and part of no fingerprint,
    kept by `Rubric::to_yaml` and `Cases::to_yaml`: a home for shared YAML
    anchors and tool data.
  - The JSON Schemas state the 1.1 shapes, `examples/jud/routing.jud` and
    `routing-cases.jud` use every feature, and `tests/jud.rs` pins that
    the schemas and the reader refuse the same 1.1 documents.
- `eval::tuning::level_sweep` and `best_level`: a Score's "this level or
  higher" decision swept over the levels, reading the nearest level as a
  `level_at_least` gate does.
- `impl IntoIterator for Questions`, by value, in wire order.

### Changed

- **Breaking:** `jud::Rubric::questions` is an
  `IndexMap<String, jud::RubricQuestion>` (the question with its
  declarations) instead of a `Questions`. Build the request with
  `Rubric::lower` (or `Case::request` for a case) and pass that to a
  backend and to `Rubric::apply`, which now takes the request it reads:
  `apply(&asked, &response)`.
- **Breaking:** `jud::Verdict::Option` gains `band` and `Verdict::Level`
  gains `band` and `reached`; a pattern naming every field needs `..`.
  `jud::Rubric` and `jud::Cases` gain `extensions`, `jud::Case` gains
  `options`, and `jud::Gate` gains `bands`, `level_at_least` and `strict`,
  so a struct literal needs `..Default::default()` (or the new fields).
- `jud::VERSION` is the major version, 1; `jud::MINOR` is the highest minor
  read and written, 1.

### Fixed

- A `.jud` recording with a field the format does not define was read and
  the field ignored; it is refused, as the reading rules always said, with
  only top-level `x-` keys (1.1) ignored.

## [0.4.0] - 2026-10-04

### Added

- The `.jud` format (`docs/reference/jud-format.md`, decision 0014), behind the new `jud`
  feature (off by default): one YAML format, JSON accepted, for a `rubric`
  (the questions in wire shape and wire order, with a `policy` of gates per
  question and the `tuning` they came from), the labelled `cases` a rubric
  is graded on (a conversation as an array state, `{from_turn: n}` for the
  turn a Noul becomes true) and a `recording` of one response. Each has a
  content fingerprint, `sha256:` over RFC 8785 canonical JSON, so a tuned
  threshold names the exact cases and model it rests on and another tool
  computes the same identity. `judgment::jud` reads and writes all three
  (`Rubric`, `Cases`, `Case::per_turn`, `grade`, `Rubric::apply` into
  `Verdict`s), lowering a rubric through the `Questions` builder so a rubric
  that parses is a request that sends; JSON Schemas under `schemas/jud/`;
  `tests/jud.rs` pins that the reader and the schemas refuse the same
  documents; `examples/jud/` and `examples/jud_calibration.rs` run the loop
  from cases to a tuned policy.
- `eval::canonical`: RFC 8785 canonical JSON (`to_string`), the `sha256:`
  fingerprint over it (`fingerprint`) and the request fingerprint over
  `{questions, state}` (`request_fingerprint`), model excluded.
- `eval::tuning`: `threshold_sweep` and `best_threshold` over a Noul's
  judgments, `gate_table` and `lowest_bar` over a Choice's or a Score's, so
  a threshold is read off a table rather than guessed.
- `Recording` carries `fingerprint`, `rubric`, `server` and `recorded_at`
  (all optional, omitted when absent, so older files read unchanged);
  `Recording::new` builds one with only the required fields; `Recorder`
  fills the fingerprint and the time; `eval::now_rfc3339` and
  `eval::rfc3339_from_unix` give the time without a date dependency.
- `Replay::add` for a recording held in memory, and `Replay` finds a
  recording by its `request_hash` or its `fingerprint`; with the `jud`
  feature it reads `*.jud` recordings beside the `*.json` ones.
- `Questions::add` for a question already in wire shape, with the builder's
  checks and no handle, and `Questions::handle` for a typed handle to a
  question by id, `None` when the id or the primitive does not match.
- `Error::InvalidRecording`: a `.jud` file in a recordings directory that is
  not a recording, with its path and the format's reason.

### Changed

- **Breaking:** `Questions` and a Choice's `criteria` keep the order the
  questions and options were added in, which is the order the model sees
  them and the order answers come back in; they were sent alphabetically.
  `Question::Choice::criteria` is an `indexmap::IndexMap<String, Value>`
  rather than a `BTreeMap`, so code that names the type changes; a request
  that relied on alphabetical order on the wire now sends the author's
  order. Recordings are unaffected: the request hash sorts keys.
- **Breaking:** `eval::Recording` has four new fields, so a struct literal
  that named every field no longer compiles; write
  `Recording { case, response, elapsed_ms, ..Recording::new(…) }` or use
  `Recording::new` directly. Files written by earlier releases read
  unchanged.
- `Questions` implements `PartialEq`.

## [0.3.0] - 2026-10-03

### Added

- `contract::OPENAPI_DOCUMENT`, behind the new `openapi` feature (off by
  default): the vendored TypeSafe OpenAPI document as text, for an
  application that validates its own traffic against the contract the crate
  is tested against. A consumer reads it this way instead of by a path into
  the crate's tree.
- One runnable example per TypeSafe pattern, written to the documentation
  page's own scenario and thresholds: `fan_out`, `confidence_routing`,
  `composite_scoring` and `intent_routing`. Each replays `jev-1.13.0`'s
  recorded answers by default, calls the API with `--live` or `--record`,
  and carries a test over its recordings that `cargo test` runs. A
  "Patterns" section in the README and the crate docs says which types
  carry each shape.
- `Error::InvalidRequest::kind`: the server's machine-readable `error_type`
  when a 400 body carries one (`api_usage_error`, `max_tokens_exceeded`,
  observed on the hosted API 2026-10-03), and `Error::is_request_too_large`
  for the refusals whose remedy is a smaller request: that 400 when its code
  is `max_tokens_exceeded`, and a 413, which `laya-serve` answers for a body
  past one of its own limits (0.3.24: a state over 50,000 characters, more
  than 64 questions, 100 options or 32 levels, a body over 2 MiB). A 400
  whose body has an `error_type` and no message now reads that code as its
  detail instead of the raw body.
- `Choice::confidence_from_probabilities`, `Score::expected_value` and
  `Score::confidence_from_probabilities`: the formulas TypeSafe documents,
  computed from the wire's probabilities. On the hosted API they agree with
  the wire's `confidence` and `score` within rounding; against a server that
  defines confidence otherwise (Laya) the difference is now a number a
  caller can log.
- The builder refuses an empty question id (the hosted API refuses it with a
  400) and an empty Choice option key (the hosted API accepts it and can
  choose it).
- Live tests for the server's own limits, the stability of repeated calls
  (the decision holds; probabilities moved by up to 0.05 between identical
  clear-cut requests and by 0.19 on an ambiguous one), the confidence formulas and the token budget
  (`tests/live.rs`); `fixtures/models.json` is the hosted API's list as
  served (two aliases, RFC 3339 release dates), no longer a guess from the
  schema. All fifteen pass against `laya-serve` 0.3.24 as well, with its
  full payload and with `LAYA_JEV_STRICT=1` behind a bearer key, and
  against the shim now at `examples/laya/serve_laya.py` (moved from the
  repository's `examples/`), on `laya` 0.3.24
  (`docs/project/verification/laya-typed-decisions.md`).
- `eval::metrics::wilson_interval` and `QuestionMetrics::accuracy_interval95`:
  a 95% Wilson interval beside every accuracy, because a ratio on three
  labelled cases and one on three hundred read the same without it. A
  consumer that builds `QuestionMetrics` by struct literal gains a field.
- `eval::fingerprint(&Value)`: the canonical FNV-1a hash that keys a
  recording, over any JSON value, so a harness can name the question texts
  or option sets a run was recorded under and refuse to grade old answers
  under new questions.
- `RetryPolicy::max_body_bytes` (default 8 MiB) caps what the shared retry
  loop buffers of a response body: a `Content-Length` over it fails before a
  byte is read, a body without one is read until it passes the cap. The
  failure is `Error::ResponseTooLarge { limit }`, never retried, reported to
  the observer as `too_large`.

### Changed

- The crate lives in its own repository, <https://github.com/chussenot/judgment>,
  moved on 2026-10-03 with its history. A git dependency names this
  repository, with no `package` key.
- The manifest declares its own edition, Rust version, licence, dependency
  versions and lints instead of inheriting a workspace's, and the repository
  carries the files a repository root needs (CI, mise tasks, hooks, agents,
  catalog entry). Nothing changes for a consumer.
- Releases are cut with cocogitto and published by CI: the commits are
  Conventional Commits, `cog bump --auto` derives the version from them,
  stamps this file's Unreleased section and tags, and the pushed `v*` tag
  runs the gate and `cargo publish` (`docs/project/releasing.md`, decision 0013).
  `publish = false` is lifted from the manifest for it; the crate is not on
  crates.io until the first tag is pushed.
- `Response::verify` accepts a structured Score level echoed as any string
  that parses to the level sent, not only as its compact JSON. `laya-serve`
  0.3.22 and later echo the JSON text they showed the model, with Python's
  `", "` and `": "` separators (0.3.20 echoed the value, as the hosted API
  does), and every structured level failed `verify` against them. Spacing
  and key order no longer matter; a number written differently (`1.0` for
  `1`) and a string level echoed as anything but itself still do.
  `Score::levels` labels a structured level with its compact JSON whichever
  way it was echoed; a string level whose own text is a JSON object or
  array is re-spaced the same way, in its label only.
- A 413 is `Error::InvalidRequest { status: 413, .. }`, with the server's
  `detail` and no retry, where it was `Error::Http`; `laya-serve` answers it
  for a body past one of its limits; the hosted API has not been seen to
  send one.
- `Error::InvalidRequest` has a new field, `kind`; a struct literal or a
  pattern that names every field needs `kind` or `..`.
- `http::Exhausted` is an enum, `Transport { attempts, source }` beside the
  new `TooLarge { attempts, limit }`; a caller that destructured the struct
  matches the first variant instead.
- Bodies are decoded as UTF-8 with invalid sequences replaced, no longer by
  the `Content-Type` charset; every upstream answers in JSON.

## [0.2.0] - 2026-09-25

Hardens the wire and closes the gaps against the official TypeSafe SDKs that
the [System One client survey](docs/project/research/system-one-client-libraries.md)
identified. TypeSafe had answered this client live once at this release;
everything else is checked against wiremock, the published OpenAPI
document (0.2.0) and the SDK references.

### Added

- TypeSafe's `x-typesafe-request-id` is carried on `Response::request_id`, on
  every error that came from an HTTP response (`Error::request_id()`, and a
  ` [request_id …]` suffix on the message), and as the `request_id` field of
  the `typesafe.evaluate` and `typesafe.list_models` spans. It is the last
  attempt's id, and optional everywhere, because the OpenAPI document lists
  no response headers. A value that is empty, longer than 256 bytes or not
  printable ASCII (a tab inside it included) is ignored. A body `request_id`
  that is not a string (the documented body has no such field) reads as
  `None` instead of failing the response.
- `http::Completed::headers`: the retry loop hands back the last response's
  headers, so each client reads its own upstream's headers.
- `Error::PermissionDenied` for HTTP 403, and `Error::InvalidApiKey` for a key
  the Python SDK (0.7.1) would refuse.
- `ValidationIssue`: the fields a 400 or 422 body names, with a dotted `path()`.
  It is `#[non_exhaustive]`, so fields such as `ctx` can be added in a minor
  release; its fields are public to read.
- `RetryPolicy::conservative()`: retries only 408, 429 and failures before the
  request left the process, for callers who would rather fail a billed call
  than pay for it twice.
- `RetryPolicy::{http_statuses, transport, budget}` and `TransportRetry`. The
  total retry budget is off by default. A 2xx is never retried, even when
  listed in `http_statuses`, as in both SDKs: a success is not sent again.
- `retry-after-ms` and the HTTP-date form of `Retry-After` are honoured, up to
  `retry_after_max`. A date is measured against the response's `Date` header.
- `Client::evaluate_with(&Request, &CallOptions)`, for a per-call timeout,
  retry policy, headers and extra body fields. Also `ClientBuilder::default_header`,
  `Client::model()` and `Client::retry()`. `x-typesafe-retry-count` is
  reserved (both SDKs own it) although this release does not send it. The
  headers HTTP owns (`content-length`, `transfer-encoding`, `host`,
  `connection`, `te`, `upgrade`) are refused too: the HTTP stack would keep a
  caller's value over its own, truncating or reframing the body, or sending
  the key to another virtual host than the base URL names.
- `Answer::Unknown(Value)` for an answer kind this release does not know,
  logged at `warn` with the answer's key and kind, both escaped and cut to 64
  characters because the server chose them. `Response::extra` keeps
  undocumented top-level fields. (`#[serde(untagged)]` on a variant needs
  serde 1.0.181 or later.)
- `Response::verify(&Questions)`, `Question::kind()` and `Error::is_unfit()`.
  Every backend in the crate (Client, Fake, Replay, Recorder) returns only a
  response that answers the questions it was sent. A structured Score level
  may be echoed as itself or as its compact JSON.
- Contract tests against the vendored `api.typesafe.ai/openapi.json`
  (`tests/contract.rs`), covering requests, Fakes and recordings, plus an
  ignored drift test (`tests/openapi_drift.rs`).

### Changed

- A 400 is `Error::InvalidRequest { status: 400, .. }`, like 422, and its
  `detail` is the server's message or the parsed issues rather than the raw
  body. Echoed request `input` is dropped from parsed issues.
- The API key is trimmed (a trailing newline from a key file is fine) and
  validated when the client is built. A blank explicit key never falls back to
  `TYPESAFE_API_KEY`.
- A Noul with neither instructions nor criteria is refused by the builder;
  criteria that describe neither outcome (`{}`, or both sides null) count as
  none. Null instructions are still sent as `null` (the schema accepts it, and
  Laya requires the key).
- Backoff jitter only shortens a wait, as in both SDKs.
- The client follows no redirect, like the Python SDK and unlike the JS SDK:
  a 3xx is `Error::Http` with its status, not retried. 0.1 followed up to
  ten, re-sending the body (the state included) on a 307 or 308; with 0.2's
  default and per-call headers it would also have sent a gateway credential
  to whatever origin the redirect named, since only `authorization` is
  stripped across origins. A moved API is a base URL to change.
- A missing `usage`, or null counts, read as zero.
- A 2xx that does not decode or does not fit the questions is reported to the
  observer as a failed attempt (`decode` or `unfit`) and is not retried.
- `Error::Http` carries the attempt count and its message names it (`unexpected
  HTTP status 503 after 3 attempts: …`), so a 5xx the policy retried reads
  apart from one it returned at once; an empty or blank body reads `no body`
  in the message, while the `body` field keeps what the server sent.
- Server-chosen strings in error messages are escaped and cut to 64
  characters, as an unknown answer kind is: the off-list option in
  `Error::UnknownOption`'s message (the `option` field keeps it whole) and a
  probability key in a Score's `Error::InvalidAnswer` reason. An answer sent
  as a JSON string is named by its type in the `Error::Decode` message, not
  quoted.
- `Fake::score` echoes the question's levels as its legend. A Fake refuses a
  scripted answer that does not fit its question.
- The rustdoc states where retries match the SDKs and where they deliberately
  differ, instead of claiming to mirror them. The `/v1/models` notes and the
  question-limit notes are corrected against the OpenAPI document.

### Fixed

- A `Retry-After` of `inf`, or a value of `1e20` or more, panicked in the retry
  loop. Such values are now ignored.
- A malformed API key is no longer reported as a URL error.

### Breaking changes

- `Error` is `#[non_exhaustive]` (S1).
- `Error::Unauthorized` is a struct variant with `request_id` (S1).
- `Error::RateLimited`, `Overloaded` and `Http` gain `request_id` (S1).
- `Error::Http` gains `attempts`, and its message names the attempt count and
  reads `no body` for an empty body (S1).
- `Error::Decode` is a struct variant `{ source, request_id }` with a
  `From<serde_json::Error>` impl (S1).
- `http::Completed` is `#[non_exhaustive]` with a new `headers` field (S1).
- `Response` gains the public field `request_id` (S1).
- `Error::InvalidRequest` gains `status`, `issues` and `request_id`, covers
  400, and its `detail` is a summary, not the raw body (S1, S2).
- A 403 is `Error::PermissionDenied`, not `Error::Http` (S2).
- A 400 is `Error::InvalidRequest { status: 400, .. }`, not `Error::Http` (S2).
- New `Error::InvalidApiKey`: a key with inner whitespace, a control character
  or a non-ASCII character (a BOM included) is refused at `build()` (S2).
- A non-UTF-8 `TYPESAFE_API_KEY` is `InvalidApiKey`, not `MissingApiKey` (S2).
- The key is trimmed; a blank explicit key is `MissingApiKey` and never falls
  back to the environment; the `MissingApiKey` message is reworded (S2).
- `Questions::noul` refuses a Noul with neither instructions nor criteria;
  criteria that describe neither outcome count as none (S2).
- `RetryPolicy` gains public fields, so struct literals need
  `..RetryPolicy::default()` (S3).
- `RetryPolicy::is_retryable` takes `&self` (S3).
- Backoff jitter only shortens a wait, for every client of the shared loop (S3).
- `retry-after-ms` and HTTP-date `Retry-After` are honoured up to
  `retry_after_max` on any retried status, for every client of the shared
  loop (S3).
- New variants `Error::ReservedHeader` and `Error::ReservedField` (S4).
- A 3xx is `Error::Http` instead of being followed (S4).
- `Answer` gains `Unknown`, is `#[non_exhaustive]`, and its hand-written
  `Deserialize` accepts unknown kinds (S5).
- `Answer::kind` returns `&str` and is no longer `const` (S5).
- `Error::AnswerTypeMismatch.actual` is a `String` (S5).
- `Response` gains the public field `extra` and no longer decodes from a JSON
  array (S5).
- A response with no `usage`, or null counts, decodes with zero instead of
  failing (S5).
- `Error::MissingAnswer` is a struct variant `{ id, request_id }` (S6).
- `Error::UnknownOption`, `InvalidAnswer` and `AnswerTypeMismatch` gain
  `request_id`; `UnknownOption`'s message is reworded, and quotes the option
  escaped and cut to 64 characters (S6).
- The client refuses a 2xx that does not fit the questions sent, without
  retrying (S6).
- `Observer::on_failed_attempt` also receives `decode` and `unfit` from the
  TypeSafe client (S6).
- `Fake` refuses a scripted answer that does not fit and records no call;
  `Fake::score`'s legend is the question's levels (S6).
- `Replay` refuses a recording that does not fit; `Recorder` writes nothing
  for such a response (S6).
- Version 0.1.0 becomes 0.2.0 (S8).
