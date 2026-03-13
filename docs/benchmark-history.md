# Benchmark History Log

## Purpose

This file is the historical ledger for real-corpus benchmark activity.

Use it to track:

- app/workspace version
- benchmark harness version or relevant script changes
- corpus changes
- execution notes
- measured results
- regressions or improvements
- follow-up actions

This is intentionally a chronological log, not a spec. The canonical harness and scenario contract still live in [`docs/benchmarks.md`](./benchmarks.md).

## Logging Rules

For each new entry, capture at minimum:

- date
- code version or git revision
- benchmark scope (`smoke-real`, `bench-real-core`, `bench-real-rich`, or `all`)
- changes since previous run
- environment notes if relevant
- result summary
- validation status
- follow-up actions

If a run is partial, say so explicitly.

## Entry Template

```md
## YYYY-MM-DD - Short Title

- Version:
- Scope:
- Trigger:
- Changes since previous run:
- Corpus notes:
- Environment notes:
- Commands:
- Results:
- Validations:
- Notes:
- Follow-up:
```

---

## 2026-03-13 - Real Corpus Harness Introduced

- Version: `2.9.6`
- Scope: harness implementation, corpus validation, `smoke-real`
- Trigger: first integration of `testfiles/` as benchmark corpus
- Changes since previous run:
  - added `scripts/bench/run-real-corpus.ps1`
  - added `scripts/ci/verify-real-corpus-harness.sh`
  - added structured CLI output via `--benchmark-json`
  - documented benchmark corpus and scenario matrix
- Corpus notes:
  - `Test1.csv`, `Test2.csv`, `Test3.csv` used as large text corpora
  - `spanish.txt` used as dense small dictionary corpus
  - Biology PDF pair validated as byte-identical via SHA-256
  - `La metamorfosis.epub` used as rich-input smoke corpus
- Environment notes:
  - local Windows workspace
  - local `bash.exe` does not see the Windows workspace path; CI helper validated logically but not runnable end-to-end through local `bash`
- Commands:
  - `cargo fmt --all`
  - `cargo test --workspace --locked`
  - `pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -ListScenarios`
  - `pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -ValidateCorpus -RequireCorpus`
  - `pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite smoke-real -SkipBuild`
- Results:
  - harness listed all scenarios successfully
  - corpus validation passed
  - `smoke-real` produced JSON/CSV outputs under `target/real-corpus-bench`
  - `small_dictionary`:
    - `ram_preserve`: `19 ms`, `70,162 tokens`, `65,809 unique`, `4,353 duplicates`
    - `ram_unordered`: `15 ms`, same counts
    - `disk_preserve`: `251 ms`, same counts
  - `small_epub`:
    - `ram_preserve`: `9 ms`, `extract_ms=3`, `21,641 tokens`, `4,498 unique`
    - `auto_preserve`: `9 ms`, `mode_effective=ram`, `extract_ms=3`
- Validations:
  - passed
  - no failed validation records
- Notes:
  - rich-input extraction timing is now separated from core pipeline timing
  - CLI summary output is now suitable for machine ingestion
- Follow-up:
  - run the full `all` suite and store the result as the first baseline ledger entry

---

## 2026-03-13 - First Full Real Corpus Baseline

- Version: `2.9.6`
- Scope: `all`
- Trigger: first complete execution of all real-corpus benchmark suites after harness integration
- Changes since previous run:
  - no engine changes between smoke validation and full-suite execution
  - validation logic extended to cover `small_epub`
- Corpus notes:
  - same corpus as the harness-introduction run
  - Biology PDF pair remained SHA-256 identical
- Environment notes:
  - local Windows workspace
  - release CLI binary reused via `-SkipBuild`
- Commands:
  - `pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite all -SkipBuild`
  - `pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -ValidateCorpus -RequireCorpus`
  - `cargo test --workspace --locked`
- Results:
  - outputs written to:
    - `target/real-corpus-bench/results-20260313-124209.json`
    - `target/real-corpus-bench/validations-20260313-124209.json`
  - validation summary:
    - `17` checks
    - `0` failures
  - key scenario results:
    - `single_huge_unique`
      - `auto_preserve`: `1688 ms`, `mode_effective=ram`
      - `disk_preserve`: `1907 ms`
      - `ram_preserve`: `1944 ms`
      - `ram_unordered`: `2579 ms`
    - `two_huge_partial_overlap`
      - `disk_preserve`: `3956 ms`
      - `ram_preserve`: `5735 ms`
      - `auto_preserve`: `5770 ms`, `mode_effective=ram`
      - `ram_unordered`: `6535 ms`
      - `disk_alpha_global`: `10591 ms`
    - `rich_duplicate_pair`
      - `ram_preserve`: `75 ms`, `extract_ms=8813`, `644,158 tokens`, `33,058 unique`, `611,100 duplicates`
      - `disk_preserve`: `319 ms`, `extract_ms=8778`, same counts
      - `auto_preserve`: `76 ms`, `extract_ms=8559`, `mode_effective=ram`, same counts
    - `rich_mixed`
      - `ram_preserve`: `52 ms`, `extract_ms=16957`
      - `disk_preserve`: `295 ms`, `extract_ms=17018`
      - `auto_preserve`: `39 ms`, `extract_ms=13079`, `mode_effective=ram`
- Validations:
  - passed
  - `single_huge_unique`: `unique_tokens == tokens_seen` confirmed
  - `two_huge_partial_overlap`: count parity across RAM/DISK/AUTO confirmed
  - `rich_duplicate_pair`: duplicate collapse confirmed
  - `rich_mixed`: extraction stage detected in all variants
- Notes:
  - `AUTO` still resolves to `ram` for the current large-file scenarios on this machine
  - `DISK + PreserveFirstSeen` is materially faster than RAM on `two_huge_partial_overlap`
  - `DISK + Alphabetical + GlobalPerfect` remains much slower, as expected
  - extraction dominates total wall time on rich-input workloads; dedupe path is not the primary cost there
- Follow-up:
  - use `two_huge_partial_overlap` as the primary optimization target
  - benchmark future DISK-path changes against this entry first
  - if `AUTO` heuristics change, compare directly against this baseline
