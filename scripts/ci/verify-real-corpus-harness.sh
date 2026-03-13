#!/usr/bin/env bash
set -euo pipefail

pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -ListScenarios

if [ -d testfiles ]; then
  pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -ValidateCorpus -RequireCorpus
else
  echo "testfiles/ not present in this checkout; skipping corpus validation."
fi
