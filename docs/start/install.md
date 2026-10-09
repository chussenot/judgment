---
title: Install
description: Every way to get the jud command and the judgment crate: mise, Homebrew, a release tarball with its checksum and attestation, cargo-binstall, cargo install, the container image, the crate as a dependency with its features, and shell completion.
status: current
last_reviewed: 2026-10-07
tags: [judgment, jud, install, mise, homebrew, cargo, container, getting-started]
---

# Install

**You will** have the `jud` command on your `PATH`, or the crate in your `Cargo.toml`, by the route that fits your machine. **Prerequisites:** a Unix shell; a Rust toolchain only for the cargo routes.

A release publishes one tarball per platform, so every route below installs the same binary, built once on a native runner and run before it was packaged ([Releasing](../project/releasing.md)).

## The `jud` command

### With mise

```sh
mise use -g github:chussenot/judgment@latest
jud --version
```

Pin a version with `@0.10.4`; `mise ls-remote github:chussenot/judgment` lists what exists. The tool is named after the repository and the executable is `jud`; mise finds it inside the tarball.

### With Homebrew

On Apple silicon or Linux:

```sh
brew install chussenot/tap/jud
```

The formula, in [chussenot/homebrew-tap](https://github.com/chussenot/homebrew-tap), installs the release tarball by its checksum and generates the shell completions from the binary; nothing is compiled. The fully qualified name trusts that one formula and nothing else, which is what Homebrew 6.0 and later ask for a third-party tap. Intel macOS has no tarball, and the formula installs from crates.io instead.

### From a release tarball

With the checksum verified first. The tag below is an example; the [releases page](https://github.com/chussenot/judgment/releases) lists them.

```sh
TAG=v0.10.4
TARGET=x86_64-unknown-linux-musl
BASE="https://github.com/chussenot/judgment/releases/download/$TAG"
curl -fsSLO "$BASE/jud-$TAG-$TARGET.tar.gz"
curl -fsSLO "$BASE/SHA256SUMS"
sha256sum --ignore-missing -c SHA256SUMS
tar -xzf "jud-$TAG-$TARGET.tar.gz"
install -m755 "jud-$TAG-$TARGET/jud" /usr/local/bin/
```

Each tarball carries a build-provenance attestation. With the GitHub CLI, `gh attestation verify jud-$TAG-$TARGET.tar.gz --repo chussenot/judgment` says which workflow, commit and run produced it.

| Target | Runs on |
|---|---|
| `x86_64-unknown-linux-musl` | any x86-64 Linux, statically linked |
| `aarch64-unknown-linux-musl` | any arm64 Linux, statically linked |
| `aarch64-apple-darwin` | Apple-silicon macOS |

Intel macOS and Windows have no build: the first installs from crates.io, the second is not supported (the configuration directory and the pipeline model assume a Unix shell).

### With cargo

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), the release tarball for the machine is downloaded instead of compiled:

```sh
cargo binstall judgment
```

From source, on any platform with a Rust toolchain (1.91 or later):

```sh
cargo install judgment --features cli
```

In a checkout, `mise run install` does the same and refreshes the completion scripts a shell already has.

### As a container

```sh
docker pull ghcr.io/chussenot/jud:0.10.4
docker run --rm ghcr.io/chussenot/jud:0.10.4 --version
```

Tags are the version, its major.minor and `latest`; the image runs as a non-root user with no shell ([The container image](../reference/container-image.md)). [Run in a container](../guides/run-in-a-container.md) says how files and a backend reach it.

### Shell completion

`jud completion <shell>` prints a completion script for `bash`, `zsh`, `fish`, `elvish` or `powershell`, generated from the same command tree the binary parses, so nothing is checked in to go stale:

```sh
jud completion bash > ~/.local/share/bash-completion/completions/jud   # or /etc/bash_completion.d/jud
jud completion zsh  > "${fpath[1]}/_jud"
jud completion fish > ~/.config/fish/completions/jud.fish
```

Regenerate the file after upgrading: a script written by an older binary completes that binary's commands. The Homebrew formula installs them for you.

## The crate

```toml
[dependencies]
judgment = "0.12"
```

Pin the minor: the crate is 0.x and a minor release may break ([Stability](../reference/stability.md)). The `http` feature, on by default, is the client; `jud` adds the `.jud` format; `openapi` the vendored OpenAPI document; `cli` the binary. [The crate](../reference/crate.md#features) says what each pulls in.

```toml
judgment = { version = "0.12", features = ["jud"] }
```

## Next

- [Your first decision from the command line](first-decision-cli.md), with no account.
- [Your first decision in Rust](first-decision-rust.md).
