#!/bin/sh
# Run cargo the way mise.toml wraps it: through mbx, which owns target/ (a
# symlink into its store, whose artifacts are read-only hard links). Plain
# cargo from a git hook cannot overwrite them, so a rebuild there failed a
# push with "output file ... is not writeable". Falls back to plain cargo
# where mise is absent (CI's prek job skips the cargo hooks anyway).
if command -v mise >/dev/null 2>&1; then
  exec mise exec -- cargo "$@"
fi
exec cargo "$@"
