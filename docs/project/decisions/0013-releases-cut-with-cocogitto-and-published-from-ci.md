---
title: 0013 Releases cut with cocogitto and published from CI
description: The version is derived from Conventional Commits by cocogitto, the CHANGELOG stays hand-written, and a pushed tag runs the gate and publishes the crate to crates.io, so no release depends on one person's laptop or memory.
status: accepted
date: 2026-10-03
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-03
tags: [decisions, release, versioning, ci, crates-io]
---

# 0013 Releases cut with cocogitto and published from CI

## Context and problem statement

The crate's one release so far, 0.2.0, was cut by hand: the version edited in the manifest, the CHANGELOG dated, a commit named "release 0.2.0", no tag, and `publish = false` in the manifest so nothing could reach crates.io by accident. In its own repository the crate is meant to be published, and a release has to be the same every time: the version must follow from what changed, the CHANGELOG must say what the version contains, the gate must have passed on exactly what is published, and the publish must not depend on a token on someone's machine. How should a version be chosen, recorded and published?

## Decision drivers

- The version must follow the crate's own rule (0.x: a minor bump may break) from the changes, not from memory
- The CHANGELOG is hand-written in Keep a Changelog form and explains changes to a consumer; a generated list of commit subjects would not
- CI runs under a repository policy that allows only GitHub-owned actions
- `main` takes changes only through pull requests, so a release cannot be a direct push
- A published version cannot be taken back, so publishing must be a deliberate step with the gate in front of it
- One tool for the convention, the version and the tag, with as little custom script as possible

## Considered options

1. cocogitto for the convention and the bump, the CHANGELOG hand-written, publication from a tag in CI
2. release-plz or cargo-release: a tool that writes the CHANGELOG from the commits and opens the release pull request or publishes directly
3. Releases by hand, as 0.2.0 was, with a written checklist

## Decision outcome

Chosen option: "cocogitto, hand-written CHANGELOG, publication from a tag in CI".

- Commits are [Conventional Commits](https://www.conventionalcommits.org). `cog verify` runs on every commit message through the prek `commit-msg` hook, and CI checks a pull request's commits with `cog check`, merge commits ignored.
- A release is `cog bump --auto` on an up-to-date `main`. cocogitto derives the next version from the commits since the last tag, under its pre-1.0 rule: a breaking change or a feature bumps the minor, a fix the patch. Its pre-bump hook, `scripts/release-bump.sh`, writes the version into `Cargo.toml` and `Cargo.lock` and turns the CHANGELOG's Unreleased section into the release's section with the date, refusing an empty one; the full gate runs; cocogitto commits and tags. cocogitto's own changelog generation is off (`disable_changelog`).
- The bump commit reaches `main` through a pull request like any other change, merged with a merge commit so the tagged commit is the one on `main`; the tag is pushed after the merge.
- The pushed tag runs `.github/workflows/release.yml`: the whole CI gate again, a check that the tag names the manifest's version, `cargo publish --dry-run`, then `cargo publish` and a GitHub release whose notes are the CHANGELOG's section for that version. The crates.io credential is a trusted-publishing token exchanged from the job's OIDC identity, or the `CARGO_REGISTRY_TOKEN` secret for the first release, before the crate exists on crates.io and a trusted publisher can be configured. The job runs in the `crates-io` environment so required reviewers can be put in front of it.
- `publish = false` is lifted from the manifest. The Bash guard denies `cargo publish` and pushing a tag to agents working in the repository: both are the release owner's decision.

### Consequences

- Good, because the version is computed from the commits and the same computation runs for every release; "which bump is this" stops being a judgment call at release time
- Good, because the CHANGELOG keeps explaining changes to a consumer, which no commit log does, and the bump only dates it
- Good, because what is published is what passed the gate on the tagged commit, and the credential lives in CI, short-lived where trusted publishing is configured
- Good, because the whole flow uses one third-party tool, installed from crates.io, and no third-party action
- Bad, because the commit convention is one more thing every contributor and agent must follow; the hook and the CI check make a slip a failed check rather than a wrong version
- Bad, because a release is two pushes (the pull request, then the tag) rather than one command; that is the cost of a protected `main`
- Bad, because the history before this repository was not written to the convention, so cocogitto's scan starts at a baseline tag, `v0.2.0`, placed on the commit that released 0.2.0

### Confirmation

`cog check --from-latest-tag` passes on `main`; `cog bump --auto --dry-run` names the next version; the `commits` job in `ci.yml` fails a pull request with a non-conforming commit; `release.yml` publishes only after `ci.yml` passed on the tag, and the version check refuses a tag whose number is not the manifest's.

## Pros and cons of the options

### cocogitto, hand-written CHANGELOG, tag-driven publication

- Good, because cocogitto does the convention, the version and the tag, and nothing else; what it does not do (the CHANGELOG stamp) is one short script
- Good, because the release is reviewable: the bump commit is a pull request
- Bad, because the baseline tag and the branch whitelist are configuration a reader must know about (`cog.toml` says why)

### release-plz or cargo-release

- Good, because release-plz opens the release pull request itself and publishes on merge, with no tag step
- Bad, because both write the CHANGELOG from the commits, replacing a consumer-facing document with commit subjects, or need the hand-written one maintained beside theirs
- Bad, because release-plz is run as a third-party action and needs a long-lived crates.io token in the repository; cargo-release publishes from the machine it runs on

### Releases by hand with a checklist

- Good, because nothing new to install or learn
- Bad, because every step the checklist describes is one a tool can do the same way every time, and 0.2.0 shows the result: no tag, a non-conventional commit, a token that would have had to be local

## More information

[Releasing](../releasing.md) is the runbook; `cog.toml`, `scripts/release-bump.sh`, `.github/workflows/release.yml` and the `commits` job of `.github/workflows/ci.yml` implement the decision; [cocogitto's documentation](https://docs.cocogitto.io) describes the bump rules and hooks.
