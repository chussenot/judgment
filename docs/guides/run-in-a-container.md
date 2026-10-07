---
title: Run in a container
description: How to run jud as the ghcr.io/chussenot/jud image: how a rubric and a state reach it through the working directory and stdin, what the non-root user without a shell changes, and the four ways a backend is reached from inside, a key, another server, the host's Ollama, a replay directory.
status: current
last_reviewed: 2026-10-07
tags: [judgment, jud, container, docker, how-to]
---

# Run in a container

**You will** evaluate a state against a rubric with nothing installed but Docker. **Prerequisites:** Docker, or another OCI runtime; a `.jud` rubric ([Write a rubric](write-a-rubric.md)). What the image holds is [The container image](../reference/container-image.md). Every command here was run against `ghcr.io/chussenot/jud:0.10.4` on 2026-10-07.

```sh
cat event.json | docker run -i --rm -v "$PWD:/work" ghcr.io/chussenot/jud triage.jud
```

That line is the whole model: `-i` carries stdin into the container, `-v "$PWD:/work"` puts the current directory where the binary looks for files, and everything after the image name is `jud`'s own command line ([The jud command line](../reference/cli.md)).

## Files reach it through `/work`

The container sees only what is mounted. The working directory is `/work`, so a mount there makes a rubric, a state file or a recordings directory reachable by a relative path. Mount read-only: no subcommand writes a file.

```sh
# Read documents as the crate reads them; status 2 if any is refused.
docker run --rm -v "$PWD/examples/jud:/work:ro" ghcr.io/chussenot/jud check triage.jud triage-cases.jud

# What a rubric asks for a state, without any backend.
docker run --rm -v "$PWD/examples/jud:/work:ro" ghcr.io/chussenot/jud lower triage.jud --state '{"message": "hi"}'
```

The binary runs as uid 65532, so a mounted file must be readable by that user: a file with mode 600 owned by you is refused with `Permission denied (os error 13)`. World-readable files (644, the usual) need nothing. There is no shell inside, so `docker exec` has nothing to run; a look around the image is `docker image inspect`.

Stdin is a file too. `docker run` passes stdin only with `-i`; without it the container reads an empty stream and refuses, `stdin is empty: pipe the JSON state in`, status 2. `-t` is never needed and would mangle the JSON on stdout.

## Reaching a backend

Inside the container the backend is chosen as outside ([Configuration](../reference/configuration.md)): the environment, then the file at `/home/jud/.config/jud/config.yaml`, then the defaults.

**The hosted API, with a key.** Pass the key as an environment variable, never baked into an image or written into a mounted file:

```sh
cat event.json | docker run -i --rm -v "$PWD:/work:ro" -e TYPESAFE_API_KEY ghcr.io/chussenot/jud triage.jud
```

`-e TYPESAFE_API_KEY` with no value copies the variable from the shell that runs `docker`, so the key appears on no command line. The CA bundle in the image is what makes the TLS handshake succeed.

**Another server, by URL.** `-e TYPESAFE_BASE_URL=https://...` with `-e TYPESAFE_API_KEY` whatever that server wants.

**The host's Ollama.** Ollama listens on `127.0.0.1:11434`, which inside a container is the container's own loopback. `--network host` gives the container the host's network namespace, and the configuration file for a local model does the rest:

```sh
echo '{"message": "My payouts have been failing for 3 days."}' \
  | docker run -i --rm --network host \
      -v "$PWD/examples/jud:/work:ro" \
      -v "$PWD/examples/jud/config-tev1.yaml:/home/jud/.config/jud/config.yaml:ro" \
      ghcr.io/chussenot/jud triage.jud
```

The alternative, `--add-host=host.docker.internal:host-gateway` with `TYPESAFE_BASE_URL=http://host.docker.internal:11434`, reaches the host's addresses but not its loopback: against an Ollama bound to `127.0.0.1` it fails with `transport error after 3 attempts`, and works only once Ollama listens on every interface (`OLLAMA_HOST=0.0.0.0`). `--network host` is a Linux feature; on Docker Desktop it is a setting to enable.

**No server at all.** `--replay DIR`, or `-e JUD_REPLAY=DIR`, answers from recordings under a mounted directory, with no key and no network ([Record, replay and test](record-replay-and-test.md)):

```sh
cat event.json | docker run -i --rm -v "$PWD/examples:/work:ro" \
  -e JUD_REPLAY=/work/recordings/jud_calibration ghcr.io/chussenot/jud jud/triage.jud
```

The directory is given as the container sees it; with the flag, the path is relative to `/work` like any other: `--replay recordings/jud_calibration jud/triage.jud`.

## Pin the image

In anything that runs twice, pin the version tag or the digest:

```sh
docker pull ghcr.io/chussenot/jud:0.10.4
docker image inspect ghcr.io/chussenot/jud:0.10.4 --format '{{index .RepoDigests 0}}'
gh attestation verify oci://ghcr.io/chussenot/jud:0.10.4 --repo chussenot/judgment
```

## Next

- [Run in CI](run-in-ci.md): the image in a job, and a job image of your own that takes the binary from it.
