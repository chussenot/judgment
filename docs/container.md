---
title: The jud container image
description: How to run the jud binary as the ghcr.io/chussenot/jud container image, with nothing installed; what is inside it and why it is built that way, how to pick a tag or a digest and verify its provenance, how a rubric and an event reach it through the working directory, the four ways a backend is reached from inside (a key, another server, the host's Ollama, a replay directory), what a non-root image without a shell changes, and how to use it in a CI job or build it locally; every command checked against 0.10.4 on 2026-10-07.
status: current
last_reviewed: 2026-10-07
tags: [judgment, jud, cli, container, docker, ghcr, ci]
---

# The jud container image

[The jud command line](cli.md) evaluates a JSON event against a `.jud` rubric and prints the verdicts. `ghcr.io/chussenot/jud` is that binary as a container image, for a pipeline, a CI job or a machine where installing a tool is the thing to avoid. This page is the walk-through: what the image is, how a file and a backend reach it, and what running as a locked-down container changes. Every command on it was run against `ghcr.io/chussenot/jud:0.10.4` on 2026-10-07 with Docker 27.

```sh
cat event.json | docker run -i --rm -v "$PWD:/work" ghcr.io/chussenot/jud triage.jud
```

That line is the whole model: `-i` carries stdin into the container, `-v "$PWD:/work"` puts the current directory where the binary looks for files, and everything after the image name is `jud`'s own command line, as if the binary were installed.

## What is inside

The image is `FROM scratch`: the statically linked `jud` binary at `/usr/local/bin/jud`, a CA bundle at `/etc/ssl/certs/ca-certificates.crt` so the client can verify a TLS backend, and a `passwd` and `group` declaring one user. Nothing else: no shell, no libc, no package manager, 10 MB. The binary is not compiled for the image; it is the one the release workflow built on a native runner, ran, checked and packaged into the release tarball, copied in byte for byte, so the image, the tarball, the Homebrew formula and `cargo binstall` all carry the same `jud` ([Releasing](project/releasing.md)). The `Dockerfile` in the repository says why each line is there.

| | |
|---|---|
| Platforms | `linux/amd64` and `linux/arm64`, one manifest list; Docker picks the machine's |
| User | `65532:65532`, the uid distroless images call `nonroot`; never root |
| Working directory | `/work`, where a mount puts the files a command names |
| Entrypoint | `/usr/local/bin/jud`; the arguments are `jud`'s |
| Environment | `HOME=/home/jud`, so the configuration file is `/home/jud/.config/jud/config.yaml`; `SSL_CERT_FILE` names the CA bundle |
| Labels | `org.opencontainers.image.version`, `revision` (the commit) and `source` (the repository) |

`docker run --rm ghcr.io/chussenot/jud --version` prints the version, and `docker run --rm ghcr.io/chussenot/jud config` prints what a run would use with nothing mounted and nothing passed: `config_file_present: false`, the hosted API, `jev-latest`, a 30 s timeout and `"api_key": "missing"`, the one line that says why a run would be refused.

## Tags, digests and provenance

Every release pushes three tags: the version (`0.10.4`), its major.minor (`0.10`), and `latest`. In a pipeline, pin the version, or better the digest, which no later push can move:

```sh
docker pull ghcr.io/chussenot/jud:0.10.4
docker image inspect ghcr.io/chussenot/jud:0.10.4 --format '{{index .RepoDigests 0}}'
# ghcr.io/chussenot/jud@sha256:1376d5f9...
docker run --rm ghcr.io/chussenot/jud@sha256:1376d5f90dbe9ac769c73bc6096ef9f26de08a5c089ddea0b02e6b911e0a1ba8 --version
```

The image carries a build-provenance attestation signed by GitHub for the workflow run that built it. `gh attestation verify oci://ghcr.io/chussenot/jud:0.10.4 --repo chussenot/judgment` exits 0 when the image was produced by this repository's release workflow and 1 otherwise; it needs the `gh` CLI and a network, nothing else. The tarballs carry the same attestation, so the binary inside can be traced to a commit either way.

## Files reach it through `/work`

The container sees only what is mounted. The working directory is `/work`, so a mount there makes a rubric, a state file or a recordings directory reachable by a relative path, exactly as it would be in a shell opened in that directory. Mount read-only unless `jud` has to write, which it never does: no subcommand writes a file.

```sh
# Read documents as the crate reads them; status 2 if any is refused.
docker run --rm -v "$PWD/examples/jud:/work:ro" ghcr.io/chussenot/jud check triage.jud triage-cases.jud

# What a rubric asks for a state, without any backend.
docker run --rm -v "$PWD/examples/jud:/work:ro" ghcr.io/chussenot/jud lower triage.jud --state '{"message": "hi"}'
```

Two things about the user. The binary runs as uid 65532, so a mounted file must be readable by that user: a file with mode 600 owned by you is refused with `Permission denied (os error 13)`, counted as a refused document by `check` and a usage error by a run. World-readable files (644, the usual) need nothing. And there is no shell inside, so `docker exec` has nothing to run and `--entrypoint sh` finds no `sh`; a look around the image is `docker image inspect`, and a file is read by mounting it where `jud` reads it.

Stdin is a file too. A run takes the state on stdin, and `docker run` passes stdin only with `-i`; without it the container reads an empty stream and refuses, `stdin is empty: pipe the JSON state in`, status 2. `-t` is never needed and would mangle the JSON on stdout.

## Reaching a backend

A run needs a model behind the wire. Inside the container the choice is made the way it is outside ([Configuration](cli.md#configuration)): the environment, then the configuration file, then the defaults. Four shapes cover what a pipeline does.

**The hosted API, with a key.** Pass the key as an environment variable, never baked into an image or written into a mounted file in a CI job:

```sh
cat event.json | docker run -i --rm -v "$PWD:/work:ro" -e TYPESAFE_API_KEY ghcr.io/chussenot/jud triage.jud
```

`-e TYPESAFE_API_KEY` with no value copies the variable from the shell that runs `docker`, so the key appears on no command line. The CA bundle in the image is what makes the TLS handshake to `api.typesafe.ai` succeed; an image without one fails every HTTPS backend, which is why it is there.

**Another server, by URL.** `-e TYPESAFE_BASE_URL=https://...` points the client at any server that speaks the wire, with `-e TYPESAFE_API_KEY` whatever that server wants (any word, for one that ignores the bearer).

**The host's Ollama.** A model running on the machine that runs Docker is the case [Open-weight models without an account](open-weights.md) sets up, and the one subtlety is the network. Ollama listens on `127.0.0.1:11434` by default, which inside a container is the container's own loopback, not the host's. `--network host` gives the container the host's network namespace, so the host's loopback is reachable at the same address, and the configuration file for a local model, mounted at `/home/jud/.config/jud/config.yaml`, does the rest:

```sh
echo '{"message": "My payouts have been failing for 3 days."}' \
  | docker run -i --rm --network host \
      -v "$PWD/examples/jud:/work:ro" \
      -v "$PWD/examples/jud/config-tev1.yaml:/home/jud/.config/jud/config.yaml:ro" \
      ghcr.io/chussenot/jud triage.jud
```

That returned `tev1:0.8b`'s verdicts on 2026-10-07, the same ones the page above shows. The alternative, `--add-host=host.docker.internal:host-gateway` with `TYPESAFE_BASE_URL=http://host.docker.internal:11434`, reaches the host's addresses but not its loopback: against an Ollama bound to `127.0.0.1` it fails with `transport error after 3 attempts`, and works only once Ollama listens on every interface (`OLLAMA_HOST=0.0.0.0`), which exposes the server to the network. `--network host` is the one that needs no change on the host; it is a Linux feature, and on Docker Desktop (macOS, Windows) it is a setting to enable.

**No server at all.** `--replay DIR`, or `-e JUD_REPLAY=DIR`, answers from recordings under a mounted directory, with no key and no network ([Without a server](cli.md#without-a-server)). A state nobody recorded is a backend failure, never a guess; that is what makes it a test:

```sh
cat event.json | docker run -i --rm -v "$PWD/examples:/work:ro" \
  -e JUD_REPLAY=/work/recordings/jud_calibration ghcr.io/chussenot/jud jud/triage.jud
```

The directory is given as the container sees it, under `/work`, with `JUD_REPLAY`; with the flag the path is relative to `/work` like any other: `--replay recordings/jud_calibration jud/triage.jud`.

## In a CI job

The image needs no install step, no toolchain and no cache, which is the point. A job that checks every `.jud` document in a repository and evaluates one event against a replay directory, so that it needs no key:

```yaml
# GitHub Actions
- name: Check the rubrics and replay the smoke case
  run: |
    docker run --rm -v "$PWD:/work:ro" ghcr.io/chussenot/jud:0.10 check rubrics/*.jud
    jq -c . tests/smoke-event.json \
      | docker run -i --rm -v "$PWD:/work:ro" -e JUD_REPLAY=/work/tests/recordings ghcr.io/chussenot/jud:0.10 rubrics/triage.jud \
      | jq -e '.desk.key == "billing"'
```

A CI system that runs a job *inside* an image (GitLab's Docker executor, a Kubernetes job, a Tekton step) runs the job's script through a shell in that image, and this image has none, so `image: ghcr.io/chussenot/jud` as a job image fails before the first line of the script. The pattern there is a job image of your own that takes the binary from this one, a single `COPY --from`, which keeps the provenance (the bytes are the release's) and adds whatever shell and tools the job needs:

```dockerfile
FROM alpine:3.21
COPY --from=ghcr.io/chussenot/jud:0.10 /usr/local/bin/jud /usr/local/bin/jud
```

`jud` is statically linked, so it runs on any Linux base, musl or glibc, with no library to add. A key in CI is a secret passed with `-e TYPESAFE_API_KEY` from the job's masked variables; pin the tag to a version or a digest, and verify the attestation in the job when the pipeline's policy asks for it.

The exit status is the binary's ([Exit status](cli.md#exit-status)): 0 with the verdicts on stdout, 1 when the backend failed or its answer did not fit, 2 for anything fixable before a call. `docker run` returns it unchanged, so a job fails for the right reason.

## Building it locally

The image is never built from source by `docker build`: its context holds binaries and nothing else (`.dockerignore` lets only `Dockerfile`, `bin/` and `image/` through), and `scripts/image-context.sh` fills it. Three ways:

```sh
mise run image                                   # this checkout's own cargo build, tagged ghcr.io/chussenot/jud:dev
scripts/image-context.sh --release v0.10.4       # a release's tarballs, verified against its SHA256SUMS
scripts/image-context.sh --dist dist/            # a directory of those tarballs, which is what the workflow does
docker buildx build --load -t ghcr.io/chussenot/jud:local .
```

`--local` builds one platform, the machine's, for trying a change to the `Dockerfile`; the release workflow builds both from the tarballs it just checked and pushes nothing on a dry run, so a broken context is found before a tag is cut. A release image is only ever pushed by that workflow ([Releasing](project/releasing.md)).

## What was checked

On 2026-10-07 against `ghcr.io/chussenot/jud:0.10.4` (digest `sha256:1376d5f9…`, `linux/amd64` on this machine), Docker 27.2.1: `--version`, `config` with and without a mounted file, `check` and `lower` over a read-only mount, a run refused without a key (status 2) and without `-i` (status 2), a mounted file with mode 600 refused as `Permission denied`, the replay through `JUD_REPLAY` and through `--replay`, a run against the host's Ollama 0.40.0 and `tev1:0.8b` through `--network host`, the same through `host.docker.internal` refused as expected, `completion bash` from the image, and `gh attestation verify` exiting 0.
