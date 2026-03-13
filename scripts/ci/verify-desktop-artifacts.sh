#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <runner-os> <bundle-dir>" >&2
  exit 2
fi

RUNNER_OS="$1"
BUNDLE_DIR="$2"

if [[ ! -d "$BUNDLE_DIR" ]]; then
  echo "Bundle directory not found: $BUNDLE_DIR" >&2
  exit 1
fi

require_match() {
  local pattern="$1"
  local label="$2"
  if ! find "$BUNDLE_DIR" -type f -path "$pattern" | grep -q .; then
    echo "Missing required artifact: $label ($pattern)" >&2
    exit 1
  fi
}

case "$RUNNER_OS" in
  Windows)
    require_match "*/nsis/*-setup.exe" "Windows NSIS installer"
    require_match "*/nsis/*-setup.exe.sig" "Windows NSIS updater signature"
    require_match "*/latest.json" "Windows updater manifest"
    ;;
  macOS)
    require_match "*/dmg/*.dmg" "macOS DMG installer"
    require_match "*/macos/*.app.tar.gz" "macOS updater bundle"
    require_match "*/macos/*.app.tar.gz.sig" "macOS updater signature"
    require_match "*/latest.json" "macOS updater manifest"
    ;;
  Linux)
    require_match "*/appimage/*.AppImage.tar.gz" "Linux updater bundle"
    require_match "*/appimage/*.AppImage.tar.gz.sig" "Linux updater signature"
    require_match "*/latest.json" "Linux updater manifest"
    ;;
  *)
    echo "Unsupported runner OS: $RUNNER_OS" >&2
    exit 2
    ;;
esac

echo "Desktop artifact validation passed for $RUNNER_OS"
