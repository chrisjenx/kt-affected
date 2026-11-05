#!/usr/bin/env bash
# Checks out every project in tests/oss/projects.tsv at its pinned commit under DIR (default .oss/).
# Read-only use: kt-affected parses their sources and build scripts; their Gradle is never run.
set -euo pipefail
cd "$(dirname "$0")/.."
DIR=${1:-.oss}
mkdir -p "$DIR"
while IFS=$'\t' read -r name url sha; do
  [ -n "$name" ] || continue
  if [ "$(git -C "$DIR/$name" rev-parse HEAD 2>/dev/null)" != "$sha" ]; then
    rm -rf "${DIR:?}/$name"
    git init -q "$DIR/$name"
    git -C "$DIR/$name" fetch -q --depth 1 "$url" "$sha"
    git -C "$DIR/$name" checkout -q FETCH_HEAD
  fi
  echo "$name $sha"
done < tests/oss/projects.tsv
