#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <tag>" >&2
  exit 1
fi

TAG="$1"

if ! git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  echo "Tag '$TAG' was not found in local refs/tags." >&2
  exit 1
fi

git fetch --no-tags origin main

TAG_COMMIT="$(git rev-list -n1 "$TAG")"
MAIN_HEAD="$(git rev-parse "origin/main")"

if ! git merge-base --is-ancestor "$TAG_COMMIT" "$MAIN_HEAD"; then
  echo "Tag '$TAG' points to commit $TAG_COMMIT which is not reachable from origin/main ($MAIN_HEAD)." >&2
  echo "Create release tags from main (or merge the release commit into main before tagging)." >&2
  exit 1
fi

echo "Tag '$TAG' is reachable from origin/main."
