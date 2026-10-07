---
title: The container image
description: What is inside ghcr.io/chussenot/jud, the user it runs as, the paths it reads, the tags a release pushes, and how its provenance is verified.
status: current
last_reviewed: 2026-10-07
tags: [judgment, jud, container, docker, ghcr, reference]
---

# The container image

`ghcr.io/chussenot/jud` is the `jud` binary as a container image. [Run in a container](../guides/run-in-a-container.md) is the procedure; this page is what the image is. Every fact here was checked against `ghcr.io/chussenot/jud:0.10.4` on 2026-10-07.

## Contents

The image is `FROM scratch`. It holds the statically linked `jud` binary, a CA bundle, and a `passwd` and `group` declaring one user; no shell, no libc, no package manager. The binary is the release tarball's, copied in byte for byte by the release workflow, so the image, the tarball, the Homebrew formula and `cargo binstall` carry the same `jud` ([Releasing](../project/releasing.md)).

| | |
|---|---|
| Binary | `/usr/local/bin/jud`, the entrypoint; the arguments are `jud`'s |
| CA bundle | `/etc/ssl/certs/ca-certificates.crt`, named by `SSL_CERT_FILE`, so the client can verify a TLS backend |
| User | `65532:65532`, the uid distroless images call `nonroot`; never root |
| Home | `HOME=/home/jud`, so the configuration file is `/home/jud/.config/jud/config.yaml` ([Configuration](configuration.md)) |
| Working directory | `/work`; a mount there is where relative paths resolve |
| Platforms | `linux/amd64` and `linux/arm64`, one manifest list |
| Size | about 10 MB |
| Labels | `org.opencontainers.image.version`, `org.opencontainers.image.revision` (the commit), `org.opencontainers.image.source` (the repository) |

The binary is the one `start/install.md` describes for Linux; its subcommands, flags and exit status are [The jud command line](cli.md). Nothing in the image writes a file.

## Tags

Every release pushes three tags: the version (`0.10.4`), its major.minor (`0.10`), and `latest`. A digest (`ghcr.io/chussenot/jud@sha256:…`) names one build for good; `docker image inspect <tag> --format '{{index .RepoDigests 0}}'` prints it.

## Provenance

The image carries a build-provenance attestation signed by GitHub for the workflow run that built it:

```sh
gh attestation verify oci://ghcr.io/chussenot/jud:0.10.4 --repo chussenot/judgment
```

exits 0 when the image was produced by this repository's release workflow and 1 otherwise. The tarballs carry the same attestation ([Install](../start/install.md#from-a-release-tarball)).

## Build

The image is built by the release workflow from the release tarballs (`Dockerfile`, `scripts/image-context.sh`), never by `docker build` from source. A local build for a change to the `Dockerfile` is described in [Contributing](../project/contributing.md#the-container-image).
