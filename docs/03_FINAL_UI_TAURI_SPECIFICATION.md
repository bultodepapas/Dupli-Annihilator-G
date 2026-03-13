# Final UI/Tauri Specification (V1, No Code)

## Related Documents
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/02_FINAL_ENGINE_SPECIFICATION.md`
- `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
- `docs/05_PENDING_DECISIONS.md`
- `docs/06_DOCUMENT_CONTROL.md`

## 1) Scope and Source of Truth
This specification defines the desktop application layer for V1 using Tauri v2.

It is aligned with the final UI and integration decisions in `ideaV0.01.md` (final UI/Tauri + UI_SPEC sections), and with the engine contract documented in `docs/02_FINAL_ENGINE_SPECIFICATION.md`.

## 2) Product Objective (UI Layer)
Deliver a single-screen desktop workflow that allows users to:
1. select multiple input files,
2. configure processing mode and output behavior,
3. choose output file destination,
4. execute and cancel jobs safely,
5. monitor progress and telemetry without degrading responsiveness.

## 3) Architectural Boundaries

### 3.1 Responsibility Split
- Frontend (React/TypeScript): state orchestration, user interaction, presentation, telemetry visualization.
- Tauri backend bridge: command handling, job lifecycle orchestration, event emission.
- Rust core engine: all heavy processing (tokenization, dedupe, sorting, disk workflows).

### 3.2 Hard Boundaries
- No token-level data must be sent to frontend.
- Frontend must not process large file content.
- File-intensive and CPU-intensive work remains in Rust.

## 4) UX Information Architecture (One Screen)

### 4.1 Layout Model
- Header: product identity + current mode indicator.
- Inputs panel: source file intake and list management.
- Processing panel: mode, ordering, advanced processing controls.
- Export panel: output path + separator controls + execution controls.
- Telemetry footer: progress bar, stage strip, core metrics, detail line.

### 4.2 Responsive Behavior
- Desktop default: three-column layout (inputs / processing / export).
- Narrow widths: single-column stacked layout while preserving action order.

## 5) Functional Requirements (MUST)

### 5.1 Inputs
- Add files via picker and drag/drop.
- Display file list with name, size, truncated path.
- Per-file remove and `Clear all` actions.

### 5.2 Processing Configuration
- `Mode`: `AUTO`, `RAM`, `DISK`.
- `AUTO` behavior: host-aware heuristic based on available memory and sampled workload shape.
- `OutputOrdering`: `PreserveFirstSeen`, `Alphabetical`, `UnorderedFast`.
- Conditional controls:
  - If `DISK + Alphabetical`: show `FastBucketLocal` vs `GlobalPerfect`.
  - If `DISK + PreserveFirstSeen`: show explicit warning that global first-seen order is not guaranteed in V1.

### 5.3 Output Separator
- Preset separators + custom separator input.
- `Interpret escapes` toggle (default ON).
- Live preview using representative token sample.
- Separator syntax support target: `\n`, `\t`, `\r`, `\r\n`, `\f`, `\\`.

### 5.4 Output Path
- Save dialog for file path.
- Prevent run when output path is empty.
- Overwrite confirmation required when target exists.

### 5.5 Execution Control
- CTA state model:
  - `RUN` (idle),
  - `CANCEL` (running),
  - `RUN AGAIN` (success),
  - `RETRY` (error).
- Runtime state display:
  - `Idle`, `Running`, `Finalizing`, `Done`, `Error`, `Canceled`.

## 6) Job Lifecycle and State Transitions

### 6.1 Lifecycle
1. `Idle`
2. `Validating`
3. `Running`
4. `Finalizing`
5. terminal state: `Done` / `Error` / `Canceled`

### 6.2 Behavioral Rules
- `RUN` must be disabled until required inputs are valid.
- `CANCEL` must be idempotent at UI level.
- Transition to terminal state must unlock the form deterministically.

## 7) IPC Contract (Frontend <-> Tauri Backend)

### 7.1 Commands
- `start_job(config)`
  - validates config,
  - allocates `jobId`,
  - starts background execution,
  - returns job identity and acceptance response.
- `cancel_job(jobId)`
  - requests cooperative cancellation,
  - returns acknowledgment.
- `get_app_info()` (optional)
  - returns version/build/platform metadata.

### 7.2 Events
- `job://started`
- `job://progress`
- `job://stage` (optional but recommended)
- `job://done`
- `job://error`
- `job://canceled`

### 7.3 Minimum Progress Payload Contract
- identity: `jobId`
- stage context: `stage`, optional `detail`
- best-effort progress: `progress01`
- file-level counters: `filesDone`, `filesTotal`
- token-level counters: `tokensSeen`, `uniqueTokens`, `duplicates`
- runtime metrics: `throughputTps`, `elapsedMs`, `etaMs`

### 7.4 Error Payload Requirements
- short user-facing message,
- structured technical detail,
- stable error category for analytics and QA triage.

## 8) Progress, Stages, and ETA Model

### 8.1 Stage Taxonomy
- RAM stages:
  - `ScanningInputs`, `Tokenizing`, `Deduplicating`, `Sorting` (if applicable), `WritingOutput`, `Finalizing`.
- DISK fast stages:
  - `PartitioningBuckets`, `ReducingBuckets`, `WritingOutput`, `Finalizing`.
- DISK global perfect stages:
  - `GeneratingRuns`, `MergingRuns`, `WritingOutput`, `Finalizing`.

### 8.2 Progress Strategy
- Two-level approach:
  - global `progress01` (best effort),
  - current stage + detail line.
- Determinate progress only when confidence is acceptable.
- Indeterminate rendering when denominator quality is insufficient.

### 8.3 ETA Strategy
- Approximate ETA is preferred over unstable precision.
- Recommended approach:
  - compute throughput as delta over time,
  - smooth with EWMA,
  - estimate remaining time from trusted remaining-work proxy.
- If confidence is low, display `ETA: -`.

## 9) Performance Requirements (MUST)

### 9.1 Event Throughput Budget
- Backend event emission capped to approximately 4-10 Hz.
- Additional event emission allowed only for major milestones.

### 9.2 Frontend Rendering Budget
- Batch or throttle state updates.
- Avoid high-frequency component tree churn.
- Avoid unbounded logs and large uncontrolled lists.

### 9.3 UI Data Discipline
- Telemetry is aggregated.
- No per-token rendering.
- List virtualization enabled only when list volume justifies cost.

## 10) Security Requirements (MUST)

### 10.1 Capability Model
- Use least-privilege Tauri capability configuration per window.
- Enable only required plugins and permissions.

### 10.2 File Access Policy
- Prefer Rust-side file operations for large data.
- Frontend access limited to selection dialogs and essential metadata.

### 10.3 Runtime Safety Expectations
- Background tasks must never block the UI thread.
- Cancellation pathway must avoid orphan jobs and stale UI states.

## 11) Localization and i18n Requirements (MUST)
- V1 locales: `en`, `zh-CN`.
- No hardcoded user-facing strings in components.
- All copy must resolve from localization keys.
- Runtime language switching must not require restart.
- Key naming must support future locale growth without refactor.

## 12) Accessibility Requirements (MUST)
- WCAG AA minimum contrast targets.
- Non-color-only status representation.
- Full keyboard navigation.
- Respect `prefers-reduced-motion`.
- Focus states must remain visible under dark theme.

## 13) Visual System (Neon Lab)

### 13.1 Design Direction
- Dark-first, scientific instrument aesthetic.
- Controlled neon accents; avoid decorative overload.
- Preserve readability and information hierarchy under load.

### 13.2 Core Tokens
- Background: `#05070D`, `#0B1020`, `#0F1730`
- Text: `#E6F0FF` + attenuated variants
- Accent set: cyan, magenta, lime, amber, red
- Borders/glow: subtle, focus-only emphasis

### 13.3 Microinteraction Policy
- Short hover/focus transitions.
- Low-amplitude running animations.
- Clear success/error states without visual noise.

## 14) Validation and Error UX

### 14.1 Pre-Run Validation
- block run when required input is missing,
- block run when output path is missing,
- validate separator policy as configured by product.

### 14.2 Runtime Error UX
- concise human-readable message,
- expandable technical diagnostic details,
- `Copy debug report` support,
- deterministic `Retry` action.

### 14.3 Cancellation UX
- cancellation state must be explicit,
- user must be able to run again immediately after terminal cancellation state.

## 15) QA Acceptance Matrix (V1)
1. RAM path end-to-end run succeeds and output is valid.
2. DISK + Alphabetical + FastBucketLocal reports expected stage progression.
3. DISK + Alphabetical + GlobalPerfect reports expected stage progression.
4. Separator escape parsing behaves as configured.
5. Cancellation does not block UI and leads to valid terminal state.
6. UI remains responsive under load with update rate <= 10Hz.
7. Accessibility checks pass for contrast and keyboard flow.
8. Runtime language switch (`en` <-> `zh-CN`) works without restart.

## 16) Non-Goals (V1)
- Rich token-level logging UI.
- Locale-specific linguistic collation controls in UI.
- Full multi-window workflow orchestration.

## 17) Forward-Compatible Enhancements
1. Advanced stage diagnostics for expert mode.
2. Expanded language packs and translation QA pipeline.
3. Optional secondary monitoring window for long-running jobs.
4. User profiles/presets for processing configurations.
