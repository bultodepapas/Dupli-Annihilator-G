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
---

## 2026-03-13 - Real Corpus Run (smoke-real)

- Version: `2.9.6`
- Git revision: `73fb97d`
- Scope: `smoke-real`
- Trigger: manual benchmark run
- Changes since previous run:
  - fill this in if the run is being appended after code or corpus changes
- Corpus notes:
  - benchmark scenarios resolved from local `testfiles/`
- Environment notes:
  - generated automatically by `scripts/bench/run-real-corpus.ps1`
- Commands:
  - `pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite smoke-real -SkipBuild -AppendHistory`
- Results:
  - outputs written to:
    - `D:\DEVELOMENT\Dupli-Annihilator-G-1\target\real-corpus-bench\results-20260313-124905.json`
    - `D:\DEVELOMENT\Dupli-Annihilator-G-1\target\real-corpus-bench\validations-20260313-124905.json`
  - validation summary:
    - `5` checks
    - `0` failures
  - key scenario results:
    - `small_dictionary`
      - `disk_preserve`, `246 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `15 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `14 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=ram`
    - `small_epub`
      - `auto_preserve`, `9 ms`, `21641 tokens`, `4498 unique`, `17143 duplicates`, `mode_effective=ram`, `extract_ms=3`
      - `ram_preserve`, `9 ms`, `21641 tokens`, `4498 unique`, `17143 duplicates`, `mode_effective=ram`, `extract_ms=2`
- Validations:
  - passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_epub` / `rich_smoke_stage_present`: passed
  - `small_epub` / `rich_smoke_stage_present`: passed
- Notes:
  - artifact timestamp: `20260313-124905`
  - edit this section before pasting into `docs/benchmark-history.md` if the run needs more context
- Follow-up:
  - fill in the next action based on these results
---

## 2026-03-13 - Full Real Corpus Run

- Version: `2.9.6`
- Git revision: `73fb97d`
- Scope: `all`
- Trigger: manual benchmark run
- Changes since previous run:
  - implemented host-aware `AUTO` selection using available memory plus sampled corpus telemetry
  - added `AUTO` decision telemetry to benchmark JSON and summaries
  - parallelized DISK bucket reduction while preserving bucket-order concatenation
  - added ASCII fast path tokenization and larger explicit I/O buffers
- Corpus notes:
  - benchmark scenarios resolved from local `testfiles/`
- Environment notes:
  - generated automatically by `scripts/bench/run-real-corpus.ps1`
- Commands:
  - `pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite all -SkipBuild -AppendHistory`
- Results:
  - outputs written to:
    - `D:\DEVELOMENT\Dupli-Annihilator-G-1\target\real-corpus-bench\results-20260313-131228.json`
    - `D:\DEVELOMENT\Dupli-Annihilator-G-1\target\real-corpus-bench\validations-20260313-131228.json`
  - validation summary:
    - `17` checks
    - `0` failures
  - key scenario results:
    - `rich_duplicate_pair`
      - `auto_preserve`, `57 ms`, `644158 tokens`, `33058 unique`, `611100 duplicates`, `mode_effective=ram`, `extract_ms=8514`
      - `disk_preserve`, `481 ms`, `644158 tokens`, `33058 unique`, `611100 duplicates`, `mode_effective=disk`, `extract_ms=8738`
      - `ram_preserve`, `57 ms`, `644158 tokens`, `33058 unique`, `611100 duplicates`, `mode_effective=ram`, `extract_ms=8886`
    - `rich_mixed`
      - `auto_preserve`, `26 ms`, `380182 tokens`, `45563 unique`, `334619 duplicates`, `mode_effective=ram`, `extract_ms=17632`
      - `disk_preserve`, `467 ms`, `380182 tokens`, `45563 unique`, `334619 duplicates`, `mode_effective=disk`, `extract_ms=17920`
      - `ram_preserve`, `38 ms`, `380182 tokens`, `45563 unique`, `334619 duplicates`, `mode_effective=ram`, `extract_ms=17233`
    - `single_huge_unique`
      - `auto_preserve`, `2174 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=ram`
      - `disk_preserve`, `1664 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `2239 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `2682 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=ram`
    - `small_dictionary`
      - `disk_preserve`, `494 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `18 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `13 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=ram`
    - `small_epub`
      - `auto_preserve`, `2 ms`, `21641 tokens`, `4498 unique`, `17143 duplicates`, `mode_effective=ram`, `extract_ms=14`
      - `ram_preserve`, `2 ms`, `21641 tokens`, `4498 unique`, `17143 duplicates`, `mode_effective=ram`, `extract_ms=11`
    - `two_huge_partial_overlap`
      - `auto_preserve`, `3303 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=disk`
      - `disk_alpha_global`, `11969 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=disk`
      - `disk_preserve`, `3451 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `6569 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `7605 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=ram`
- Validations:
  - passed
  - `rich_duplicate_pair` / `count_parity`: passed
  - `rich_duplicate_pair` / `pair_collapses_duplicates`: passed
  - `rich_duplicate_pair` / `pair_collapses_duplicates`: passed
  - `rich_duplicate_pair` / `pair_collapses_duplicates`: passed
  - `rich_mixed` / `rich_extraction_stage_present`: passed
  - `rich_mixed` / `rich_extraction_stage_present`: passed
  - `rich_mixed` / `rich_extraction_stage_present`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_epub` / `rich_smoke_stage_present`: passed
  - `small_epub` / `rich_smoke_stage_present`: passed
  - `two_huge_partial_overlap` / `ram_disk_auto_count_parity`: passed
- Notes:
  - artifact timestamp: `20260313-131228`
  - `AUTO` now resolves to `disk` for `two_huge_partial_overlap` on the baseline machine
  - the target case improved materially, but the harness still computed `core_pipeline_ms` incorrectly for rich-input scenarios in this run
- Follow-up:
  - fix the harness `core_pipeline_ms` calculation and rerun the full suite once
---

## 2026-03-13 - Full Real Corpus Run

- Version: `2.9.6`
- Git revision: `73fb97d`
- Scope: `all`
- Trigger: manual benchmark run
- Changes since previous run:
  - fixed `run-real-corpus.ps1` so `core_pipeline_ms` is taken directly from `summary.elapsed_ms`
  - reran the full suite with the same release binary to refresh artifacts
- Corpus notes:
  - benchmark scenarios resolved from local `testfiles/`
- Environment notes:
  - generated automatically by `scripts/bench/run-real-corpus.ps1`
- Commands:
  - `pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite all -SkipBuild -AppendHistory`
- Results:
  - outputs written to:
    - `D:\DEVELOMENT\Dupli-Annihilator-G-1\target\real-corpus-bench\results-20260313-131537.json`
    - `D:\DEVELOMENT\Dupli-Annihilator-G-1\target\real-corpus-bench\validations-20260313-131537.json`
  - validation summary:
    - `17` checks
    - `0` failures
  - key scenario results:
    - `rich_duplicate_pair`
      - `auto_preserve`, `87 ms`, `644158 tokens`, `33058 unique`, `611100 duplicates`, `mode_effective=ram`, `extract_ms=8519`
      - `disk_preserve`, `467 ms`, `644158 tokens`, `33058 unique`, `611100 duplicates`, `mode_effective=disk`, `extract_ms=8555`
      - `ram_preserve`, `57 ms`, `644158 tokens`, `33058 unique`, `611100 duplicates`, `mode_effective=ram`, `extract_ms=8696`
    - `rich_mixed`
      - `auto_preserve`, `27 ms`, `380182 tokens`, `45563 unique`, `334619 duplicates`, `mode_effective=ram`, `extract_ms=17916`
      - `disk_preserve`, `475 ms`, `380182 tokens`, `45563 unique`, `334619 duplicates`, `mode_effective=disk`, `extract_ms=17674`
      - `ram_preserve`, `35 ms`, `380182 tokens`, `45563 unique`, `334619 duplicates`, `mode_effective=ram`, `extract_ms=17515`
    - `single_huge_unique`
      - `auto_preserve`, `1836 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=ram`
      - `disk_preserve`, `1591 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `2066 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `2480 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=ram`
    - `small_dictionary`
      - `disk_preserve`, `451 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `15 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `17 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=ram`
    - `small_epub`
      - `auto_preserve`, `2 ms`, `21641 tokens`, `4498 unique`, `17143 duplicates`, `mode_effective=ram`, `extract_ms=12`
      - `ram_preserve`, `2 ms`, `21641 tokens`, `4498 unique`, `17143 duplicates`, `mode_effective=ram`, `extract_ms=12`
    - `two_huge_partial_overlap`
      - `auto_preserve`, `3290 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=disk`
      - `disk_alpha_global`, `11151 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=disk`
      - `disk_preserve`, `3208 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `6129 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `6871 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=ram`
- Validations:
  - passed
  - `rich_duplicate_pair` / `count_parity`: passed
  - `rich_duplicate_pair` / `pair_collapses_duplicates`: passed
  - `rich_duplicate_pair` / `pair_collapses_duplicates`: passed
  - `rich_duplicate_pair` / `pair_collapses_duplicates`: passed
  - `rich_mixed` / `rich_extraction_stage_present`: passed
  - `rich_mixed` / `rich_extraction_stage_present`: passed
  - `rich_mixed` / `rich_extraction_stage_present`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_epub` / `rich_smoke_stage_present`: passed
  - `small_epub` / `rich_smoke_stage_present`: passed
  - `two_huge_partial_overlap` / `ram_disk_auto_count_parity`: passed
- Notes:
  - artifact timestamp: `20260313-131537`
  - `AUTO` remained `ram` for `single_huge_unique` and switched to `disk` for `two_huge_partial_overlap`
  - `disk_preserve` reached `3208 ms` on `two_huge_partial_overlap`, meeting the phase-1 target of `<= 3360 ms`
  - `auto_preserve` reached `3290 ms` on `two_huge_partial_overlap`, within 5% of `disk_preserve`
- Follow-up:
  - next engine work should focus on the `single_huge_unique` RAM path, where `disk_preserve` is still faster on this machine
---

## 2026-03-13 - Full Real Corpus Run

- Version: `2.9.6`
- Git revision: `73fb97d`
- Scope: `all`
- Trigger: manual benchmark run
- Changes since previous run:
  - switched RAM store insertion from `contains()+insert()` to direct insert to reduce double-lookup overhead on unique-heavy workloads
  - rebuilt release binary and reran the full real-corpus suite
- Corpus notes:
  - benchmark scenarios resolved from local `testfiles/`
- Environment notes:
  - generated automatically by `scripts/bench/run-real-corpus.ps1`
- Commands:
  - `pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite all -SkipBuild -AppendHistory`
- Results:
  - outputs written to:
    - `D:\DEVELOMENT\Dupli-Annihilator-G-1\target\real-corpus-bench\results-20260313-132027.json`
    - `D:\DEVELOMENT\Dupli-Annihilator-G-1\target\real-corpus-bench\validations-20260313-132027.json`
  - validation summary:
    - `17` checks
    - `0` failures
  - key scenario results:
    - `rich_duplicate_pair`
      - `auto_preserve`, `88 ms`, `644158 tokens`, `33058 unique`, `611100 duplicates`, `mode_effective=ram`, `extract_ms=8357`
      - `disk_preserve`, `484 ms`, `644158 tokens`, `33058 unique`, `611100 duplicates`, `mode_effective=disk`, `extract_ms=8359`
      - `ram_preserve`, `79 ms`, `644158 tokens`, `33058 unique`, `611100 duplicates`, `mode_effective=ram`, `extract_ms=8269`
    - `rich_mixed`
      - `auto_preserve`, `41 ms`, `380182 tokens`, `45563 unique`, `334619 duplicates`, `mode_effective=ram`, `extract_ms=16171`
      - `disk_preserve`, `469 ms`, `380182 tokens`, `45563 unique`, `334619 duplicates`, `mode_effective=disk`, `extract_ms=17384`
      - `ram_preserve`, `42 ms`, `380182 tokens`, `45563 unique`, `334619 duplicates`, `mode_effective=ram`, `extract_ms=16450`
    - `single_huge_unique`
      - `auto_preserve`, `1829 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=ram`
      - `disk_preserve`, `1587 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `1907 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `2300 ms`, `4804929 tokens`, `4804929 unique`, `0 duplicates`, `mode_effective=ram`
    - `small_dictionary`
      - `disk_preserve`, `483 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `16 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `16 ms`, `70162 tokens`, `65809 unique`, `4353 duplicates`, `mode_effective=ram`
    - `small_epub`
      - `auto_preserve`, `3 ms`, `21641 tokens`, `4498 unique`, `17143 duplicates`, `mode_effective=ram`, `extract_ms=13`
      - `ram_preserve`, `3 ms`, `21641 tokens`, `4498 unique`, `17143 duplicates`, `mode_effective=ram`, `extract_ms=14`
    - `two_huge_partial_overlap`
      - `auto_preserve`, `3496 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=disk`
      - `disk_alpha_global`, `11143 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=disk`
      - `disk_preserve`, `3352 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=disk`
      - `ram_preserve`, `6082 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=ram`
      - `ram_unordered`, `6976 ms`, `12657531 tokens`, `10041269 unique`, `2616262 duplicates`, `mode_effective=ram`
- Validations:
  - passed
  - `rich_duplicate_pair` / `count_parity`: passed
  - `rich_duplicate_pair` / `pair_collapses_duplicates`: passed
  - `rich_duplicate_pair` / `pair_collapses_duplicates`: passed
  - `rich_duplicate_pair` / `pair_collapses_duplicates`: passed
  - `rich_mixed` / `rich_extraction_stage_present`: passed
  - `rich_mixed` / `rich_extraction_stage_present`: passed
  - `rich_mixed` / `rich_extraction_stage_present`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `single_huge_unique` / `unique_equals_tokens_seen`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_dictionary` / `non_empty_output`: passed
  - `small_epub` / `rich_smoke_stage_present`: passed
  - `small_epub` / `rich_smoke_stage_present`: passed
  - `two_huge_partial_overlap` / `ram_disk_auto_count_parity`: passed
- Notes:
  - artifact timestamp: `20260313-132027`
  - `single_huge_unique` improved again in RAM mode: `ram_preserve` moved from `2066 ms` to `1907 ms`
  - `two_huge_partial_overlap` stayed within the phase-1 target: `disk_preserve = 3352 ms`, `auto_preserve = 3496 ms`
  - the latest run favors keeping the direct-insert RAM change, but future tuning should watch the tradeoff between unique-heavy RAM workloads and duplicate-heavy multi-file workloads
- Follow-up:
  - next iteration should tune RAM-store insertion more selectively so `single_huge_unique` keeps the gain without giving back time on the two-file overlap case

