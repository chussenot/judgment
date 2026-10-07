# The jud command line as a container image: ghcr.io/chussenot/jud.
#
#   cat event.json | docker run -i --rm -v "$PWD:/work" ghcr.io/chussenot/jud triage.jud
#
# FROM scratch: the image is the statically linked musl binary and nothing
# else, no shell, no libc, no package manager, so there is no base image to
# patch and nothing to escalate into. The binary is not built here: it is
# the one the release workflow built on a native runner, ran, checked and
# packaged into the release tarball (docs/reference/cli.md, Platforms), copied in from
# a build context the workflow fills from those tarballs, one binary per
# platform under bin/<TARGETOS>/<TARGETARCH>/jud. So the image carries, byte
# for byte, the binary the tarball and the Homebrew formula carry, and a
# `docker build .` from a plain checkout fails for want of that context:
# scripts/image-context.sh fills it from a release or a local build.
#
# A run needs no network for `check`, `lower`, `completion` or `--replay`;
# a run against a backend reads TYPESAFE_API_KEY and TYPESAFE_BASE_URL from
# the environment, as outside a container (`-e TYPESAFE_API_KEY`). The
# configuration file is read from /home/jud/.config/jud/config.yaml, so a
# host file is mounted there.
FROM scratch

ARG TARGETOS
ARG TARGETARCH

# The directories first, with their own modes: a COPY --chmod into a
# directory that does not exist yet creates it with the file's mode, and
# 444 on /etc or /etc/ssl/certs has no execute bit, so a non-root user can
# read nothing under them and the client finds no roots. image/tree holds
# the empty tree with 755 directories (scripts/image-context.sh).
COPY --chmod=755 image/tree/ /

# A user that is not root, declared here since there is no /etc to add one
# to: uid and gid 65532, the one distroless images use for "nonroot".
COPY --chmod=444 image/passwd /etc/passwd
COPY --chmod=444 image/group /etc/group
COPY --chmod=555 bin/${TARGETOS}/${TARGETARCH}/jud /usr/local/bin/jud

# The client's TLS roots. reqwest's rustls feature verifies through the
# platform's store (rustls-platform-verifier, rustls-native-certs on Linux),
# which reads /etc/ssl/certs/ca-certificates.crt; an image without it fails
# every handshake to a backend. The bundle is the one the workflow's runner
# carries (Ubuntu's ca-certificates), copied into the context by
# scripts/image-context.sh, and SSL_CERT_FILE names it explicitly.
COPY --chmod=444 image/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

# HOME lets the configuration file be found; /work is where a rubric and an
# event file are mounted.
ENV HOME=/home/jud
WORKDIR /work
USER 65532:65532

ENTRYPOINT ["/usr/local/bin/jud"]
