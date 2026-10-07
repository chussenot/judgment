#!/usr/bin/env sh
# Fill the container image's build context (Dockerfile): the jud binaries,
# one per platform under bin/<os>/<arch>/jud, the passwd and group that give
# the image its non-root user, and the CA bundle the client verifies with.
#
#   scripts/image-context.sh --release TAG      from the GitHub release's tarballs
#   scripts/image-context.sh --dist DIR         from a directory of those tarballs
#   scripts/image-context.sh --local            from `cargo build` on this machine
#
# Then: docker buildx build --platform linux/amd64,linux/arm64 -t ghcr.io/chussenot/jud:X.Y.Z .
#
# The release workflow uses --dist on the tarballs it just built and checked
# (the same bytes the release, the formula and binstall carry), so the image
# is never built from a second compilation. --release verifies the tarballs
# against the release's SHA256SUMS first. --local is for trying the
# Dockerfile on a checkout: one platform, this machine's.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
die() { echo "image-context: $*" >&2; exit 1; }

# Target triple -> the image's os/arch directory.
place() {
  case "$1" in
    x86_64-unknown-linux-musl) echo linux/amd64 ;;
    aarch64-unknown-linux-musl) echo linux/arm64 ;;
    *) return 1 ;;
  esac
}

rm -rf bin image
mkdir -p bin image
# The image's directories, copied in first with 755 so that a later COPY
# --chmod=444 of a file does not create its parents without the execute bit
# (which locks a non-root user out of /etc; the Dockerfile says more).
mkdir -p image/tree/etc/ssl/certs image/tree/usr/local/bin image/tree/home/jud image/tree/work

# A non-root user with no shell, uid and gid 65532 as distroless names it.
printf 'root:x:0:0:root:/:/sbin/nologin\njud:x:65532:65532:jud:/home/jud:/sbin/nologin\n' > image/passwd
printf 'root:x:0:\njud:x:65532:\n' > image/group

# The CA bundle rustls-native-certs reads on Linux, from this machine (the
# workflow's Ubuntu runner, kept current by its image).
bundle=${SSL_CERT_FILE:-/etc/ssl/certs/ca-certificates.crt}
[ -r "$bundle" ] || die "no CA bundle at $bundle (apt install ca-certificates, or set SSL_CERT_FILE)"
cp "$bundle" image/ca-certificates.crt

from_tarballs() {
  dir=$1
  n=0
  for tgz in "$dir"/jud-*-linux-musl.tar.gz; do
    [ -f "$tgz" ] || continue
    name=$(basename "$tgz" .tar.gz)
    triple=${name#jud-*-}
    # the tag part of the name may itself hold hyphens; the triple is the
    # last three dash-separated words joined back: <arch>-unknown-linux-musl
    triple=$(echo "$name" | awk -F- '{print $(NF-3)"-"$(NF-2)"-"$(NF-1)"-"$NF}')
    dest=$(place "$triple") || continue
    mkdir -p "bin/$dest"
    tar -xzf "$tgz" -C "bin/$dest" --strip-components=1 "$name/jud"
    chmod 755 "bin/$dest/jud"
    echo "bin/$dest/jud  <-  $tgz"
    n=$((n + 1))
  done
  [ "$n" -gt 0 ] || die "no jud-*-linux-musl.tar.gz under $dir"
}

case "${1:-}" in
  --release)
    tag=${2:?usage: $0 --release TAG}
    tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT INT TERM
    base="https://github.com/chussenot/judgment/releases/download/$tag"
    for triple in x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
      curl -fsSLO --output-dir "$tmp" "$base/jud-$tag-$triple.tar.gz"
    done
    curl -fsSLO --output-dir "$tmp" "$base/SHA256SUMS"
    (cd "$tmp" && sha256sum --ignore-missing --quiet -c SHA256SUMS)
    from_tarballs "$tmp"
    ;;
  --dist)
    from_tarballs "${2:?usage: $0 --dist DIR}"
    ;;
  --local)
    case "$(uname -m)" in
      x86_64) triple=x86_64-unknown-linux-musl ;;
      aarch64|arm64) triple=aarch64-unknown-linux-musl ;;
      *) die "no musl target for $(uname -m)" ;;
    esac
    dest=$(place "$triple")
    cargo build -q --release --locked --features cli --bin jud --target "$triple"
    mkdir -p "bin/$dest"
    cp "target/$triple/release/jud" "bin/$dest/jud"
    echo "bin/$dest/jud  <-  target/$triple/release/jud"
    ;;
  *)
    echo "usage: $0 --release TAG | --dist DIR | --local" >&2; exit 2 ;;
esac
