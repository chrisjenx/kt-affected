#!/usr/bin/env bash
# Static discovery of each pinned open-source project must equal its golden file.
#
#   scripts/oss-golden.sh BIN [--update]
#
# A deliberate discovery change re-runs this with --update and commits the new goldens; the diff in
# the pull request is the review of what discovery now sees differently.
set -euo pipefail
cd "$(dirname "$0")/.."
BIN=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
UPDATE=${2:-}
scripts/oss-checkout.sh .oss > /dev/null
status=0
while IFS=$'\t' read -r name _ _; do
  [ -n "$name" ] || continue
  golden=tests/oss/$name.discover.json
  "$BIN" discover --root ".oss/$name" --json --static 2>/dev/null > ".oss/$name.discover.json"
  if [ "$UPDATE" = --update ]; then
    cp ".oss/$name.discover.json" "$golden"
    echo "updated $golden"
  elif diff -u "$golden" ".oss/$name.discover.json" > ".oss/$name.diff"; then
    echo "ok $name"
  else
    echo "::error::static discovery of $name changed; review and run scripts/oss-golden.sh $1 --update"
    head -80 ".oss/$name.diff"
    status=1
  fi
done < tests/oss/projects.tsv
exit $status
