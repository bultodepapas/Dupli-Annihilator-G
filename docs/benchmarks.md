# Real Corpus Benchmarks

## Purpose

This document defines the local real-world corpus used to benchmark and regression-check the dedupe engine outside the normal PR test loop.

The benchmark harness lives at:

```powershell
scripts/bench/run-real-corpus.ps1
```

The harness is intentionally built around `testfiles/`, which remains a local corpus directory. In normal CI checkouts this directory may be absent; CI only verifies that the harness can list scenarios and validates the corpus only when `testfiles/` is present.

Historical benchmark runs and baseline evolution are tracked in [`docs/benchmark-history.md`](./benchmark-history.md).

## Canonical Local Corpus

These files are the current benchmark targets when they exist under `testfiles/`:

| Scenario Key | Files | Purpose |
|---|---|---|
| `single_huge_unique` | `Test2.csv` | Worst-case RAM store pressure and bucketing cost with almost no duplicate wins. |
| `two_huge_partial_overlap` | `Test1.csv` + `Test3.csv` | Primary benchmark for 2 large files with partial overlap. |
| `small_dictionary` | `spanish.txt` | Fast smoke corpus for exactness and quick regressions. |
| `rich_duplicate_pair` | `*44638b230c0a6734a3c32b66b2ba0ed0*.pdf` x2 | Two byte-identical PDFs that isolate extraction plus overlap-100% dedupe. |
| `rich_mixed` | `*a3b22e2d4c54ee67642136dd94f3f5ca*.pdf` + `Dune.epub` | Mixed rich-input workload for extraction and `AUTO` validation. |
| `small_epub` | `La metamorfosis.epub` | Small rich-input smoke corpus. |

Notes:

- The `.csv` files in `testfiles/` are treated as text corpora, not as guaranteed tabular CSV fixtures.
- The Biology PDF pair is validated by SHA-256 and must remain byte-identical for the `rich_duplicate_pair` scenario.

## Suites

The harness groups scenarios by cost:

| Suite | Scenarios | Intended Use |
|---|---|---|
| `smoke-real` | `small_dictionary`, `small_epub` | Fast local validation after engine changes. |
| `bench-real-core` | `single_huge_unique`, `two_huge_partial_overlap` | Core engine throughput and RAM/DISK/AUTO comparisons. |
| `bench-real-rich` | `rich_duplicate_pair`, `rich_mixed` | Rich-input extraction plus dedupe behavior. |

## Commands

List scenarios:

```powershell
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -ListScenarios
```

Validate the local corpus:

```powershell
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -ValidateCorpus -RequireCorpus
```

Run only the smoke suite:

```powershell
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite smoke-real
```

Run the main large-file suite and write outputs under `target/real-corpus-bench`:

```powershell
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite bench-real-core -OutputDir target/real-corpus-bench
```

Use an existing CLI binary without rebuilding:

```powershell
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite bench-real-rich -SkipBuild
```

Append the generated Markdown entry directly to the history log:

```powershell
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite smoke-real -SkipBuild -AppendHistory
```

To append into a different history file:

```powershell
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite smoke-real -SkipBuild -AppendHistory -HistoryPath docs/benchmark-history.md
```

## Output Artifacts

Each run writes:

- `results-<timestamp>.json`
- `results-<timestamp>.csv`
- `validations-<timestamp>.json`
- `validations-<timestamp>.csv`
- `history-entry-<timestamp>.md`
- `latest-results.json`
- `latest-results.csv`
- `latest-validations.json`
- `latest-validations.csv`
- `latest-history-entry.md`

The history-entry artifacts are Markdown blocks generated from the current run and meant to be pasted into [`docs/benchmark-history.md`](./benchmark-history.md) after adding any missing human context.

Each result record includes:

- scenario and suite
- input names and total input bytes
- `tokens_seen`
- `unique_tokens`
- `duplicates`
- `elapsed_ms`
- requested mode and effective mode
- requested ordering and effective ordering
- disk settings
- `extract_ms` and `core_pipeline_ms`
- `AUTO` decision telemetry (`auto_*` fields) when `mode=auto`
- stage durations serialized as JSON

## Baseline Numbers

These are the current local baselines gathered on `2026-03-13` from the latest full historical run:

| Scenario | Variant | Baseline |
|---|---|---|
| `single_huge_unique` | `AUTO + PreserveFirstSeen` | `1829 ms` |
| `single_huge_unique` | `RAM + PreserveFirstSeen` | `1907 ms` |
| `single_huge_unique` | `DISK + PreserveFirstSeen` | `1587 ms` |
| `two_huge_partial_overlap` | `DISK + PreserveFirstSeen` | `3352 ms` |
| `two_huge_partial_overlap` | `RAM + PreserveFirstSeen` | `6082 ms` |
| `two_huge_partial_overlap` | `AUTO + PreserveFirstSeen` | `3496 ms` |
| `rich_duplicate_pair` | pair of Biology PDFs | `644,158 tokens`, `33,058 unique`, `611,100 duplicates` |

These are regression anchors, not portable universal benchmarks. Hardware, filesystem, antivirus, and OS effects still matter.

## Acceptance Focus

Phase-1 optimization work should be judged against the real corpus first:

- `Test2.csv`: `RAM + PreserveFirstSeen` should not remain more than 10% slower than `DISK + PreserveFirstSeen`; otherwise `AUTO` should favor DISK for this profile.
- `Test1.csv + Test3.csv`: DISK-path improvements should target at least 15% additional gain.
- Rich-input analysis should track `ExtractingText` separately from the rest of the pipeline so extraction wins and dedupe wins are not conflated.
- `AUTO` changes must be evaluated with the new `auto_*` decision telemetry, not only by `mode_effective`.
