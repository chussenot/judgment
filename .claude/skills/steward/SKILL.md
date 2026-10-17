---
name: steward
description: This repository's conventions for driving an open pull request to green after it is opened, merging main in, regenerating generated files, the beads export, and a pull request that gets no checks. Read before acting on a CI, review or merge-conflict event on a pull request here.
---

# Driving a pull request here

`pr-shepherd` reads a red check and tells a failure the change caused from
one it did not. This skill holds what it does not: how this repository
brings `main` in and what to look at afterwards.

## No checks after a push means a conflict

GitHub runs the pull-request workflows on the merge of the branch into its
base. When that merge conflicts, no workflow runs and the check list is
empty, which reads like "still queued". Fetch the pull request: a
`mergeable_state` of `dirty` is the answer, and the conflict is the work.

## Bringing `main` in

- A merge commit, never a rebase or a force push: `git fetch origin main`,
  `git merge --no-ff origin/main`, the message git proposes
  (`Merge remote-tracking branch 'origin/main' into <branch>`; `cog`
  ignores merge commits). Every branch here has done it this way.
- A conflict in a generated file is resolved by taking either side and
  running its generator: `docs/llms*.txt` (`scripts/gen-llms-txt.sh`),
  `docs/reference/cli.md` (`scripts/gen-cli-reference.sh`), the plugin's
  `references/format.md` and Rust templates (`scripts/gen-plugin-*.sh`).
- A conflict on a page is resolved from `main`'s text with this branch's
  change applied on top: `main` may have rewritten the paragraph since.

## The beads export after a merge

`.beads/issues.jsonl` merges as a line union (`.gitattributes`), and the
pre-commit hook imports the file before beads' own flush exports the
database over it, so the merge commit carries both sides' issues. Check it
before pushing, because a regression here deletes other people's work:

```sh
git diff origin/main HEAD -- .beads/issues.jsonl | grep '^-{'
```

Every line it prints is an issue `main` has and this commit lost or took
back to an older state. None is the expected result. If there are some,
`git show origin/main:.beads/issues.jsonl > /tmp/main.jsonl`, then
`bd import /tmp/main.jsonl` (newer rows win) and `scripts/bd-sync.sh
export`, and commit the export on its own.

## Several Dependabot pull requests merged

Each one's CI ran on its own merge into the `main` it saw, so the set
together was never tested; two majors can each pass and fail combined.
After a batch merges, run CI's gates on `main` itself, the local ones and
the four `mise run check` leaves out:

```sh
git fetch origin main && git checkout --detach origin/main
mise run check && mise run deny msrv hack semver
```

A failure there is a pull request of its own against `main`, not a fix
pushed into whichever branch is open. Read the changelog of any major
(`~/.cargo/registry/src/*/<crate>-<version>/CHANGELOG.md`) against what
the crate uses: a breaking change in an API the crate never calls is
noise, one in an API it does call needs a test that exercises it.

## After the merge

Close the bead the pull request implemented, with its number:
`bd close <id> --reason "merged as #N"`. The export travels in the next
commit on any branch; the hooks stage it.
