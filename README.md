<p align="center">
  <img src="apps/desktop/src-tauri/icons/icon.png" alt="Dupli-Annihilator-G" width="128" />
</p>

<h1 align="center">Dupli-Annihilator-G</h1>

<p align="center">
  <strong>Eliminate duplicates from massive text files — in seconds.</strong>
</p>

<p align="center">
  Built with Rust &bull; Powered by Tauri 2 &bull; Available on Windows, macOS & Linux
</p>

---

## The Problem

You have text files with thousands — or millions — of lines. Wordlists, email lists, log entries, datasets, CSV exports. Many of those lines are duplicated. Cleaning them manually is impractical, and most tools either crash on large files, are painfully slow, or force you into the command line.

## The Solution

**Dupli-Annihilator-G** is a desktop application that merges one or more text files into a single, clean output with every duplicate removed. Drag your files in, click RUN, and get your deduplicated result. That's it.

It is built entirely on **Rust**, which means it processes data at native speed with minimal memory overhead. Whether your file has 10,000 lines or 10,000,000, the engine handles it without breaking a sweat.

---

## Why Dupli-Annihilator-G?

| Strength | Detail |
|---|---|
| **Blazing fast** | The core engine uses high-performance hash structures (`ahash`, `hashbrown`) — the same building blocks used in production-grade Rust infrastructure. |
| **Handles any file size** | Small files run entirely in RAM. For massive datasets, switch to DISK mode: the engine partitions data into buckets or performs external merge sort, so you're never limited by available memory. |
| **Deterministic results** | Choose your ordering: preserve first-seen order, sort alphabetically, or use unordered mode for maximum throughput. The output is always consistent and reproducible. |
| **Real-time feedback** | Watch progress, throughput (tokens/sec), elapsed time, and ETA update live as the engine works. |
| **Cross-platform** | Native installers for Windows (`.exe` / `.msi`), macOS (`.dmg` / `.app`), and Linux (`.AppImage` / `.deb`). No runtime dependencies. |
| **10 languages** | UI available in English, Spanish, French, Portuguese, Chinese, Hindi, Arabic, Bengali, Russian, and Urdu. |

---

## Key Features

- **Multi-file merge** — Combine as many input files as you need into one deduplicated output.
- **3 ordering modes** — Preserve first-seen order, sort alphabetically, or run unordered for max speed.
- **3 execution modes** — RAM (in-memory), DISK (memory-bounded for huge files), or AUTO.
- **Custom output separators** — Newline, tab, comma, semicolon, or any custom string.
- **Token normalization** — Trim whitespace and drop empty tokens automatically.
- **Case-sensitive deduplication** — `Apple`, `apple`, and `APPLE` are treated as three distinct tokens.
- **Mission Report** — After every run, review a detailed summary with statistics, diagnostics, and timeline. Export it as JSON or copy to clipboard.
- **Drag & Drop** — Drop files directly into the app window.
- **Cancel & retry** — Safely stop a running job and restart with different settings.
- **Built-in updater** — Check for new versions and install updates from within the app.
- **Word Search** — Load any wordlist and instantly check whether a specific word exists in it. O(1) lookup powered by the same high-performance hash engine used for deduplication.

---

## Quick Start

1. Download the latest installer from the [**GitHub Releases**](../../releases) page.
2. Install and open the app.
3. Add one or more input files (file picker or drag & drop).
4. Choose where to save the output.
5. Click **RUN** and wait for **DONE**.

That's all. Your deduplicated file is ready.

---

## How It Works Under the Hood

Dupli-Annihilator-G is built as a layered Rust architecture:

```
┌─────────────────────────────────┐
│      Desktop UI (React)         │  User-facing interface
├─────────────────────────────────┤
│      Tauri 2 Bridge             │  Native OS integration
├─────────────────────────────────┤
│      Backend API                │  Command routing
├─────────────────────────────────┤
│      Job Runner                 │  Orchestration & events
├─────────────────────────────────┤
│      Core Engine (Rust)         │  Deduplication algorithms
└─────────────────────────────────┘
```

### RAM Mode
Tokens are streamed from input files, inserted into a high-speed hash set (`IndexSet` or `HashSet` depending on ordering), and written to output. Ideal for files that fit comfortably in memory.

### DISK Mode — Bucket Partitioning
For large datasets: tokens are hashed and distributed across temporary bucket files. Each bucket is then loaded, deduplicated in memory, and flushed to the final output. Memory stays bounded regardless of input size.

### DISK Mode — External Merge Sort (Alphabetical)
When alphabetical global ordering is needed on huge files: sorted runs are generated, then merged using a k-way merge with a binary heap. Deduplication happens during the merge pass.

### Performance Optimizations
- **AHash** — Non-cryptographic, extremely fast hash function.
- **Lossy UTF-8 reader** — Gracefully handles non-UTF-8 input without crashing.
- **BOM stripping** — Automatically strips byte-order marks.
- **EWMA throughput** — Smoothed tokens/sec metrics for accurate progress reporting.
- **Batched progress updates** — Minimal overhead from UI updates (every 100K tokens).
- **Cancellation checks** — Cooperative cancellation every 8,192 tokens for responsive UX.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Core engine | **Rust** (edition 2021) |
| Desktop framework | **Tauri 2** |
| Frontend | **React 18** + **TypeScript** |
| Build tool | **Vite** |
| Hashing | `ahash`, `hashbrown`, `indexmap` |
| CLI parser | `clap` |
| Serialization | `serde` / `serde_json` |

---

## Project Layout

```
Dupli-Annihilator-G/
├── crates/
│   ├── core/              Core deduplication engine
│   ├── job_runner/         Job orchestration & event streaming
│   └── backend/           API layer (core <-> Tauri)
├── apps/
│   ├── cli/               Command-line interface
│   └── desktop/           Tauri desktop app (React frontend)
├── docs/                  Product & engineering specifications
└── scripts/               Release automation
```

---

## How Deduplication Works

- **Delimiters** — Tokens are split by whitespace, comma `,`, and semicolon `;`.
- **Matching** — Exact, case-sensitive. `Perro`, `perro`, and `PERRO` are three different tokens.
- **Output** — Unique tokens are written with your chosen separator. No trailing separator at the end.

---

## Language Support

The desktop UI is available in 10 languages:

| Language | Code |
|---|---|
| English | `en` |
| Spanish | `es` |
| French | `fr` |
| Portuguese | `pt` |
| Chinese (Simplified) | `zh-CN` |
| Hindi | `hi` |
| Arabic | `ar` |
| Bengali | `bn` |
| Russian | `ru` |
| Urdu | `ur` |

---

## Local Development

**Requirements:** Rust stable toolchain, Node.js 20+, npm

Run tests:
```bash
cargo test --workspace
```

Run the desktop app in dev mode:
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "^2.0" --locked
cd apps/desktop/src-tauri
cargo tauri dev --ci
```

---

## Build Installers

**Windows** (run on Windows):
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "^2.0" --locked
cd apps/desktop/src-tauri
cargo tauri build --ci --no-sign
```

**macOS** (run on macOS):
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "^2.0" --locked
cd apps/desktop/src-tauri
cargo tauri build --ci --no-sign
```

Artifacts are generated under `apps/desktop/src-tauri/target/release/bundle`.

---

## CI/CD

GitHub Actions workflows handle continuous integration and release publishing:

- **CI** — `.github/workflows/ci.yml`
- **Release** — `.github/workflows/desktop-release.yml`

### Release Process

- Push a tag matching `v*` (e.g., `v1.3.3`) to build installers and publish a GitHub Release.
- Release tags must point to a commit reachable from `main`.
- Manual runs supported via `workflow_dispatch`.
- Signed updater artifacts are built automatically when `TAURI_SIGNING_PRIVATE_KEY` is configured.

### Updater Secrets

| Secret / Variable | Required | Description |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Yes | Signs updater artifacts |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Yes | Key password |
| `TAURI_UPDATER_PUBKEY` | Yes | Public key for verification |
| `TAURI_UPDATER_ENDPOINT` | No | Custom endpoint (defaults to GitHub Releases) |
| `DUPLI_UPDATE_CHANNEL` | No | Channel label (default: `stable`) |

### Release Helpers

```bash
# Dry run version bump
node scripts/release/bump-version.mjs 1.3.1 --dry-run

# Full release preparation
node scripts/release/prepare-release.mjs 1.3.1 --commit --tag

# Useful flags: --allow-dirty, --skip-build, --skip-tests, --push
```

---

## Licensing

This project is licensed under the **[PolyForm Small Business License 1.0.0](https://polyformproject.org/licenses/small-business/1.0.0/)**.

- **Free** for personal use and for organizations that qualify as a "Small Business" under the license (fewer than 100 employees/contractors and less than $1M USD annual revenue).
- **Commercial license required** for organizations that do not qualify.

See:
- [`LICENSE`](LICENSE) — Full PolyForm Small Business License 1.0.0 text
- [`COMMERCIAL_LICENSE.md`](COMMERCIAL_LICENSE.md) — How to obtain a commercial license

---

## Documentation

The project includes a comprehensive specification set in `docs/`:

1. `00_FINAL_DOCUMENTATION_INDEX.md` — Document index
2. `01_FINAL_EXECUTIVE_SUMMARY.md` — Executive summary
3. `02_FINAL_ENGINE_SPECIFICATION.md` — Engine specification
4. `03_FINAL_UI_TAURI_SPECIFICATION.md` — UI specification
5. `04_FINAL_PM_IMPLEMENTATION_PLAN.md` — Implementation plan
6. `05_PENDING_DECISIONS.md` — Decision register
7. `06_DOCUMENT_CONTROL.md` — Version control

---

<p align="center">
  <br/>
  If Dupli-Annihilator-G saved you time or made your workflow easier,<br/>
  a <strong>GitHub star</strong> would mean the world to me.<br/><br/>
  It's a small gesture that helps others discover this tool<br/>
  and keeps me motivated to keep improving it.<br/><br/>
  Thank you for using it!
</p>

---

<p align="center">
  <strong>Principal Author:</strong> Giuseppe Rojas
</p>
