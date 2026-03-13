<p align="center">
  <img src="apps/desktop/src-tauri/icons/icon.png" alt="Dupli-Annihilator-G" width="128" />
</p>

<h1 align="center">Dupli-Annihilator-G</h1>

<p align="center">
  <strong>Rust-powered deduplication and text-set toolkit for large file collections.</strong>
</p>

<p align="center">
  Desktop app: Tauri 2 + React &bull; Engine: Rust &bull; Platforms: Windows, macOS, Linux
</p>

---

## What It Does

**Dupli-Annihilator-G** started as a high-performance duplicate remover for large text corpora. It is now a broader desktop and CLI toolkit for:

- exact deduplication across one or many files,
- rich-input extraction from `PDF` and `EPUB`,
- frequency analysis,
- fuzzy clustering for near-duplicates,
- relational set operations between file groups,
- fast word membership checks against loaded wordlists.

The Rust core is built to handle both small and very large datasets without turning the UI into a guessing game. Jobs expose live stage progress, throughput, ETA, and a final diagnostic summary.

---

## Core Capabilities

| Area | What you get |
|---|---|
| **Exact deduplication** | Merge one or many inputs into one duplicate-free output. |
| **Execution modes** | `ram`, `disk`, and `auto` mode selection. |
| **Ordering modes** | `preserve_first_seen`, `alphabetical`, `unordered_fast`. |
| **Disk alphabetical strategies** | `fast_bucket_local` for speed or `global_perfect` for strict global sort order. |
| **Normalization controls** | `trim`, `drop_empty`, and character-length filtering with `drop_length_min` / `drop_length_max`. |
| **Rich inputs** | Direct processing of `PDF` and `EPUB` files through an extraction stage. |
| **Folder ingestion** | Add a folder and expand supported files recursively. |
| **Output control** | Newline, CRLF, tab, comma, semicolon, pipe, or any custom separator. |
| **Mission Report** | End-of-run summary with metrics, warnings, stage timing, JSON export, and copy/open actions. |
| **Cancellation** | Running jobs can be canceled cleanly. |
| **Updater flow** | In-app update check/install flow with GitHub Releases fallback when needed. |
| **Localization** | 10 desktop UI languages. |

---

## Desktop Toolkit

The desktop app is no longer just a single dedupe screen. It includes several tools backed by the same Rust engine.

### 1. Duplicate Removal

- Add files or folders with picker or drag-and-drop.
- Run exact, case-sensitive deduplication across the entire input set.
- Choose output ordering and execution mode.
- Tune disk processing with bucket count and run size when operating on very large inputs.

### 2. Word Checker

- Load a wordlist from a text-based file.
- Check whether a specific token exists in the loaded set.
- O(1) membership lookup after the wordlist is loaded.

### 3. Frequency Analysis

- Scan one or many inputs and return the most frequent tokens.
- Supports `top N` limiting.
- Useful for corpus inspection before or after cleanup.

### 4. Fuzzy Cluster

- Groups near-duplicate tokens using edit distance.
- Good for typos, variant spellings, and noisy datasets.
- Exposed in the desktop app as a separate output-producing tool.

### 5. Set Operations

- `A - B`
- `intersect(A, B)`
- `union(A, B)`

This makes the app useful not only for deduplication, but also for dataset comparison and corpus curation.

---

## Supported Inputs

### Direct text-like inputs

- `txt`
- `csv`
- `tsv`
- `log`

### Rich inputs

- `pdf`
- `epub`

Rich inputs are extracted into temporary text before tokenization. Recent engine changes improved this path substantially:

- extraction progress is visible immediately,
- `PDF` and `EPUB` extraction runs with bounded parallelism,
- large PDFs are streamed page by page to reduce memory pressure,
- `AUTO` mode chooses between `RAM` and `DISK` after extraction using host memory telemetry plus a corpus sample, not just the compressed container size.

### Folder inputs

When a folder is selected, the backend recursively expands compatible files inside it.

---

## Tokenization and Matching Rules

- Tokens are split by whitespace, comma `,`, and semicolon `;`.
- Matching is exact and case-sensitive.
- `Apple`, `apple`, and `APPLE` are three different tokens.
- `trim` removes leading/trailing whitespace before deduplication.
- `drop_empty` skips empty tokens after normalization.
- Length filtering drops tokens whose character count falls inside an inclusive `[min, max]` range.
- Output is written using the separator you choose, with no trailing separator appended at the end.

---

## Why It Scales

### RAM mode

Fastest path when the unique-token set fits comfortably in memory.

### DISK mode

For large workloads, the engine avoids memory blowups by switching to bounded on-disk strategies:

- bucket partitioning for large unsorted or locally sorted workloads,
- external merge sort when strict global alphabetical output is required.

### AUTO mode

Lets the engine choose the effective mode based on:

- the resolved post-extraction input footprint,
- current available memory on the host,
- a lightweight token sample that estimates uniqueness and duplicate pressure,
- the workload shape (`1` file vs `2+` files with partial overlap).

The same corpus may resolve to different effective modes on different machines if available memory differs.

### Runtime telemetry

The desktop app reports:

- current stage,
- file progress,
- rich-input extraction progress,
- current input path,
- tokens seen,
- unique tokens,
- duplicates,
- throughput,
- elapsed time,
- ETA.

---

## Mission Report

Every completed run ends with a detailed summary screen that can:

- show unique count, duplicate count, reduction ratio, elapsed time, and throughput,
- display stage timings and warnings,
- open the output file or its folder,
- copy a text report to the clipboard,
- export the full summary as JSON,
- reset the app for a new unrelated task,
- run the same job again.

Optional per-file breakdown is also available in `RAM` mode, including:

- source path,
- file size,
- tokens seen,
- duplicates,
- unique contributions,
- tokens filtered by length.

---

## CLI

This repository also ships a CLI app for scripted or headless workflows.

### Current CLI features

- multiple `--input` paths,
- file or folder inputs,
- `--mode auto|ram|disk`,
- `--ordering preserve-first-seen|alphabetical|unordered-fast`,
- `--disk-alphabetical-mode fast-bucket-local|global-perfect`,
- custom separators with escaped or raw handling,
- `--trim`, `--drop-empty`,
- `--drop-length-min`, `--drop-length-max`,
- disk tuning via `--disk-buckets` and `--disk-run-size`,
- `--benchmark-json` for machine-readable summary output,
- `Ctrl+C` cancellation support.

### Example

```bash
cargo run -p dedupe_cli -- \
  --input ./data/a.csv ./data/b.pdf \
  --output ./out/ready.txt \
  --mode auto \
  --ordering alphabetical \
  --disk-alphabetical-mode global-perfect \
  --separator "\n" \
  --trim true \
  --drop-empty true
```

Show full CLI help:

```bash
cargo run -p dedupe_cli -- --help
```

---

## Desktop Feature Snapshot

| Feature | Desktop | CLI |
|---|---|---|
| Exact dedupe pipeline | Yes | Yes |
| Folder expansion | Yes | Yes |
| `PDF` / `EPUB` ingestion | Yes | Yes |
| Live progress + stage telemetry | Yes | Yes |
| Mission Report UI | Yes | No |
| Word Checker | Yes | No |
| Frequency Analysis | Yes | No |
| Fuzzy Cluster | Yes | No |
| Set Operations | Yes | No |
| In-app updater | Yes | No |
| Localization | Yes | No |

---

## Project Layout

```text
Dupli-Annihilator-G/
|-- crates/
|   |-- core/         Deduplication engine, rich-input readers, analysis helpers
|   |-- job_runner/   Background job orchestration and event streaming
|   `-- backend/      API layer used by desktop and CLI surfaces
|-- apps/
|   |-- cli/          Command-line interface
|   `-- desktop/      Tauri desktop app with React frontend
|-- docs/             Specifications, release notes, operations docs
`-- scripts/          Release, CI, and benchmarking helpers
```

---

## Tech Stack

| Layer | Technology |
|---|---|
| Core engine | Rust 2021 |
| Desktop shell | Tauri 2 |
| Frontend | React 18 + TypeScript |
| Build tool | Vite |
| CLI parsing | clap |
| Serialization | serde / serde_json |
| Hashing/data structures | ahash, hashbrown, indexmap |

---

## Language Support

The desktop UI currently ships with 10 locales:

- English (`en`)
- Spanish (`es`)
- French (`fr`)
- Portuguese (`pt`)
- Chinese Simplified (`zh-CN`)
- Hindi (`hi`)
- Arabic (`ar`)
- Bengali (`bn`)
- Russian (`ru`)
- Urdu (`ur`)

---

## Quick Start

### End users

1. Download the latest desktop release from [GitHub Releases](../../releases).
2. Install the platform-native package:
   Windows: `-setup.exe`
   macOS: `.dmg`
   Linux: `.AppImage` or `.deb`
3. Add files or a folder.
4. Choose an output path.
5. Run the job and review the Mission Report.

### Developers

Requirements:

- Rust stable
- Node.js 20+
- npm

Run workspace tests:

```bash
cargo test --workspace
```

Real-corpus benchmark harness:

```bash
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -ListScenarios
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -ValidateCorpus -RequireCorpus
pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1 -Suite smoke-real
```

When `testfiles/` is present locally, the benchmark harness treats it as the canonical real corpus. The current scenarios expect large text corpora such as `Test1.csv`, `Test2.csv`, `Test3.csv`, a dense wordlist such as `spanish.txt`, and rich inputs such as local `.pdf` and `.epub` files. These `.csv` fixtures are benchmark text corpora and are not assumed to be tabular CSV datasets.

See [`docs/benchmarks.md`](docs/benchmarks.md) for the scenario matrix, baseline numbers, and emitted JSON/CSV artifacts.

Install desktop dependencies:

```bash
npm ci --prefix apps/desktop
```

Run the desktop app in dev mode:

```bash
cargo install tauri-cli --version "2.10.1" --locked
cd apps/desktop/src-tauri
cargo tauri dev --ci
```

Build the frontend bundle only:

```bash
npm --prefix apps/desktop run build
```

---

## Local Packaging

Artifacts are generated under `apps/desktop/src-tauri/target/release/bundle`.

### Windows

```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "2.10.1" --locked
cd apps/desktop/src-tauri
cargo tauri build --ci --bundles nsis --no-sign
```

### macOS

```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "2.10.1" --locked
cd apps/desktop/src-tauri
cargo tauri build --ci --bundles dmg --no-sign
```

### Linux

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
npm ci --prefix apps/desktop
cargo install tauri-cli --version "2.10.1" --locked
cd apps/desktop/src-tauri
cargo tauri build --ci --no-sign
```

---

## Releases and Docs

- Release notes live in `docs/releases/`.
- Product and engine documentation live in `docs/`.
- Release automation helpers live in `scripts/release/`.
- CI verification helpers live in `scripts/ci/`.
- Real-corpus benchmark helpers live in `scripts/bench/`.

If you are preparing a release, review:

- `docs/07_RELEASE_OPERATIONS.md`
- `docs/releases/TEMPLATE.md`
- `scripts/release/bump-version.mjs`
- `scripts/release/prepare-release.mjs`

---

## Licensing

This project is licensed under the **[PolyForm Small Business License 1.0.0](https://polyformproject.org/licenses/small-business/1.0.0/)**.

- Free for personal use and for organizations that qualify as a small business under the license.
- A commercial license is required for organizations that do not qualify.

See:

- [`LICENSE`](LICENSE)
- [`COMMERCIAL_LICENSE.md`](COMMERCIAL_LICENSE.md)
- [`NOTICE`](NOTICE)

---

<p align="center">
  <strong>Principal Author:</strong> Giuseppe Rojas
</p>
