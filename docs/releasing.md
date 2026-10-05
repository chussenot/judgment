---
title: Releasing
description: How a version of the judgment crate is cut from its Conventional Commits with cocogitto, how the pushed tag publishes it to crates.io from CI, what to set up once, and what to do when a release goes wrong.
status: current
last_reviewed: 2026-10-05
tags: [judgment, release, versioning, cocogitto, crates-io, ci]
---

# Releasing

A release has to be the same every time: the version follows from what changed, the CHANGELOG says what the version contains, the gate has passed on exactly what is published, and the credential that publishes lives in CI rather than on a laptop. [Decision 0013](decisions/0013-releases-cut-with-cocogitto-and-published-from-ci.md) says why the pieces below were chosen over release-plz, cargo-release and a checklist. This page says how to use them.

## The pieces

| Piece | Where | What it does |
|---|---|---|
| Conventional Commits | every commit; `cog verify` in the prek `commit-msg` hook, `cog check` in the `commits` job of CI | Gives cocogitto the facts it derives a version from, and refuses a commit message it cannot read |
| `cog bump --auto` | `cog.toml`, run as `mise run release` | Computes the next version from the commits since the last tag, runs the pre-bump hooks, commits `chore(version): vX.Y.Z` and tags it |
| `scripts/release-bump.sh` | the first pre-bump hook | Writes the version into `Cargo.toml` and `Cargo.lock` and its major.minor into the README's install snippet, turns the CHANGELOG's Unreleased section into the release's dated section, and refuses an empty one |
| The gate | the remaining pre-bump hooks, then CI on the pull request and on the tag | Formatting, clippy, the no-`http` build, the tests, rustdoc, the documentation checks and what the package would ship |
| `.github/workflows/release.yml` | on a pushed `v*` tag | Runs the CI gate on the tagged commit, checks the tag against `Cargo.toml`, runs `cargo publish --dry-run`, publishes, and creates a GitHub release with the CHANGELOG section as notes |

The CHANGELOG stays hand-written, in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) form: a pull request that changes what a consumer sees adds its entry under Unreleased, and the bump only dates that section. cocogitto's own changelog generation is off, because a list of commit subjects does not explain a change to a consumer.

## What the commits decide

cocogitto reads the commits since the latest tag. Below 1.0, as the crate is:

| Commits since the last tag | Next version |
|---|---|
| Any `feat`, or any commit with `!` after its type or a `BREAKING CHANGE` footer | minor: `0.2.0` to `0.3.0` |
| Only `fix` (and types that bump nothing) | patch: `0.2.0` to `0.2.1` |
| Only `docs`, `chore`, `ci`, `test`, `refactor`, `style`, `build`, `perf` | no release; `cog bump` says so and does nothing |

A breaking change in a 0.x crate is a minor bump, which is the rule the CHANGELOG states; from 1.0 on, cocogitto makes it a major. Merge commits are ignored. `mise run release:dry` prints the version the commits call for without touching anything.

## Cutting a release

1. Be on an up-to-date `main` with a clean tree. Check that the CHANGELOG's Unreleased section says what the release contains; a release with an empty section is refused.
2. `mise run release:dry` to see the version. If it is not the one the changes warrant, the commits say something the changes do not; fix that first (an amended message on an unmerged branch, or a further commit), never the version by hand.
3. `git switch -c release/vX.Y.Z` with that version, then `mise run release`. cocogitto runs the pre-bump hooks (the version into the three files, then the gate), commits `chore(version): vX.Y.Z` and tags `vX.Y.Z` on that commit. A failed hook leaves the tree edited and nothing committed; `git checkout .` restores it.
4. Push the branch and open a pull request. CI runs the gate and the commit check on it like any other change.
5. Merge it with a merge commit. Never squash or rebase this pull request: the tag points at the bump commit, and `main` has to carry that very commit for the tag to be a commit on `main`.
6. `git push origin vX.Y.Z`. The release workflow runs: the gate once more, the version check, the dry run, the publish, the GitHub release. The crate is on crates.io when the `publish` job is green.

Agents working in the repository cannot do steps 3 and 6 by accident: the Bash guard denies `cargo publish` (the dry run is allowed) and pushing a tag.

## One-time setup

- **The baseline tag.** cocogitto scans from the latest tag because the history before this repository was not written to the convention. Tag the commit that released 0.2.0 and push the tag; its tree has no release workflow, so nothing runs:

  ```sh
  git tag -a v0.2.0 9ccd761 -m "judgment 0.2.0"
  git push origin v0.2.0
  ```

- **The first publish** (done: 0.3.0, 2026-10-03). crates.io lets a trusted publisher be configured only on a crate that already exists, so the first release used an API token with the `publish-new` and `publish-update` scopes, stored as the `CARGO_REGISTRY_TOKEN` secret of the `crates-io` environment. crates.io also requires a verified email address on the publishing account; without one the upload is refused with a 400 and the job is re-run once the address is verified.
- **Trusted publishing, now that the crate exists.** On the crate's settings page on crates.io, add a trusted publisher: GitHub, owner `chussenot`, repository `judgment`, workflow `release.yml`, environment `crates-io`. Then delete the `CARGO_REGISTRY_TOKEN` secret: the workflow exchanges its OIDC identity for a token that lives for the job and is revoked at its end. That path has not run yet; watch the first release that uses it.
- **The environment.** Put required reviewers on the `crates-io` environment to have a person approve each publish, and restrict it to tags matching `v*`.
- **Merge method.** Disable squash merging and rebase merging in the repository settings, or take care on the release pull request (step 5 above).
- **Locally.** `mise install` builds `cog` from crates.io; `mise run setup` installs the hooks, the `commit-msg` one included; `mise run commits:check` is what CI runs on a pull request.

## When something goes wrong

- **The commit check fails on a pull request.** Reword the commit on the branch and force-push the branch; the message, not the code, is what failed.
- **`cog bump` refuses to run.** It wants a clean tree on `main` or a `release/*` branch and a non-empty Unreleased section; its message says which.
- **The release workflow fails before `cargo publish`.** Nothing was published, and the version is still the one `main` carries, so there is nothing to bump again (the bump script refuses a manifest that is already past the latest tag). Fix the cause on `main` through a pull request, move the tag to the fixed head and push it again: `git tag -f vX.Y.Z origin/main && git push --force origin vX.Y.Z`. The workflow's checks hold on any commit that carries the version. Never move a tag once the publish step has run: the version is on crates.io and the tag must keep naming what was published.
- **`cargo publish` succeeded and a later step failed.** The version is on crates.io and cannot be re-published. Re-run the failed job for the GitHub release; do not retag.
- **A published version is wrong.** `cargo yank --version X.Y.Z` by hand, from a machine with a token, hides it from new resolutions; a yank is not a deletion and is recorded in the CHANGELOG under the version. The fix is the next release.
