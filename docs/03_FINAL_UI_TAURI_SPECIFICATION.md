# UI/Tauri v2 Specification (Final, No Code)

## Related Documents
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/02_FINAL_ENGINE_SPECIFICATION.md`
- `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
- `docs/05_PENDING_DECISIONS.md`

## 1) Desktop Application Objective
A single-screen desktop experience that enables users to:
1. select multiple input files,
2. configure mode, ordering, and output separator,
3. choose output target path and file,
4. run, monitor, and cancel jobs without blocking the UI.

## 2) Recommended Stack (Senior Team Baseline)
- Tauri v2 as desktop shell.
- React + TypeScript frontend.
- Tailwind + headless components for UI system.
- Rust engine for heavy processing; frontend handles orchestration and visualization.

## 3) MUST Functional Requirements

### 3.1 Inputs
- Support N input files (button + drag and drop).
- File list must display name, size, truncated path, and per-item remove action.
- `Clear all` action is required.

### 3.2 Processing Configuration
- `Mode` selector: `AUTO`, `RAM`, `DISK`.
- V1 rule for `AUTO`: behaves as `RAM`, with explicit tooltip.
- `Output Ordering` selector:
  - `PreserveFirstSeen` (default),
  - `Alphabetical`,
  - `UnorderedFast` (advanced).
- If `DISK + Alphabetical`, show sub-options:
  - `Fast (Recommended)` = `FastBucketLocal`,
  - `Global Perfect (Slower)` = `GlobalPerfect`.
- If `DISK + PreserveFirstSeen`, show a strong warning:
  global first-seen order is not guaranteed in V1.

### 3.3 Output Separator
- Must accept arbitrary string separator.
- Must provide common presets.
- Must support custom separator input.
- Must include `Interpret escapes` toggle (default ON).
- Must include output preview visualization.

### 3.4 Output File
- Path and filename selection via Save dialog.
- Must validate non-empty output path.
- Must confirm overwrite when target exists.

### 3.5 Run and Cancel
- Primary CTA states:
  - `RUN`,
  - `CANCEL` (while running),
  - `RUN AGAIN` (completed),
  - `RETRY` (error).
- Visible runtime status states:
  - `Idle`, `Running`, `Finalizing`, `Done`, `Error`, `Canceled`.

## 4) Progress, Telemetry, and ETA

### 4.1 Performance Principle
- No per-token log streaming to UI.
- Only aggregated telemetry and throttled events.

### 4.2 Progress Visualization
- Prominent progress bar.
- Determinate mode when reliable progress basis exists.
- Indeterminate mode otherwise.
- Current stage and detail line are required.

### 4.3 Live Metrics
- `tokens_seen`
- `unique_tokens`
- `duplicates`
- smoothed `throughput (tokens/sec)`
- `elapsed`
- `ETA (approx)` or `-` when unreliable

### 4.4 Reference Stages in UI
- RAM:
  - `ScanningInputs`,
  - `Tokenizing`,
  - `Deduplicating`,
  - `Sorting` (if applicable),
  - `WritingOutput`,
  - `Finalizing`.
- DISK fast:
  - `PartitioningBuckets`,
  - `ReducingBuckets`,
  - `WritingOutput`,
  - `Finalizing`.
- DISK global perfect:
  - `GeneratingRuns`,
  - `MergingRuns`,
  - `WritingOutput`,
  - `Finalizing`.

## 5) Frontend <-> Rust Integration Contract

### 5.1 Commands
- `start_job(config)` returns job identifier.
- `cancel_job(job_id)` requests cancellation.
- `get_app_info()` optional.

### 5.2 Events
- `job://started`
- `job://progress`
- `job://done`
- `job://error`
- `job://canceled`

### 5.3 Minimum Progress Payload Fields
- `jobId`
- `stage`
- `progress01` (best effort)
- `filesDone/filesTotal`
- `tokensSeen/uniqueTokens/duplicates`
- `throughput`
- `elapsed`
- `eta`
- `detail`

## 6) Tauri v2 Security
- Per-window least-privilege capability model.
- Heavy file read/write must run in Rust.
- Frontend file system access limited to dialog and minimal metadata operations.

## 7) Mandatory UI Performance Rules
1. Backend-to-frontend throttling: max 4-10 updates/sec.
2. Frontend state batching/throttling to limit re-render pressure.
3. Avoid large DOM structures (infinite logs, oversized tables).
4. Virtualize lists only when item volume justifies it.

## 8) Final Visual System ("Neon Lab")

### 8.1 Visual Direction
- Dark-first palette with controlled neon accents.
- Scientific instrument aesthetic, not arcade.
- Clear hierarchy and low visual noise.

### 8.2 Consolidated Base Palette
- Backgrounds: `#05070D`, `#0B1020`, `#0F1730`.
- Text: `#E6F0FF` and attenuated variants.
- Accents: cyan, magenta, lime, amber, red.
- Border/glow: subtle focus and active-state use only.

### 8.3 Final One-Screen Layout
- Header: brand + current mode.
- Inputs panel.
- Processing panel.
- Export panel.
- Live telemetry footer.

### 8.4 Microinteractions
- Clear, short hover/focus transitions.
- Minimal, stable running-state motion.
- Non-distracting success and error feedback.

## 9) Localization and Internationalization (MUST)
- Required V1 locales:
  - English (`en`),
  - Simplified Chinese (`zh-CN`).
- No hardcoded text inside UI components.
- All visible copy must come from i18n keys.
- i18n structure must support adding future locales without major refactor.

## 10) Accessibility (MUST)
- Minimum WCAG AA contrast.
- Non-color-only status signaling.
- Full keyboard navigation.
- Respect system `reduce motion` preference.

## 11) Error Handling
- Human-readable short error message.
- Collapsible technical detail.
- `Copy debug report` action.
- `Retry` action.

## 12) QA Acceptance Criteria
1. Full RAM flow produces valid output.
2. DISK + Alphabetical Fast flow reports correct stages.
3. DISK + Alphabetical GlobalPerfect flow reports correct stages.
4. Separator escape interpretation is correct.
5. Cancellation works without UI blocking.
6. UI remains fluid under load with updates <= 10Hz.
7. Contrast and focus visibility are compliant.
8. Runtime language switch `en` <-> `zh-CN` works without app restart.

