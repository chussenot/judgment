#!/usr/bin/env sh
# Render the Homebrew formula for `jud` from a release's checksums.
#
#   scripts/homebrew-formula.sh VERSION SHA256SUMS > jud.rb
#
# VERSION is the release's version (0.10.2, no v) and SHA256SUMS the file the
# release workflow attaches to the GitHub release: one line per tarball,
# `<sha256>  jud-<tag>-<triple>.tar.gz`. The formula installs the release's
# tarballs, one per platform the workflow builds (docs/cli.md, Platforms), and
# nothing is compiled: Apple silicon from the darwin tarball, Linux x86-64 and
# arm64 from the musl ones. The version is the one Homebrew reads from the
# tarball's name (an explicit `version` is an audit failure, "redundant"). Intel macOS has no tarball and no formula branch;
# it installs from crates.io, as the page says.
#
# The release workflow runs this and commits the result to the tap
# (github.com/chussenot/homebrew-tap, Formula/jud.rb), so the file there is
# this script's output and nothing else; edit this script, not the tap. The
# tap's CI runs `brew audit --strict`, installs and tests what is committed.
set -eu

version=${1:?usage: $0 VERSION SHA256SUMS}
sums=${2:?usage: $0 VERSION SHA256SUMS}
case "$version" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "homebrew-formula: '$version' is not a version" >&2; exit 2 ;;
esac
[ -r "$sums" ] || { echo "homebrew-formula: cannot read $sums" >&2; exit 2; }

# The checksum of the tarball for a target triple, whatever the tag part of
# the name (a dry run names its tarballs jud-dryrun-<triple>.tar.gz).
sum_for() {
  s=$(awk -v t="-$1.tar.gz" 'index($2, t) == length($2) - length(t) + 1 { print $1; exit }' "$sums")
  case "$s" in
    [0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]*) ;;
    *) echo "homebrew-formula: no checksum for $1 in $sums" >&2; exit 1 ;;
  esac
  [ ${#s} -eq 64 ] || { echo "homebrew-formula: '$s' is not a sha256" >&2; exit 1; }
  printf '%s' "$s"
}

darwin_arm=$(sum_for aarch64-apple-darwin)
linux_arm=$(sum_for aarch64-unknown-linux-musl)
linux_x86=$(sum_for x86_64-unknown-linux-musl)
base="https://github.com/chussenot/judgment/releases/download/v$version"

cat <<EOF
# Rendered by scripts/homebrew-formula.sh in github.com/chussenot/judgment on
# each release; edit that script, not this file. The tarballs are the GitHub
# release's, built on native runners and checked before they were packaged
# (docs/cli.md); each has a build-provenance attestation:
#   gh attestation verify jud-v$version-<triple>.tar.gz --repo chussenot/judgment
class Jud < Formula
  desc "Evaluate JSON against a .jud rubric with a calibrated System One model"
  homepage "https://github.com/chussenot/judgment"
  license "MIT"

  livecheck do
    url :stable
    strategy :github_latest
  end

  # Intel macOS has no release tarball (no native runner builds one); it
  # installs from crates.io: cargo install judgment --features cli
  on_macos do
    depends_on arch: :arm64
    on_arm do
      url "$base/jud-v$version-aarch64-apple-darwin.tar.gz"
      sha256 "$darwin_arm"
    end
  end

  # Statically linked (musl): any distribution, glibc or not.
  on_linux do
    on_arm do
      url "$base/jud-v$version-aarch64-unknown-linux-musl.tar.gz"
      sha256 "$linux_arm"
    end
    on_intel do
      url "$base/jud-v$version-x86_64-unknown-linux-musl.tar.gz"
      sha256 "$linux_x86"
    end
  end

  def install
    bin.install "jud"
    doc.install "README.md", "CHANGELOG.md"
    # \`jud completion <shell>\` prints the script clap derives from the
    # command tree, so it cannot drift from the binary.
    generate_completions_from_executable(bin/"jud", "completion")
  end

  test do
    assert_match "jud #{version}", shell_output("#{bin}/jud --version")
    # A document the binary reads the way the crate does: a minimal Rubric
    # in the envelope the format takes (jud/v1.3), accepted with status 0.
    (testpath/"ok.jud").write <<~YAML
      apiVersion: jud/v1.3
      kind: Rubric
      metadata:
        name: ok
      spec:
        questions:
          fine:
            type: noul
            instructions: Is \`message\` fine?
        policy:
          fine:
            threshold: 0.5
    YAML
    system bin/"jud", "check", testpath/"ok.jud"
    # Without a key or a network, a run says what is missing and exits 2.
    output = pipe_output("#{bin}/jud #{testpath}/ok.jud 2>&1", "{}", 2)
    assert_match "no API key", output
  end
end
EOF
