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
| **Cross-platform** | Native installers for Windows (`-setup.exe`), macOS (`.dmg`), and Linux (`.AppImage` / `.deb`). Signed updater artifacts are published on supported release lanes. |
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
   Windows: prefer `-setup.exe`.
   macOS: prefer `.dmg`.
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

For manual validation, keep local sample files in `testfiles/`. This folder is intentionally gitignored so you can store different fixture types there without uploading them to GitHub, for example `.csv`, `.epub`, and `.txt` files used to exercise duplicate-detection scenarios.

Run the desktop app in dev mode:
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "2.10.1" --locked
cd apps/desktop/src-tauri
cargo tauri dev --ci
```

---

## Build Installers

Use these commands when you want to validate a local desktop bundle before cutting a release.

**Windows** (run on Windows):
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "2.10.1" --locked
cd apps/desktop/src-tauri
cargo tauri build --ci --bundles nsis --no-sign
```

**macOS** (run on macOS):
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "2.10.1" --locked
cd apps/desktop/src-tauri
cargo tauri build --ci --bundles dmg --no-sign
```

**Linux** (run on Linux):
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

Artifacts are generated under `apps/desktop/src-tauri/target/release/bundle`.

---

## CI/CD

GitHub Actions workflows handle continuous integration and release publishing:

- **CI** — `.github/workflows/ci.yml`
- **Release** — `.github/workflows/desktop-release.yml`

### Desktop Release Runbook

This repository publishes desktop releases through the `desktop-release` GitHub Actions workflow.

The workflow is triggered by:

- pushing a tag that matches `v*`
- manually running `desktop-release` with `workflow_dispatch` and an existing tag

The workflow performs these stages:

1. `verify-release`
2. `build-desktop` on `windows-latest`, `macos-latest`, and `ubuntu-22.04`
3. `publish-release`

The GitHub Release is only published if **all three platform builds succeed**.

### What The Workflow Verifies

Before any installer is built, CI verifies:

- the release tag is strict semver in the form `vX.Y.Z`
- every release-managed manifest has the same version
- the tag points to a commit reachable from `origin/main`
- tagged desktop releases include the signing credentials required for updater-capable bundles
- tagged desktop releases emit the updater manifests and signatures required by the public release contract

This protects against the two most common release mistakes:

- tagging a version that does not match the repository manifests
- tagging a commit that is not actually on the main release line

### Release-Managed Version Files

These files must stay version-aligned for every release:

- `apps/desktop/package.json`
- `apps/desktop/package-lock.json`
- `apps/desktop/src-tauri/tauri.conf.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `crates/core/Cargo.toml`
- `crates/job_runner/Cargo.toml`
- `crates/backend/Cargo.toml`
- `apps/cli/Cargo.toml`

In practice, also review:

- `apps/desktop/src-tauri/Cargo.lock`
- `docs/releases/vX.Y.Z.md`
- `README.md`

Do not assume the helper script staged every release-relevant file automatically. Always inspect `git status` before committing the release.

### Recommended Release Procedure

Use this procedure for normal releases. It is intentionally conservative and optimized for correctness over speed.

#### 1. Start From A Clean `main`

```bash
git checkout main
git pull origin main
git status --short
```

Requirements before continuing:

- working tree is clean
- all intended feature/fix commits are already on `main`
- no unreviewed local changes are mixed into the release

#### 2. Choose The Next Version Carefully

Use strict semver:

- patch: small fixes, UI polish, internal corrections
- minor: new user-facing capabilities without breaking compatibility
- major: breaking changes or explicit release-line reset

Never reuse or move an existing release tag.

If a release fails after pushing a tag, do **not** retag the same version to a different commit. Fix the issue and publish the next patch version instead.

#### 3. Dry-Run The Version Bump First

```bash
node scripts/release/bump-version.mjs X.Y.Z --dry-run
```

This confirms:

- the version format is valid
- the target files are the expected ones
- you are about to bump the correct release line

#### 4. Run The Real Version Bump

```bash
node scripts/release/bump-version.mjs X.Y.Z
```

This updates the managed manifest versions and keeps the repository version-coherent.

#### 5. Refresh The Desktop Lockfile

```bash
npm --prefix apps/desktop install --package-lock-only
```

This ensures the frontend lockfile stays aligned with the new desktop app version metadata.

#### 6. Add Release Notes

Create:

```text
docs/releases/vX.Y.Z.md
```

Release notes should explain:

- what changed
- why it matters
- compatibility expectations
- any operational caveats

If this file is missing, the workflow falls back to `docs/releases/TEMPLATE.md`, which is acceptable for emergency recovery but not for a polished release.

#### 7. Run Local Preflight Checks

Recommended minimum checks:

```bash
npm --prefix apps/desktop run build
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
```

Recommended broader checks when the workspace is healthy:

```bash
cargo test --workspace --locked || cargo test --workspace
```

Important operational note:

- the release workflow builds the desktop Tauri application on all three runners
- a successful frontend build alone is **not** enough
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` is the fastest useful local guard against stale Tauri/backend integration drift

If workspace-wide tests are already failing for an unrelated, known reason, document that explicitly before releasing. Do not silently ignore red checks.

#### 8. Review The Final Diff

Before committing the release, inspect:

```bash
git status --short
git diff --stat
```

Confirm that the release commit includes:

- the version bumps
- the release notes file
- any lockfile updates required by the desktop/Tauri validation path

#### 9. Create The Release Commit

```bash
git add -A
git commit -m "chore(release): prepare vX.Y.Z"
```

Operational rule:

- create the release commit **before** doing anything with the tag
- do not create the tag in parallel with the commit
- if the tag is created before the final release commit exists locally, it can point to the previous release commit and CI will fail with `Tag/version mismatch`

#### 10. Tag The Release

```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z"
```

Immediately verify the tag target before pushing it:

```bash
git show --no-patch --pretty=fuller vX.Y.Z
```

The tag must point to the exact `chore(release): prepare vX.Y.Z` commit you just created.

If it points anywhere else:

- do **not** push the tag
- delete the local tag and recreate it correctly:

```bash
git tag -d vX.Y.Z
git tag -a vX.Y.Z -m "Release vX.Y.Z"
```

#### 11. Push `main` First, Then Push The Tag

```bash
git push origin main
git push origin vX.Y.Z
```

Pushing the tag triggers the release workflow and starts the cross-platform bundle build.

Operational rule:

- never push the tag first
- never assume the tag points to `HEAD` without checking
- `main` and the tag must reference the same release commit

### Release Mistakes We Already Made

These are real failure modes that already happened during release prep and should be treated as hard rules, not suggestions.

#### Mistake 1: Tag Created Before The Final Release Commit

What happened:

- the tag was created while the new release commit had not been finalized yet
- the pushed tag pointed to the previous release commit
- CI failed immediately in `verify-release` with:

```text
Tag/version mismatch: tag=vX.Y.Z, expected=vA.B.C
```

How to avoid it:

- always commit first, tag second
- always run `git show --no-patch --pretty=fuller vX.Y.Z` before pushing the tag
- if CI fails this way after the tag is already pushed, cut the **next patch version**; do not move the old tag

#### Mistake 2: Commit And Tag Were Effectively Done In Parallel

What happened:

- release steps were executed out of order
- even though both commit and tag existed locally, the tag still captured the old commit

How to avoid it:

- treat release prep as a strict sequence:
  1. version bump
  2. lockfile refresh
  3. release notes
  4. local checks
  5. `git add -A`
  6. `git commit`
  7. `git tag`
  8. verify tag target
  9. push `main`
  10. push tag

#### Mistake 3: Assuming Local Validation Was The Same As CI Validation

What happened:

- local build/test checks passed
- release still failed because CI also validates tag/version coherence against the exact tagged commit

How to avoid it:

- do not stop at `cargo test` / `npm build`
- explicitly verify:
  - manifest versions are aligned
  - the tag matches the manifest version
  - the tag points to the intended release commit
  - the tagged commit is already on `origin/main`

#### Mistake 4: Reusing A Broken Release Version

What happened:

- once a bad tag was pushed, that version was no longer safe to reuse

How to avoid it:

- if a pushed release tag is wrong, do **not** retag that same version to a different commit
- fix the issue and publish the next patch version instead

#### Mistake 5: Running Bash Release Checks From The Wrong Shell Path

What happened:

- a bash-based verification command was invoked with a path that did not exist in the current shell environment

How to avoid it:

- run shell-script checks from the repository root
- do not guess path translations between PowerShell, Git Bash, and WSL
- verify the current working directory and shell path semantics before relying on local script output

### Release Helpers

You may use the helper scripts to automate part of the process:

```bash
# Dry run version bump
node scripts/release/bump-version.mjs X.Y.Z --dry-run

# Release preparation helper
node scripts/release/prepare-release.mjs X.Y.Z --commit --tag

# Full helper-driven path
node scripts/release/prepare-release.mjs X.Y.Z --commit --tag --push
```

Useful flags:

- `--allow-dirty`
- `--skip-build`
- `--skip-tests`
- `--push`

Operational guidance:

- prefer the manual runbook for important releases or when the repo state is unusual
- use the helper when the repo is already clean and the release is routine
- even when using the helper, still review `git status` before pushing
- the helper does not replace operator judgment

### Post-Tag Monitoring

After pushing the tag, monitor the workflow instead of assuming success.

Recommended with GitHub CLI:

```bash
gh run list --workflow desktop-release --limit 5
gh run watch <run-id> --exit-status
gh release view vX.Y.Z
```

What success looks like:

- `verify-release` succeeds
- `build-desktop (windows-latest)` succeeds
- `build-desktop (macos-latest)` succeeds
- `build-desktop (ubuntu-22.04)` succeeds
- `publish-release` succeeds
- the GitHub Release page contains the expected assets

### Failure Handling

If a release workflow fails:

1. Inspect the failed job logs immediately.
2. Identify whether the failure is:
   - version/tag coherence
   - desktop frontend build
   - Tauri/Rust desktop build
   - platform-specific packaging
   - release publication
3. Fix the underlying issue on `main`.
4. Create a **new** patch version.
5. Repeat the release process with the new version.

Do not:

- force-move an existing release tag
- overwrite a published version with different contents
- guess that a failure is “just CI noise” without reading the logs

### Manual Rebuild Of An Existing Tag

If the tag is already correct and you only need to rerun the workflow, use `workflow_dispatch` with the existing tag from GitHub Actions.

This is appropriate only when:

- the tagged commit is correct
- the manifests are correct
- the failure was transient or infrastructure-related

If the tagged commit itself is wrong, cut a new version instead of reusing the old tag.

### Updater Secrets

| Secret / Variable | Required | Description |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Yes | Signs updater artifacts |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Yes | Key password |
| `TAURI_UPDATER_PUBKEY` | Yes | Public key for verification |
| `TAURI_UPDATER_ENDPOINT` | No | Custom endpoint (defaults to GitHub Releases) |
| `WINDOWS_CERTIFICATE` | Yes for Windows releases | Base64-encoded Authenticode `.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD` | Yes for Windows releases | Password for the Windows signing certificate |
| `WINDOWS_CERTIFICATE_THUMBPRINT` | Yes for Windows releases | Thumbprint used by the Tauri bundler signing pass |
| `WINDOWS_TIMESTAMP_URL` | No | RFC3161 timestamp endpoint for Windows signing |
| `APPLE_CERTIFICATE` | Yes for macOS releases | Base64-encoded Apple signing `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Yes for macOS releases | Password for the Apple signing certificate |
| `APPLE_SIGNING_IDENTITY` | Yes for macOS releases | Developer ID Application signing identity |
| `APPLE_API_ISSUER` | Yes for macOS releases | App Store Connect issuer ID for notarization |
| `APPLE_API_KEY` | Yes for macOS releases | App Store Connect key ID for notarization |
| `APPLE_API_KEY_CONTENT` | Yes for macOS releases | Private `.p8` content written to a temporary CI file |
| `KEYCHAIN_PASSWORD` | Yes for macOS releases | Password for the temporary CI keychain |
| `DUPLI_UPDATE_CHANNEL` | No | Channel label (default: `stable`) |

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
