# Release Operations & Incident Log

> Part of the [Documentation Index](00_FINAL_DOCUMENTATION_INDEX.md).
> For product architecture see [03_FINAL_UI_TAURI_SPECIFICATION.md](03_FINAL_UI_TAURI_SPECIFICATION.md).
> For individual release notes see [releases/](releases/).

---

## 1. Release Procedure (Correct Process)

Always use the release script. Never bump versions or create tags manually.

```bash
# 1. Make sure main is clean and up-to-date
git checkout main && git pull

# 2. Bump all versions, build, test, commit, tag, and push in one step
node scripts/release/prepare-release.mjs <X.Y.Z> --commit --tag --push
```

The script (`scripts/release/prepare-release.mjs`) handles:
- Bumping version in **all** versioned files (see table below)
- Rebuilding the frontend
- Running `cargo test --workspace`
- Creating the release commit and annotated git tag
- Pushing both to `origin/main`

The CI pipeline (`desktop-release.yml`) triggers on the tag and:
1. Verifies version coherence across all files (`scripts/ci/verify-release-consistency.sh`)
2. Runs `configure-updater.mjs` to write the correct `plugins.updater` block into `tauri.conf.json`
3. Builds signed Windows and macOS installers
4. Creates the GitHub Release and uploads artifacts

### Files that must all carry the same version

| File | Field |
|---|---|
| `apps/desktop/package.json` | `version` |
| `apps/desktop/package-lock.json` | `version` (top-level) |
| `apps/desktop/src-tauri/tauri.conf.json` | `version` |
| `apps/desktop/src-tauri/Cargo.toml` | `version` |
| `crates/core/Cargo.toml` | `version` |
| `crates/job_runner/Cargo.toml` | `version` |
| `crates/backend/Cargo.toml` | `version` |
| `apps/cli/Cargo.toml` | `version` |

If any file is out of sync the `verify-release-consistency.sh` CI check blocks the build immediately.

---

## 2. Updater Configuration Contract

The CI script `scripts/release/configure-updater.mjs` rewrites `plugins.updater` in `tauri.conf.json` before every build. The value it writes depends on the CI secrets available:

| Secrets present | Written config | Updater behavior |
|---|---|---|
| `TAURI_UPDATER_PUBKEY` **and** `TAURI_SIGNING_PRIVATE_KEY` | `{ active: true, endpoints: [...], pubkey: "<key>" }` | Fully enabled |
| Either secret missing | `{ active: false, pubkey: "" }` | Disabled; no update checks |

**Critical:** `pubkey` must always be present. `tauri-plugin-updater ≥ 2.10.0` deserializes the config into a Rust struct where `pubkey: String` has no `#[serde(default)]`. Omitting the field causes a fatal startup panic even when `active: false`.

> See incident [INC-002](#inc-002--v136-startup-crash-missing-pubkey-field) and [INC-001](#inc-001--v135-startup-crash-null-updater-config) below.

---

## 3. Desktop Cargo.lock Policy

`apps/desktop/src-tauri/Cargo.lock` **must be committed** and kept up-to-date.

- It pins every transitive Rust dependency to an exact version.
- Without it, each CI build resolves whatever is latest on crates.io, meaning two consecutive builds of the same tag can produce different binaries.
- The workspace root `Cargo.lock` covers only CLI/backend crates; the desktop is a separate Cargo workspace and has its own lock file.

**To refresh the lock file** (e.g., after bumping a dependency in `Cargo.toml`):

```bash
# Option A: trigger the workflow manually from GitHub Actions UI
# Workflow: .github/workflows/pin-desktop-lockfile.yml

# Option B: locally if Rust is available
cd apps/desktop/src-tauri
cargo generate-lockfile
git add Cargo.lock
git commit -m "chore(deps): refresh desktop Cargo.lock"
```

> See incident [INC-003](#inc-003--desktop-cargolock-not-committed) below.

---

## 4. Incident Log

### INC-001 — v1.3.5 Startup Crash: Null Updater Config

**Affected version:** v1.3.5
**Symptom:** Application installed successfully but never opened. No error dialog.
**Root cause:** `tauri-plugin-updater` was updated to 2.10.0 in the v1.3.5 build (Cargo.lock was not committed, so CI resolved the latest available version). Version 2.10.0 introduced a stricter startup check: it panics if `plugins.updater` is `null` or absent in `tauri.conf.json`. The CI build script at the time omitted the `plugins.updater` section entirely when signing keys were not configured. Because the desktop binary uses `windows_subsystem = "windows"`, the panic produced no visible output — the application simply did not appear.

**Panic logged internally:**
```
failed to run tauri app: PluginInitialization("updater",
"Error deserializing 'plugins.updater' within your Tauri configuration:
invalid type: null, expected struct Config")
```

**Fix applied in v1.3.6:** `configure-updater.mjs` was updated to always write a `plugins.updater` object. When signing keys are absent it writes `{ "active": false }`.

**Why v1.3.6 was still broken:** See INC-002.

---

### INC-002 — v1.3.6 Startup Crash: Missing `pubkey` Field

**Affected version:** v1.3.6
**Symptom:** Identical to INC-001 — application installed but never opened.
**Root cause:** The fix in v1.3.6 wrote `{ "active": false }` as the disabled updater config. This satisfied the "non-null object" requirement but not the full deserialization contract. In `tauri-plugin-updater` 2.10.0 the configuration struct is defined in Rust as:

```rust
pub struct Config {
    pub active: bool,
    pub pubkey: String,   // required — no #[serde(default)]
    pub endpoints: Vec<Url>,
    // ...
}
```

`serde` requires every field without a default to be present in the JSON. `{ "active": false }` is missing `pubkey`, which causes:

```
failed to run tauri app: PluginInitialization("updater",
"Error deserializing 'plugins.updater' within your Tauri configuration:
missing field `pubkey`")
```

Again, the panic is invisible to the end user because of `windows_subsystem = "windows"`.

**Diagnosis method:** Run the installed `.exe` directly from a terminal (not by double-clicking) to capture stdout/stderr:
```
C:\Users\<user>\AppData\Local\Dupli-Annihilator-G\dedupe_desktop_tauri.exe
```

**Fix applied in v1.3.7:** `configure-updater.mjs` now writes `{ "active": false, "pubkey": "" }` when disabled. An empty string satisfies the `String` deserializer. When `active` is `false` the plugin performs no update checks and never validates the key.

**Relevant file:** [`scripts/release/configure-updater.mjs`](../scripts/release/configure-updater.mjs)

---

### INC-003 — Desktop Cargo.lock Not Committed

**Discovered during:** Investigation of INC-001.
**Root cause:** `apps/desktop/src-tauri/Cargo.lock` was listed in `.gitignore`. Every CI build resolved dependency versions fresh from crates.io. This is what allowed `tauri-plugin-updater` to silently jump from a working version to 2.10.0 between releases without any explicit change in `Cargo.toml`.

**Fix applied:**
1. Removed `apps/desktop/src-tauri/Cargo.lock` from `.gitignore`.
2. Added workflow `pin-desktop-lockfile.yml` to generate and commit the lock file on demand.
3. Generated and committed the initial lock file (pinning `tauri-plugin-updater` at `2.10.0` and all other transitive dependencies).

**Relevant files:**
- [`.github/workflows/pin-desktop-lockfile.yml`](../.github/workflows/pin-desktop-lockfile.yml)
- [`apps/desktop/src-tauri/Cargo.lock`](../apps/desktop/src-tauri/Cargo.lock)

---

### INC-004 — v1.3.7 Release: Incomplete Version Bump

**Affected version:** v1.3.7 (first attempt)
**Symptom:** CI `verify-release-consistency.sh` blocked the build immediately with:
```
Version mismatch: crates/job_runner/Cargo.toml=1.3.6, expected 1.3.7
```

**Root cause:** The version was bumped manually without using `prepare-release.mjs`. Only the desktop files were updated; the backend crates (`crates/core`, `crates/job_runner`, `crates/backend`, `apps/cli`) were missed.

**Fix applied:** Bumped all crate `Cargo.toml` versions to `1.3.7`, updated `apps/desktop/package-lock.json`, and updated the workspace root `Cargo.lock`. Re-tagged and pushed.

**Prevention:** Always use `node scripts/release/prepare-release.mjs <X.Y.Z> --commit --tag --push`. The script bumps all files atomically via `scripts/release/bump-version.mjs` and will not create the commit unless every file is consistent.

---

## 5. Diagnosing Silent Startup Crashes (Windows)

Because the release binary suppresses the console window, panics and startup errors are invisible when launching via a shortcut or the Start menu.

**To capture the error:**

1. Open **Windows Terminal** or **Command Prompt**
2. Run the installed binary directly:
   ```
   "C:\Users\<your-username>\AppData\Local\Dupli-Annihilator-G\dedupe_desktop_tauri.exe"
   ```
3. The panic message will appear in the terminal before the process exits.

**For Rust backtraces** (more detail):
```
set RUST_BACKTRACE=1
"C:\Users\<your-username>\AppData\Local\Dupli-Annihilator-G\dedupe_desktop_tauri.exe"
```

This technique was used to diagnose INC-001 and INC-002.

---

*Document version: 1.0 — 2026-03-10*
