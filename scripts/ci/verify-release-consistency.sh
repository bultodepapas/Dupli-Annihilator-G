#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

TAG_INPUT="${1:-}"

extract_toml_version() {
  local file="$1"
  local value
  value="$(sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "$file" | head -n1)"
  if [[ -z "$value" ]]; then
    echo "Failed to read version from $file" >&2
    exit 1
  fi
  printf '%s' "$value"
}

DESKTOP_UI_VERSION="$(node -p "require('./apps/desktop/package.json').version")"
TAURI_CONF_VERSION="$(node -p "require('./apps/desktop/src-tauri/tauri.conf.json').version")"
TAURI_CRATE_VERSION="$(extract_toml_version "apps/desktop/src-tauri/Cargo.toml")"
CORE_CRATE_VERSION="$(extract_toml_version "crates/core/Cargo.toml")"
JOB_RUNNER_CRATE_VERSION="$(extract_toml_version "crates/job_runner/Cargo.toml")"
BACKEND_CRATE_VERSION="$(extract_toml_version "crates/backend/Cargo.toml")"
CLI_CRATE_VERSION="$(extract_toml_version "apps/cli/Cargo.toml")"

declare -A VERSIONS=(
  ["apps/desktop/package.json"]="$DESKTOP_UI_VERSION"
  ["apps/desktop/src-tauri/tauri.conf.json"]="$TAURI_CONF_VERSION"
  ["apps/desktop/src-tauri/Cargo.toml"]="$TAURI_CRATE_VERSION"
  ["crates/core/Cargo.toml"]="$CORE_CRATE_VERSION"
  ["crates/job_runner/Cargo.toml"]="$JOB_RUNNER_CRATE_VERSION"
  ["crates/backend/Cargo.toml"]="$BACKEND_CRATE_VERSION"
  ["apps/cli/Cargo.toml"]="$CLI_CRATE_VERSION"
)

BASE_VERSION="$DESKTOP_UI_VERSION"
SEMVER_RE='^[0-9]+\.[0-9]+\.[0-9]+$'

if [[ ! "$BASE_VERSION" =~ $SEMVER_RE ]]; then
  echo "Desktop UI version '$BASE_VERSION' is not strict semver (X.Y.Z)." >&2
  exit 1
fi

for file in "${!VERSIONS[@]}"; do
  version="${VERSIONS[$file]}"
  if [[ "$version" != "$BASE_VERSION" ]]; then
    echo "Version mismatch: $file=$version, expected $BASE_VERSION" >&2
    exit 1
  fi
  if [[ ! "$version" =~ $SEMVER_RE ]]; then
    echo "Invalid semver in $file: '$version'" >&2
    exit 1
  fi
done

if [[ -n "$TAG_INPUT" ]]; then
  TAG_RE='^v[0-9]+\.[0-9]+\.[0-9]+$'
  if [[ ! "$TAG_INPUT" =~ $TAG_RE ]]; then
    echo "Tag '$TAG_INPUT' is invalid. Expected format: vX.Y.Z" >&2
    exit 1
  fi

  EXPECTED_TAG="v$BASE_VERSION"
  if [[ "$TAG_INPUT" != "$EXPECTED_TAG" ]]; then
    echo "Tag/version mismatch: tag=$TAG_INPUT, expected=$EXPECTED_TAG" >&2
    exit 1
  fi
fi

echo "Release consistency check passed for version $BASE_VERSION"
