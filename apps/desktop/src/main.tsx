import React from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { confirm, open, save } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { isSupportedLocale, type I18nKey, type Locale, supportedLocales, t } from "./i18n";
import "./styles.css";

type AppInfo = {
  appName: string;
  appVersion: string;
  backendVersion: string;
  updateChannel: string;
};

type GitHubRelease = {
  tag_name: string;
  html_url: string;
  name?: string | null;
  draft?: boolean;
  prerelease?: boolean;
};

type AvailableUpdate = {
  version: string;
  tag: string;
  url: string;
  isMajor: boolean;
};

type UpdateStatus = "idle" | "checking" | "up_to_date" | "available" | "downloading" | "ready_to_restart" | "error";
type UpdaterProgressEvent =
  | { event: "Started"; data: { contentLength?: number } }
  | { event: "Progress"; data: { chunkLength?: number } }
  | { event: "Finished" };

type RuntimeState = {
  isRunning: boolean;
  activeJobId: number | null;
};

type EmittedEvent = {
  topic: string;
  payload: Record<string, unknown>;
};

type ProgressSnapshot = {
  stage: string;
  filesDone: number;
  filesTotal: number;
  tokensSeen: number;
  uniqueTokens: number;
  duplicates: number;
  throughputTps: number;
  elapsedMs: number;
  etaMs: number | null;
};

type RunSummary = {
  jobId: number;
  status: "success" | "error" | "canceled";
  startedAt: string;
  finishedAt: string;
  filesTotal: number;
  filesDone: number;
  tokensSeen: number;
  uniqueTokens: number;
  duplicates: number;
  reductionPct: number;
  uniqPct: number;
  inputBytesTotal: number;
  outputPath: string;
  outputBytes: number;
  mode: string;
  modeEffective: string;
  ordering: string;
  diskAlphabeticalMode: string | null;
  diskBuckets: number | null;
  diskRunBytes: number | null;
  trim: boolean;
  dropEmpty: boolean;
  outputSeparatorRaw: string;
  outputSeparatorPreview: string;
  elapsedMs: number;
  avgThroughputTps: number;
  peakThroughputTps: number | null;
  stageDurationsMs: Record<string, number> | null;
  tempBytesTotal: number | null;
  warnings: string[];
  errorMessage: string | null;
};

type RunStatus = "idle" | "running" | "done" | "error" | "canceled";

type FormState = {
  inputsText: string;
  outputPath: string;
  allowOverwrite: boolean;
  separator: string;
  rawSeparator: boolean;
  mode: "auto" | "ram" | "disk";
  ordering: "preserve_first_seen" | "alphabetical" | "unordered_fast";
  diskAlphabeticalMode: "fast_bucket_local" | "global_perfect";
  trim: boolean;
  dropEmpty: boolean;
  diskBuckets: number;
  diskRunBytes: number;
};

const DEFAULT_FORM: FormState = {
  inputsText: "",
  outputPath: "",
  allowOverwrite: false,
  separator: "\\n",
  rawSeparator: false,
  mode: "ram",
  ordering: "preserve_first_seen",
  diskAlphabeticalMode: "fast_bucket_local",
  trim: true,
  dropEmpty: true,
  diskBuckets: 256,
  diskRunBytes: 256 * 1024 * 1024,
};

const LOCALE_STORAGE_KEY = "dupli.locale";
const INT_FORMAT = new Intl.NumberFormat();

function loadInitialLocale(): Locale {
  try {
    const raw = window.localStorage.getItem(LOCALE_STORAGE_KEY);
    if (raw && isSupportedLocale(raw)) {
      return raw;
    }
  } catch {
    // Ignore unavailable localStorage.
  }
  return "en";
}

const INITIAL_LOCALE = loadInitialLocale();

const EMPTY_PROGRESS: ProgressSnapshot = {
  stage: "-",
  filesDone: 0,
  filesTotal: 0,
  tokensSeen: 0,
  uniqueTokens: 0,
  duplicates: 0,
  throughputTps: 0,
  elapsedMs: 0,
  etaMs: null,
};

type SeparatorPreset = {
  label: string;
  value: string;
  raw: boolean;
};

const SEPARATOR_PRESETS: SeparatorPreset[] = [
  { label: "\\n", value: "\\n", raw: false },
  { label: "\\r\\n", value: "\\r\\n", raw: false },
  { label: "\\t", value: "\\t", raw: false },
  { label: ",", value: ",", raw: true },
  { label: ";", value: ";", raw: true },
  { label: "|", value: "|", raw: true },
];

// Stable request object — avoids allocating a new object on every poll tick.
// timeoutMs: 0 — next_events is a blocking Tauri command (recv_timeout on an mpsc Receiver).
// A non-zero timeout blocks a thread for that duration, which delays native dialog IPC calls
// (open/save dialogs) queued behind it. With 0ms the poll returns immediately via try_recv()
// semantics; the 300ms JS interval provides sufficient pacing with no perceptible event latency.
const NEXT_EVENTS_REQ = { req: { maxEvents: 64, timeoutMs: 0 } } as const;

// ─── Job state management ─────────────────────────────────────────────────────

type JobState = {
  runStatus: RunStatus;
  activeJobId: number | null;
  progress: ProgressSnapshot;
  message: string;
  lastSummary: RunSummary | null;
};

type JobAction =
  | { type: "job_starting"; message: string }
  | { type: "job_invoke_ok"; jobId: number }
  | { type: "job_invoke_err"; message: string }
  | { type: "event_job_started"; jobId: number | null; message: string }
  | { type: "event_progress"; snapshot: Partial<ProgressSnapshot> }
  | { type: "event_stage"; stage: string }
  | { type: "event_done"; message: string }
  | { type: "event_error"; message: string }
  | { type: "event_canceled"; message: string }
  | { type: "event_summary"; summary: RunSummary; message: string }
  | { type: "set_message"; message: string }
  | { type: "reset_summary" }
  | { type: "sync_runtime"; isRunning: boolean; activeJobId: number | null };

const INITIAL_JOB_STATE: JobState = {
  runStatus: "idle",
  activeJobId: null,
  progress: EMPTY_PROGRESS,
  message: t(INITIAL_LOCALE, "message.idle"),
  lastSummary: null,
};

function jobReducer(state: JobState, action: JobAction): JobState {
  switch (action.type) {
    case "job_starting":
      return {
        ...state,
        runStatus: "running",
        activeJobId: null,
        progress: EMPTY_PROGRESS,
        message: action.message,
        lastSummary: null,
      };
    case "job_invoke_ok":
      return { ...state, activeJobId: action.jobId };
    case "job_invoke_err":
      return { ...state, runStatus: "error", activeJobId: null, message: action.message };
    case "event_job_started":
      return {
        ...state,
        runStatus: "running",
        activeJobId: action.jobId ?? state.activeJobId,
        message: action.message,
      };
    case "event_progress":
      return { ...state, progress: { ...state.progress, ...action.snapshot } };
    case "event_stage":
      return { ...state, progress: { ...state.progress, stage: action.stage } };
    case "event_done":
      return { ...state, runStatus: "done", activeJobId: null, message: action.message };
    case "event_error":
      return { ...state, runStatus: "error", activeJobId: null, message: action.message };
    case "event_canceled":
      return { ...state, runStatus: "canceled", activeJobId: null, message: action.message };
    case "event_summary":
      return { ...state, lastSummary: action.summary, message: action.message };
    case "set_message":
      return { ...state, message: action.message };
    case "reset_summary":
      return { ...state, lastSummary: null };
    case "sync_runtime":
      return {
        ...state,
        runStatus: action.isRunning ? "running" : "idle",
        activeJobId: action.activeJobId,
      };
    default:
      return state;
  }
}

// ─── Utility functions ────────────────────────────────────────────────────────

function parseInputs(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

function parseEscapedSeparator(input: string): string {
  let out = "";
  let i = 0;

  while (i < input.length) {
    const c = input[i];
    if (c !== "\\") {
      out += c;
      i += 1;
      continue;
    }

    const next = input[i + 1];
    if (next === "n") {
      out += "\n";
      i += 2;
      continue;
    }
    if (next === "t") {
      out += "\t";
      i += 2;
      continue;
    }
    if (next === "f") {
      out += "\f";
      i += 2;
      continue;
    }
    if (next === "r") {
      if (input[i + 2] === "n") {
        out += "\r\n";
        i += 3;
      } else {
        out += "\r";
        i += 2;
      }
      continue;
    }
    if (next === "\\") {
      out += "\\";
      i += 2;
      continue;
    }
    if (next === undefined) {
      out += "\\";
      i += 1;
      continue;
    }
    out += `\\${next}`;
    i += 2;
  }

  return out;
}

function resolveSeparator(separator: string, raw: boolean): string {
  return raw ? separator : parseEscapedSeparator(separator);
}

function escapeControlChars(input: string): string {
  return input
    .replace(/\\/g, "\\\\")
    .replace(/\r/g, "\\r")
    .replace(/\n/g, "\\n")
    .replace(/\t/g, "\\t")
    .replace(/\f/g, "\\f");
}

function normalizeDroppedPath(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) {
    return null;
  }

  if (trimmed.startsWith("file://")) {
    try {
      const url = new URL(trimmed);
      const decoded = decodeURIComponent(url.pathname);
      if (/^\/[a-zA-Z]:/.test(decoded)) {
        return decoded.slice(1).replace(/\//g, "\\");
      }
      return decoded;
    } catch {
      return null;
    }
  }

  return trimmed;
}

function extractDroppedPaths(event: React.DragEvent<HTMLElement>): string[] {
  const out: string[] = [];
  const files = Array.from(event.dataTransfer.files);
  for (const file of files) {
    const candidate = (file as File & { path?: string }).path;
    if (candidate && candidate.trim().length > 0) {
      out.push(candidate.trim());
    }
  }

  const uriList = event.dataTransfer.getData("text/uri-list");
  if (uriList) {
    for (const line of uriList.split(/\r?\n/)) {
      if (!line || line.startsWith("#")) {
        continue;
      }
      const normalized = normalizeDroppedPath(line);
      if (normalized) {
        out.push(normalized);
      }
    }
  }

  const plain = event.dataTransfer.getData("text/plain");
  if (plain) {
    for (const line of plain.split(/\r?\n/)) {
      const normalized = normalizeDroppedPath(line);
      if (normalized) {
        out.push(normalized);
      }
    }
  }

  return Array.from(new Set(out));
}

function asNumber(value: unknown, fallback = 0): number {
  const n = Number(value);
  return Number.isFinite(n) ? n : fallback;
}

function asString(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function parseRunSummary(raw: Record<string, unknown>): RunSummary | null {
  const status = asString(raw.status) as RunSummary["status"];
  if (status !== "success" && status !== "error" && status !== "canceled") {
    return null;
  }

  const stageDurationsRaw =
    raw.stage_durations_ms && typeof raw.stage_durations_ms === "object"
      ? (raw.stage_durations_ms as Record<string, unknown>)
      : null;
  const stageDurationsMs = stageDurationsRaw
    ? Object.fromEntries(
        Object.entries(stageDurationsRaw).map(([k, v]) => [k, asNumber(v)]),
      )
    : null;

  return {
    jobId: asNumber(raw.job_id),
    status,
    startedAt: asString(raw.started_at),
    finishedAt: asString(raw.finished_at),
    filesTotal: asNumber(raw.files_total),
    filesDone: asNumber(raw.files_done),
    tokensSeen: asNumber(raw.tokens_seen),
    uniqueTokens: asNumber(raw.unique_tokens),
    duplicates: asNumber(raw.duplicates),
    reductionPct: asNumber(raw.reduction_pct),
    uniqPct: asNumber(raw.uniq_pct),
    inputBytesTotal: asNumber(raw.input_bytes_total),
    outputPath: asString(raw.output_path),
    outputBytes: asNumber(raw.output_bytes),
    mode: asString(raw.mode),
    modeEffective: asString(raw.mode_effective),
    ordering: asString(raw.ordering),
    diskAlphabeticalMode: raw.disk_alphabetical_mode ? asString(raw.disk_alphabetical_mode) : null,
    diskBuckets: raw.disk_buckets === null || raw.disk_buckets === undefined ? null : asNumber(raw.disk_buckets),
    diskRunBytes:
      raw.disk_run_bytes === null || raw.disk_run_bytes === undefined ? null : asNumber(raw.disk_run_bytes),
    trim: Boolean(raw.trim),
    dropEmpty: Boolean(raw.drop_empty),
    outputSeparatorRaw: asString(raw.output_separator_raw),
    outputSeparatorPreview: asString(raw.output_separator_preview),
    elapsedMs: asNumber(raw.elapsed_ms),
    avgThroughputTps: asNumber(raw.avg_throughput_tps),
    peakThroughputTps:
      raw.peak_throughput_tps === null || raw.peak_throughput_tps === undefined
        ? null
        : asNumber(raw.peak_throughput_tps),
    stageDurationsMs,
    tempBytesTotal:
      raw.temp_bytes_total === null || raw.temp_bytes_total === undefined ? null : asNumber(raw.temp_bytes_total),
    warnings: Array.isArray(raw.warnings) ? raw.warnings.map((w) => String(w)) : [],
    errorMessage: raw.error_message ? asString(raw.error_message) : null,
  };
}

function formatInt(value: number): string {
  return INT_FORMAT.format(value);
}

function formatPct(value: number): string {
  return `${value.toFixed(2)}%`;
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return "-";
  }
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = bytes;
  let idx = 0;
  while (size >= 1024 && idx < units.length - 1) {
    size /= 1024;
    idx += 1;
  }
  const frac = idx === 0 ? 0 : 2;
  return `${size.toFixed(frac)} ${units[idx]}`;
}

function formatElapsed(elapsedMs: number): string {
  const totalMs = Math.max(0, Math.floor(elapsedMs));
  const totalSec = Math.floor(totalMs / 1000);
  const minutes = Math.floor(totalSec / 60);
  const seconds = totalSec % 60;
  const tenths = Math.floor((totalMs % 1000) / 100);
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${tenths}`;
}

function formatIsoLocal(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso || "-";
  }
  return date.toLocaleString();
}

function parseSemver(version: string): { major: number; minor: number; patch: number } | null {
  const match = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(version.trim());
  if (!match) {
    return null;
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  };
}

function compareSemver(
  a: { major: number; minor: number; patch: number },
  b: { major: number; minor: number; patch: number },
): number {
  if (a.major !== b.major) {
    return a.major - b.major;
  }
  if (a.minor !== b.minor) {
    return a.minor - b.minor;
  }
  return a.patch - b.patch;
}

function isMajorUpgrade(
  latest: { major: number; minor: number; patch: number },
  current: { major: number; minor: number; patch: number },
): boolean {
  return latest.major > current.major;
}

function prettyMode(summary: RunSummary): string {
  if (summary.mode === "auto" && summary.modeEffective) {
    return `AUTO(->${summary.modeEffective.toUpperCase()})`;
  }
  return summary.mode.toUpperCase();
}

function buildSummaryBadge(summary: RunSummary): { text: string; cls: string } {
  if (summary.status === "error") {
    return { text: "ERROR", cls: "summary-badge-error" };
  }
  if (summary.status === "canceled" || summary.warnings.length > 0) {
    return { text: "WARNING", cls: "summary-badge-warning" };
  }
  return { text: "SUCCESS", cls: "summary-badge-success" };
}

function validateForm(form: FormState, tr: (key: I18nKey, params?: Record<string, string | number>) => string): string[] {
  const errors: string[] = [];
  const inputs = parseInputs(form.inputsText);
  if (inputs.length === 0) {
    errors.push(tr("validation.inputs_required"));
  }
  if (form.outputPath.trim().length === 0) {
    errors.push(tr("validation.output_required"));
  }
  if (form.separator.length === 0) {
    errors.push(tr("validation.separator_required"));
  }
  if (form.mode === "disk") {
    if (!Number.isFinite(form.diskBuckets) || form.diskBuckets < 8) {
      errors.push(tr("validation.disk_buckets_min"));
    }
    if (!Number.isFinite(form.diskRunBytes) || form.diskRunBytes < 1_000_000) {
      errors.push(tr("validation.disk_run_bytes_min"));
    }
  }
  return errors;
}

function statusKey(status: RunStatus): I18nKey {
  switch (status) {
    case "idle":
      return "status.idle";
    case "running":
      return "status.running";
    case "done":
      return "status.done";
    case "error":
      return "status.error";
    case "canceled":
      return "status.canceled";
    default:
      return "status.idle";
  }
}

function runButtonKey(status: RunStatus): I18nKey {
  switch (status) {
    case "done":
      return "button.run_again";
    case "error":
      return "button.retry";
    default:
      return "button.run";
  }
}

function summaryTitleKey(status: RunSummary["status"]): I18nKey {
  switch (status) {
    case "success":
      return "summary.title.success";
    case "error":
      return "summary.title.error";
    case "canceled":
      return "summary.title.canceled";
    default:
      return "summary.title.success";
  }
}

async function resolveDefaultOutputPath(): Promise<string | null> {
  try {
    return await invoke<string>("default_output_path");
  } catch {
    return null;
  }
}

// ─── TelemetryFooter ──────────────────────────────────────────────────────────

type TelemetryFooterProps = {
  progress: ProgressSnapshot;
  validationErrors: readonly string[];
  runStatus: RunStatus;
  message: string;
  tr: (key: I18nKey, params?: Record<string, string | number>) => string;
};

const TelemetryFooter = React.memo(function TelemetryFooter({
  progress,
  validationErrors,
  runStatus,
  message,
  tr,
}: TelemetryFooterProps) {
  const progressPercent =
    progress.filesTotal > 0 ? Math.min(100, (progress.filesDone / progress.filesTotal) * 100) : 0;

  return (
    <footer className="telemetry card">
      <h2>{tr("section.telemetry")}</h2>
      {validationErrors.length > 0 && runStatus !== "running" ? (
        <div className="errors">
          {validationErrors.map((err) => (
            <div key={err}>{err}</div>
          ))}
        </div>
      ) : null}
      <div className="bar-wrap">
        <div className="bar" style={{ width: `${progressPercent}%` }} />
      </div>
      <div className="metrics">
        <div>
          {tr("metric.stage")}: {progress.stage}
        </div>
        <div>
          {tr("metric.files")}: {progress.filesDone}/{progress.filesTotal}
        </div>
        <div>
          {tr("metric.tokens")}: {progress.tokensSeen}
        </div>
        <div>
          {tr("metric.unique")}: {progress.uniqueTokens}
        </div>
        <div>
          {tr("metric.duplicates")}: {progress.duplicates}
        </div>
        <div>
          {tr("metric.tps")}: {progress.throughputTps}
        </div>
        <div>
          {tr("metric.elapsed_ms")}: {progress.elapsedMs}
        </div>
        <div>
          {tr("metric.eta_ms")}: {progress.etaMs ?? "-"}
        </div>
      </div>
      <p className="message">{message}</p>
    </footer>
  );
});

// ─── Header ───────────────────────────────────────────────────────────────────

type HeaderProps = {
  locale: Locale;
  updateStatus: UpdateStatus;
  availableUpdate: AvailableUpdate | null;
  runStatus: RunStatus;
  tr: (key: I18nKey, params?: Record<string, string | number>) => string;
  onLocaleChange: (locale: Locale) => void;
  onCheckForUpdates: () => void;
};

const Header = React.memo(function Header({
  locale,
  updateStatus,
  availableUpdate,
  runStatus,
  tr,
  onLocaleChange,
  onCheckForUpdates,
}: HeaderProps) {
  return (
    <header className="topbar">
      <div>
        <h1>Dupli-Annihilator-G</h1>
        <p className="subtitle">{tr("app.subtitle")}</p>
      </div>
      <div className="topbar-actions">
        <button
          className="secondary topbar-btn"
          type="button"
          disabled={updateStatus === "checking" || updateStatus === "downloading"}
          onClick={onCheckForUpdates}
        >
          {updateStatus === "checking" ? tr("button.checking_updates") : tr("button.check_updates")}
        </button>
        <label className="lang-control">
          <span>{tr("field.language")}</span>
          <select value={locale} onChange={(e) => onLocaleChange(e.target.value as Locale)}>
            {supportedLocales.map((loc) => (
              <option key={loc} value={loc}>
                {t(loc, "lang.name")}
              </option>
            ))}
          </select>
        </label>
        {availableUpdate ? (
          <div className="update-pill">{tr("update.available_pill", { version: availableUpdate.tag })}</div>
        ) : null}
        <div className={`status-chip status-${runStatus}`}>{tr(statusKey(runStatus))}</div>
      </div>
    </header>
  );
});

// ─── UpdateBanner ─────────────────────────────────────────────────────────────

type UpdateBannerProps = {
  availableUpdate: AvailableUpdate;
  updateStatus: UpdateStatus;
  updateDownloadPct: number | null;
  appInfo: AppInfo | null;
  updaterInstallReady: boolean;
  tr: (key: I18nKey, params?: Record<string, string | number>) => string;
  onInstall: () => void;
  onOpenRelease: () => void;
  onCheckForUpdates: () => void;
  onRestart: () => void;
};

const UpdateBanner = React.memo(function UpdateBanner({
  availableUpdate,
  updateStatus,
  updateDownloadPct,
  appInfo,
  updaterInstallReady,
  tr,
  onInstall,
  onOpenRelease,
  onCheckForUpdates,
  onRestart,
}: UpdateBannerProps) {
  return (
    <section className="card update-banner">
      <h2>{tr("update.banner_title", { version: availableUpdate.tag })}</h2>
      <p>{tr("update.banner_body", { current: appInfo?.appVersion ?? "-", latest: availableUpdate.tag })}</p>
      {availableUpdate.isMajor ? <p>{tr("update.major_notice")}</p> : null}
      {updateStatus === "downloading" ? (
        <p>{tr("update.download_progress", { pct: Math.round(updateDownloadPct ?? 0) })}</p>
      ) : null}
      <div className="update-actions">
        {updateStatus === "ready_to_restart" ? (
          <button className="primary" type="button" onClick={onRestart}>
            {tr("button.restart_to_apply")}
          </button>
        ) : updaterInstallReady && !availableUpdate.isMajor ? (
          <button
            className="primary"
            type="button"
            disabled={updateStatus === "downloading"}
            onClick={onInstall}
          >
            {updateStatus === "downloading" ? tr("button.installing_update") : tr("button.install_update")}
          </button>
        ) : null}
        <button
          className="secondary"
          type="button"
          disabled={updateStatus === "downloading"}
          onClick={onOpenRelease}
        >
          {tr("button.download_update")}
        </button>
        <button
          className="secondary"
          type="button"
          disabled={updateStatus === "checking" || updateStatus === "downloading"}
          onClick={onCheckForUpdates}
        >
          {tr("button.check_updates")}
        </button>
      </div>
    </section>
  );
});

// ─── SummaryScreen ────────────────────────────────────────────────────────────

type SummaryScreenProps = {
  summary: RunSummary;
  summaryBadge: { text: string; cls: string };
  summaryTopStages: Array<[string, number]>;
  summaryStageRows: Array<[string, number]>;
  appInfo: AppInfo | null;
  canRun: boolean;
  tr: (key: I18nKey, params?: Record<string, string | number>) => string;
  onOpenOutput: () => void;
  onOpenFolder: () => void;
  onCopyReport: () => void;
  onExportJson: () => void;
  onClose: () => void;
  onRunAgain: () => void;
};

const SummaryScreen = React.memo(function SummaryScreen({
  summary,
  summaryBadge,
  summaryTopStages,
  summaryStageRows,
  appInfo,
  canRun,
  tr,
  onOpenOutput,
  onOpenFolder,
  onCopyReport,
  onExportJson,
  onClose,
  onRunAgain,
}: SummaryScreenProps) {
  const reductionBarWidth = `${Math.min(100, Math.max(0, summary.reductionPct))}%`;

  return (
    <section className="summary-shell">
      <header className="card summary-header">
        <div>
          <h2 className="summary-kicker">{tr("summary.mission_report")}</h2>
          <h3>{tr(summaryTitleKey(summary.status))}</h3>
          <p className="subtitle">
            {tr("summary.subline", {
              jobId: summary.jobId,
              timestamp: formatIsoLocal(summary.finishedAt),
              mode: prettyMode(summary),
              ordering: summary.ordering,
            })}
          </p>
        </div>
        <div className={`summary-badge ${summaryBadge.cls}`}>{summaryBadge.text}</div>
      </header>

      <div className="summary-grid">
        <section className="card summary-card">
          <h3>{tr("summary.section.key_results")}</h3>
          <div className="summary-hero-value">{formatInt(summary.uniqueTokens)}</div>
          <div className="summary-hero-label">{tr("summary.metric.unique_output")}</div>
          <div className="summary-kpis">
            <div>
              <strong>{formatInt(summary.tokensSeen)}</strong>
              <span>{tr("summary.metric.total_scanned")}</span>
            </div>
            <div>
              <strong>{formatInt(summary.duplicates)}</strong>
              <span>{tr("summary.metric.duplicates_removed")}</span>
            </div>
            <div>
              <strong>{formatPct(summary.reductionPct)}</strong>
              <span>{tr("summary.metric.reduction")}</span>
            </div>
            <div>
              <strong>{formatPct(summary.uniqPct)}</strong>
              <span>{tr("summary.metric.uniq_rate")}</span>
            </div>
          </div>
          <div className="summary-gauge-wrap">
            <div className="summary-gauge-label">{tr("summary.metric.reduction_ratio")}</div>
            <div className="bar-wrap">
              <div className="bar" style={{ width: reductionBarWidth }} />
            </div>
          </div>

          <h4>{tr("summary.section.output")}</h4>
          <div className="summary-meta-grid">
            <div>{tr("summary.metric.output_path")}</div>
            <code>{summary.outputPath || "-"}</code>
            <div>{tr("summary.metric.output_bytes")}</div>
            <div>{formatBytes(summary.outputBytes)}</div>
            <div>{tr("summary.metric.separator")}</div>
            <div>
              <code>{summary.outputSeparatorRaw || "-"}</code> ({summary.outputSeparatorPreview || "-"})
            </div>
          </div>
        </section>

        <section className="card summary-card">
          <h3>{tr("summary.section.performance")}</h3>
          <div className="summary-meta-grid">
            <div>{tr("summary.metric.elapsed")}</div>
            <div>{formatElapsed(summary.elapsedMs)}</div>
            <div>{tr("summary.metric.avg_tps")}</div>
            <div>{formatInt(summary.avgThroughputTps)}</div>
            <div>{tr("summary.metric.peak_tps")}</div>
            <div>{summary.peakThroughputTps ? formatInt(summary.peakThroughputTps) : "-"}</div>
            <div>{tr("summary.metric.input_bytes")}</div>
            <div>{formatBytes(summary.inputBytesTotal)}</div>
            <div>{tr("summary.metric.mode_ordering")}</div>
            <div>{prettyMode(summary)} / {summary.ordering}</div>
            <div>{tr("summary.metric.normalization")}</div>
            <div>
              trim={summary.trim ? "on" : "off"} | drop_empty={summary.dropEmpty ? "on" : "off"}
            </div>
            <div>{tr("summary.metric.app_version")}</div>
            <div>{appInfo?.appVersion ?? "-"}</div>
            <div>{tr("summary.metric.backend_version")}</div>
            <div>{appInfo?.backendVersion ?? "-"}</div>
            <div>{tr("summary.metric.update_channel")}</div>
            <div>{appInfo?.updateChannel ?? "stable"}</div>
            {summary.diskAlphabeticalMode ? (
              <>
                <div>{tr("summary.metric.disk_mode")}</div>
                <div>{summary.diskAlphabeticalMode}</div>
              </>
            ) : null}
          </div>

          <details className="summary-timeline" open={summaryTopStages.length > 0}>
            <summary>{tr("summary.section.timeline")}</summary>
            {summaryTopStages.length > 0 ? (
              <div className="summary-stage-table">
                {summaryTopStages.map(([stage, duration]) => (
                  <div key={stage} className="summary-stage-row summary-stage-top">
                    <span>{stage}</span>
                    <strong>{formatElapsed(duration)}</strong>
                  </div>
                ))}
              </div>
            ) : (
              <div className="summary-empty">{tr("summary.no_stage_data")}</div>
            )}
            {summaryStageRows.length > 3 ? (
              <div className="summary-stage-table">
                {summaryStageRows.slice(3).map(([stage, duration]) => (
                  <div key={stage} className="summary-stage-row">
                    <span>{stage}</span>
                    <span>{formatElapsed(duration)}</span>
                  </div>
                ))}
              </div>
            ) : null}
          </details>

          <h4>{tr("summary.section.diagnostics")}</h4>
          {summary.warnings.length > 0 ? (
            <div className="warnings">
              {summary.warnings.map((warning) => (
                <div key={warning}>{warning}</div>
              ))}
            </div>
          ) : (
            <div className="summary-empty">{tr("summary.no_warnings")}</div>
          )}
          {summary.errorMessage ? (
            <div className="errors">
              <div>{summary.errorMessage}</div>
            </div>
          ) : null}
        </section>
      </div>

      <footer className="card summary-footer">
        <div className="button-row">
          <button className="secondary" onClick={onOpenOutput}>{tr("button.open_output")}</button>
          <button className="secondary" onClick={onOpenFolder}>{tr("button.open_folder")}</button>
          <button className="secondary" onClick={onCopyReport}>{tr("button.copy_report")}</button>
          <button className="secondary" onClick={onExportJson}>{tr("button.export_json")}</button>
          <button className="secondary" onClick={onClose}>{tr("button.close_report")}</button>
          <button className="primary" disabled={!canRun} onClick={onRunAgain}>{tr("button.run_again")}</button>
        </div>
      </footer>
    </section>
  );
});

// ─── InputSection ─────────────────────────────────────────────────────────────

type InputSectionProps = {
  form: FormState;
  runStatus: RunStatus;
  inputsDragActive: boolean;
  tr: (key: I18nKey, params?: Record<string, string | number>) => string;
  onFormChange: React.Dispatch<React.SetStateAction<FormState>>;
  onPickInputFiles: () => void;
  onPickInputFolder: () => void;
  onPickOutputFile: () => void;
  onDragOver: (e: React.DragEvent<HTMLDivElement>) => void;
  onDragLeave: (e: React.DragEvent<HTMLDivElement>) => void;
  onDrop: (e: React.DragEvent<HTMLDivElement>) => void;
};

const InputSection = React.memo(function InputSection({
  form,
  runStatus,
  inputsDragActive,
  tr,
  onFormChange,
  onPickInputFiles,
  onPickInputFolder,
  onPickOutputFile,
  onDragOver,
  onDragLeave,
  onDrop,
}: InputSectionProps) {
  return (
    <section className="card">
      <h2>{tr("section.inputs")}</h2>
      <div className="button-row compact">
        <button className="secondary" disabled={runStatus === "running"} onClick={onPickInputFiles}>
          {tr("button.add_files")}
        </button>
        <button className="secondary" disabled={runStatus === "running"} onClick={onPickInputFolder}>
          {tr("button.add_folder")}
        </button>
      </div>
      <label className="field">
        <span>{tr("field.inputs")}</span>
        <div
          className={`drop-zone ${inputsDragActive ? "drop-zone-active" : ""}`}
          onDragOver={onDragOver}
          onDragLeave={onDragLeave}
          onDrop={onDrop}
        >
          <textarea
            value={form.inputsText}
            onChange={(e) => onFormChange((f) => ({ ...f, inputsText: e.target.value }))}
            rows={8}
            placeholder={tr("placeholder.inputs")}
          />
          <div className="drop-hint">{tr("hint.drop_files")}</div>
        </div>
      </label>
      <label className="field">
        <span>{tr("field.output")}</span>
        <input
          value={form.outputPath}
          onChange={(e) => onFormChange((f) => ({ ...f, outputPath: e.target.value }))}
          placeholder={tr("placeholder.output")}
        />
      </label>
      <div className="button-row compact">
        <button className="secondary" disabled={runStatus === "running"} onClick={onPickOutputFile}>
          {tr("button.pick_output")}
        </button>
      </div>
    </section>
  );
});

// ─── ProcessingSection ────────────────────────────────────────────────────────

type ProcessingSectionProps = {
  form: FormState;
  tr: (key: I18nKey, params?: Record<string, string | number>) => string;
  onFormChange: React.Dispatch<React.SetStateAction<FormState>>;
};

const ProcessingSection = React.memo(function ProcessingSection({
  form,
  tr,
  onFormChange,
}: ProcessingSectionProps) {
  return (
    <section className="card">
      <h2>{tr("section.processing")}</h2>
      <div className="row">
        <label className="field" title={tr("tooltip.processing.mode")}>
          <span title={tr("tooltip.processing.mode")}>{tr("field.mode")}</span>
          <select
            title={tr("tooltip.processing.mode")}
            value={form.mode}
            onChange={(e) => onFormChange((f) => ({ ...f, mode: e.target.value as FormState["mode"] }))}
          >
            <option value="auto">{tr("option.mode.auto")}</option>
            <option value="ram">{tr("option.mode.ram")}</option>
            <option value="disk">{tr("option.mode.disk")}</option>
          </select>
        </label>
        <label className="field" title={tr("tooltip.processing.ordering")}>
          <span title={tr("tooltip.processing.ordering")}>{tr("field.ordering")}</span>
          <select
            title={tr("tooltip.processing.ordering")}
            value={form.ordering}
            onChange={(e) =>
              onFormChange((f) => ({ ...f, ordering: e.target.value as FormState["ordering"] }))
            }
          >
            <option value="preserve_first_seen">{tr("option.ordering.preserve_first_seen")}</option>
            <option value="alphabetical">{tr("option.ordering.alphabetical")}</option>
            <option value="unordered_fast">{tr("option.ordering.unordered_fast")}</option>
          </select>
        </label>
      </div>

      <label className="field" title={tr("tooltip.processing.disk_alphabetical_mode")}>
        <span title={tr("tooltip.processing.disk_alphabetical_mode")}>
          {tr("field.disk_alphabetical_mode")}
        </span>
        <select
          title={tr("tooltip.processing.disk_alphabetical_mode")}
          value={form.diskAlphabeticalMode}
          onChange={(e) =>
            onFormChange((f) => ({
              ...f,
              diskAlphabeticalMode: e.target.value as FormState["diskAlphabeticalMode"],
            }))
          }
        >
          <option value="fast_bucket_local">{tr("option.disk_mode.fast_bucket_local")}</option>
          <option value="global_perfect">{tr("option.disk_mode.global_perfect")}</option>
        </select>
      </label>

      <div className="row">
        <label className="field" title={tr("tooltip.processing.disk_buckets")}>
          <span title={tr("tooltip.processing.disk_buckets")}>{tr("field.disk_buckets")}</span>
          <input
            title={tr("tooltip.processing.disk_buckets")}
            type="number"
            min={8}
            value={form.diskBuckets}
            onChange={(e) => onFormChange((f) => ({ ...f, diskBuckets: Number(e.target.value) }))}
          />
        </label>
        <label className="field" title={tr("tooltip.processing.disk_run_bytes")}>
          <span title={tr("tooltip.processing.disk_run_bytes")}>{tr("field.disk_run_bytes")}</span>
          <input
            title={tr("tooltip.processing.disk_run_bytes")}
            type="number"
            min={1_000_000}
            value={form.diskRunBytes}
            onChange={(e) => onFormChange((f) => ({ ...f, diskRunBytes: Number(e.target.value) }))}
          />
        </label>
      </div>

      <div className="row flags">
        <label title={tr("tooltip.processing.trim")}>
          <input
            title={tr("tooltip.processing.trim")}
            type="checkbox"
            checked={form.trim}
            onChange={(e) => onFormChange((f) => ({ ...f, trim: e.target.checked }))}
          />
          {tr("flag.trim")}
        </label>
        <label title={tr("tooltip.processing.drop_empty")}>
          <input
            title={tr("tooltip.processing.drop_empty")}
            type="checkbox"
            checked={form.dropEmpty}
            onChange={(e) => onFormChange((f) => ({ ...f, dropEmpty: e.target.checked }))}
          />
          {tr("flag.drop_empty")}
        </label>
      </div>
    </section>
  );
});

// ─── OutputSection ────────────────────────────────────────────────────────────

type OutputSectionProps = {
  form: FormState;
  runStatus: RunStatus;
  activeJobId: number | null;
  appInfo: AppInfo | null;
  canRun: boolean;
  canCancel: boolean;
  separatorPreview: string;
  separatorPreviewVisible: string;
  resolvedSeparator: string;
  tr: (key: I18nKey, params?: Record<string, string | number>) => string;
  onFormChange: React.Dispatch<React.SetStateAction<FormState>>;
  onStartJob: () => void;
  onCancelJob: () => void;
};

const OutputSection = React.memo(function OutputSection({
  form,
  runStatus,
  activeJobId,
  appInfo,
  canRun,
  canCancel,
  separatorPreview,
  separatorPreviewVisible,
  resolvedSeparator,
  tr,
  onFormChange,
  onStartJob,
  onCancelJob,
}: OutputSectionProps) {
  return (
    <section className="card">
      <h2>{tr("section.output")}</h2>
      <label className="field">
        <span>{tr("field.separator")}</span>
        <input
          value={form.separator}
          onChange={(e) => onFormChange((f) => ({ ...f, separator: e.target.value }))}
          placeholder="\\n"
        />
      </label>
      <label className="field">
        <span>{tr("field.separator_presets")}</span>
        <div className="preset-row">
          {SEPARATOR_PRESETS.map((preset) => (
            <button
              key={`${preset.label}:${preset.value}:${preset.raw}`}
              className="secondary preset-btn"
              type="button"
              onClick={() =>
                onFormChange((f) => ({ ...f, separator: preset.value, rawSeparator: preset.raw }))
              }
            >
              {preset.label}
            </button>
          ))}
        </div>
      </label>
      <label className="field">
        <span>{tr("field.separator_preview")}</span>
        <pre className="separator-preview">{separatorPreview}</pre>
        <div className="separator-preview-meta">
          {tr("meta.effective_separator")}: <code>{escapeControlChars(resolvedSeparator)}</code>
        </div>
        <div className="separator-preview-meta">
          {tr("metric.tokens")}: <code>{separatorPreviewVisible}</code>
        </div>
      </label>
      <label className="field checkbox">
        <input
          type="checkbox"
          checked={form.rawSeparator}
          onChange={(e) => onFormChange((f) => ({ ...f, rawSeparator: e.target.checked }))}
        />
        <span>{tr("field.raw_separator")}</span>
      </label>
      <label className="field checkbox">
        <input
          type="checkbox"
          checked={form.allowOverwrite}
          onChange={(e) => onFormChange((f) => ({ ...f, allowOverwrite: e.target.checked }))}
        />
        <span>{tr("field.allow_overwrite")}</span>
      </label>

      <div className="button-row">
        <button className="primary" disabled={!canRun} onClick={onStartJob}>
          {tr(runButtonKey(runStatus))}
        </button>
        <button className="danger" disabled={!canCancel} onClick={onCancelJob}>
          {tr("button.cancel")}
        </button>
      </div>

      <div className="meta">
        <div>
          {tr("meta.app")}: {appInfo?.appName ?? "-"} {appInfo?.appVersion ?? ""}
        </div>
        <div>
          {tr("meta.backend")}: {appInfo?.backendVersion ?? "-"}
        </div>
        <div>
          {tr("meta.update_channel")}: {appInfo?.updateChannel ?? "stable"}
        </div>
        <div>
          {tr("meta.job_id")}: {activeJobId ?? "-"}
        </div>
        <div className="license-notice">
          {tr("meta.license")}
        </div>
      </div>
    </section>
  );
});

// ─── WordCheckerPanel ─────────────────────────────────────────────────────────

type WordCheckerPanelProps = {
  checkerPath: string;
  checkerStatus: "idle" | "loading" | "ready" | "error";
  checkerWordCount: number;
  checkerWord: string;
  checkerResult: "found" | "not_found" | null;
  checkerMessage: string;
  tr: (key: I18nKey, params?: Record<string, string | number>) => string;
  onPathChange: (path: string) => void;
  onPickFile: () => void;
  onLoad: () => void;
  onWordChange: (word: string) => void;
  onCheck: () => void;
};

const WordCheckerPanel = React.memo(function WordCheckerPanel({
  checkerPath,
  checkerStatus,
  checkerWordCount,
  checkerWord,
  checkerResult,
  checkerMessage,
  tr,
  onPathChange,
  onPickFile,
  onLoad,
  onWordChange,
  onCheck,
}: WordCheckerPanelProps) {
  return (
    <section className="card">
      <h2>{tr("section.checker")}</h2>

      <div className="button-row">
        <input
          className="path-input"
          value={checkerPath}
          onChange={(e) => onPathChange(e.target.value)}
          placeholder={tr("checker.path_placeholder")}
          disabled={checkerStatus === "loading"}
        />
        <button
          className="secondary"
          type="button"
          onClick={onPickFile}
          disabled={checkerStatus === "loading"}
        >
          {tr("button.pick_file")}
        </button>
        <button
          className="primary"
          type="button"
          onClick={onLoad}
          disabled={checkerStatus === "loading" || !checkerPath.trim()}
        >
          {checkerStatus === "loading" ? tr("checker.loading") : tr("checker.load")}
        </button>
      </div>

      {checkerStatus === "ready" && (
        <p className="message">{tr("checker.ready", { count: checkerWordCount })}</p>
      )}
      {checkerMessage && checkerStatus !== "ready" && (
        <p className="message">{checkerMessage}</p>
      )}

      {checkerStatus === "ready" && (
        <div className="button-row">
          <input
            className="path-input"
            value={checkerWord}
            onChange={(e) => onWordChange(e.target.value)}
            placeholder={tr("checker.word_placeholder")}
            onKeyDown={(e) => { if (e.key === "Enter") onCheck(); }}
          />
          <button
            className="primary"
            type="button"
            onClick={onCheck}
            disabled={!checkerWord.trim()}
          >
            {tr("checker.check")}
          </button>
        </div>
      )}

      {checkerResult === "found" && (
        <p className="message checker-found">{tr("checker.result_found")}</p>
      )}
      {checkerResult === "not_found" && (
        <p className="message checker-not-found">{tr("checker.result_not_found")}</p>
      )}
    </section>
  );
});

// ─── App ──────────────────────────────────────────────────────────────────────

function App() {
  const [locale, setLocale] = React.useState<Locale>(INITIAL_LOCALE);
  const tr = React.useCallback(
    (key: I18nKey, params?: Record<string, string | number>) => t(locale, key, params),
    [locale],
  );
  // Stable ref for locale — lets pollEvents and checkForUpdates use t(localeRef.current, ...)
  // without capturing tr in their closures (which would cause the polling interval to restart
  // every time the user changes the language).
  const localeRef = React.useRef<Locale>(locale);
  React.useEffect(() => {
    localeRef.current = locale;
  }, [locale]);

  const [form, setForm] = React.useState<FormState>(DEFAULT_FORM);
  const [appInfo, setAppInfo] = React.useState<AppInfo | null>(null);
  const [jobState, dispatch] = React.useReducer(jobReducer, INITIAL_JOB_STATE);
  const [inputsDragActive, setInputsDragActive] = React.useState(false);
  const [updateStatus, setUpdateStatus] = React.useState<UpdateStatus>("idle");
  const [availableUpdate, setAvailableUpdate] = React.useState<AvailableUpdate | null>(null);
  const [updateDownloadPct, setUpdateDownloadPct] = React.useState<number | null>(null);
  const [updaterInstallReady, setUpdaterInstallReady] = React.useState(false);

  const [checkerPath, setCheckerPath] = React.useState<string>("");
  const [checkerStatus, setCheckerStatus] = React.useState<"idle" | "loading" | "ready" | "error">("idle");
  const [checkerWordCount, setCheckerWordCount] = React.useState<number>(0);
  const [checkerWord, setCheckerWord] = React.useState<string>("");
  const [checkerResult, setCheckerResult] = React.useState<"found" | "not_found" | null>(null);
  const [checkerMessage, setCheckerMessage] = React.useState<string>("");

  const pollingRef = React.useRef(false);
  const updateAutoCheckedRef = React.useRef(false);

  // Destructure for convenience in derived values and callbacks.
  const { runStatus, activeJobId, progress, message, lastSummary } = jobState;

  const parsedInputs = React.useMemo(() => parseInputs(form.inputsText), [form.inputsText]);
  const validationErrors = React.useMemo(() => validateForm(form, tr), [form, tr]);
  const resolvedSeparator = React.useMemo(
    () => resolveSeparator(form.separator, form.rawSeparator),
    [form.separator, form.rawSeparator],
  );
  const separatorPreview = React.useMemo(
    () => ["alpha", "beta", "gamma"].join(resolvedSeparator),
    [resolvedSeparator],
  );
  const separatorPreviewVisible = React.useMemo(
    () => escapeControlChars(separatorPreview),
    [separatorPreview],
  );
  const showSummaryScreen = lastSummary !== null && runStatus !== "running";
  const summaryBadge = React.useMemo(
    () => (lastSummary ? buildSummaryBadge(lastSummary) : null),
    [lastSummary],
  );
  const summaryStageRows = React.useMemo(() => {
    if (!lastSummary?.stageDurationsMs) {
      return [] as Array<[string, number]>;
    }
    return Object.entries(lastSummary.stageDurationsMs).sort((a, b) => b[1] - a[1]);
  }, [lastSummary]);
  const summaryTopStages = React.useMemo(() => summaryStageRows.slice(0, 3), [summaryStageRows]);

  const canRun = runStatus !== "running" && validationErrors.length === 0;
  const canCancel = runStatus === "running" && activeJobId !== null;

  const buildSummaryReport = React.useCallback(
    (summary: RunSummary): string => {
      const lines: string[] = [];
      lines.push("MISSION REPORT");
      lines.push(
        `${summary.status.toUpperCase()} | ${prettyMode(summary)} | ${summary.ordering} | ${summary.diskAlphabeticalMode ?? "-"}`,
      );
      lines.push(`job_id: ${summary.jobId}`);
      lines.push(`finished_at: ${summary.finishedAt}`);
      lines.push(`app_version: ${appInfo?.appVersion ?? "-"}`);
      lines.push(`backend_version: ${appInfo?.backendVersion ?? "-"}`);
      lines.push(`update_channel: ${appInfo?.updateChannel ?? "stable"}`);
      lines.push(`tokens_seen: ${summary.tokensSeen}`);
      lines.push(`unique_tokens: ${summary.uniqueTokens}`);
      lines.push(`duplicates: ${summary.duplicates}`);
      lines.push(`reduction_pct: ${summary.reductionPct}`);
      lines.push(`uniq_pct: ${summary.uniqPct}`);
      lines.push(`elapsed_ms: ${summary.elapsedMs}`);
      lines.push(`avg_throughput_tps: ${summary.avgThroughputTps}`);
      lines.push(`output_path: ${summary.outputPath}`);
      lines.push(`output_bytes: ${summary.outputBytes}`);
      lines.push(`separator_raw: ${summary.outputSeparatorRaw}`);
      lines.push(`separator_preview: ${summary.outputSeparatorPreview}`);
      lines.push(`trim: ${summary.trim ? "on" : "off"}`);
      lines.push(`drop_empty: ${summary.dropEmpty ? "on" : "off"}`);
      if (summary.warnings.length > 0) {
        lines.push("warnings:");
        for (const warning of summary.warnings) {
          lines.push(`- ${warning}`);
        }
      }
      if (summary.errorMessage) {
        lines.push(`error_message: ${summary.errorMessage}`);
      }
      return lines.join("\n");
    },
    [appInfo?.appVersion, appInfo?.backendVersion, appInfo?.updateChannel],
  );

  const mergeInputs = React.useCallback((incoming: string[]) => {
    if (incoming.length === 0) {
      return;
    }
    setForm((prev) => {
      const merged = [...parseInputs(prev.inputsText), ...incoming];
      const deduped = Array.from(new Set(merged));
      return { ...prev, inputsText: deduped.join("\n") };
    });
  }, []);

  const syncRuntimeState = React.useCallback(async () => {
    const state = await invoke<RuntimeState>("get_runtime_state");
    dispatch({ type: "sync_runtime", isRunning: state.isRunning, activeJobId: state.activeJobId ?? null });
  }, [dispatch]);

  React.useEffect(() => {
    try {
      window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    } catch {
      // Ignore unavailable localStorage.
    }
  }, [locale]);

  // pollEvents only depends on dispatch (stable) — not on tr or appendMessage.
  // This means the 300ms polling interval is set once on mount and never restarted.
  const pollEvents = React.useCallback(async () => {
    if (pollingRef.current) {
      return;
    }
    pollingRef.current = true;
    try {
      const batch = await invoke<EmittedEvent[]>("next_events", NEXT_EVENTS_REQ);
      if (batch.length === 0) {
        return;
      }

      for (const ev of batch) {
        switch (ev.topic) {
          case "job://started": {
            const id = Number(ev.payload.job_id ?? 0);
            dispatch({
              type: "event_job_started",
              jobId: Number.isFinite(id) && id > 0 ? id : null,
              message: t(localeRef.current, "message.job_started"),
            });
            break;
          }
          case "job://stage": {
            dispatch({ type: "event_stage", stage: String(ev.payload.stage ?? "-") });
            break;
          }
          case "job://progress": {
            const p = ev.payload;
            const snapshot: Partial<ProgressSnapshot> = {};
            if (p.stage != null) snapshot.stage = String(p.stage);
            if (p.files_done != null) snapshot.filesDone = Number(p.files_done);
            if (p.files_total != null) snapshot.filesTotal = Number(p.files_total);
            if (p.tokens_seen != null) snapshot.tokensSeen = Number(p.tokens_seen);
            if (p.unique_tokens != null) snapshot.uniqueTokens = Number(p.unique_tokens);
            if (p.duplicates != null) snapshot.duplicates = Number(p.duplicates);
            if (p.throughput_tps != null) snapshot.throughputTps = Number(p.throughput_tps);
            if (p.elapsed_ms != null) snapshot.elapsedMs = Number(p.elapsed_ms);
            snapshot.etaMs =
              p.eta_ms === null || p.eta_ms === undefined ? null : Number(p.eta_ms);
            dispatch({ type: "event_progress", snapshot });
            break;
          }
          case "job://done": {
            dispatch({ type: "event_done", message: t(localeRef.current, "message.done") });
            break;
          }
          case "job://error": {
            dispatch({
              type: "event_error",
              message: t(localeRef.current, "message.error", {
                detail: String(ev.payload.message ?? t(localeRef.current, "fallback.unknown_error")),
              }),
            });
            break;
          }
          case "job://canceled": {
            dispatch({ type: "event_canceled", message: t(localeRef.current, "message.canceled") });
            break;
          }
          case "job://summary": {
            const raw = ev.payload.summary;
            if (!raw || typeof raw !== "object") {
              break;
            }
            const parsed = parseRunSummary(raw as Record<string, unknown>);
            if (!parsed) {
              break;
            }
            dispatch({
              type: "event_summary",
              summary: parsed,
              message: t(localeRef.current, "message.summary_ready"),
            });
            break;
          }
          default:
            break;
        }
      }
    } finally {
      pollingRef.current = false;
    }
  }, [dispatch]);

  React.useEffect(() => {
    void invoke<AppInfo>("get_app_info").then(setAppInfo).catch(() => null);
    void syncRuntimeState().catch(() => null);
    void resolveDefaultOutputPath()
      .then((path) => {
        if (!path) {
          return;
        }
        setForm((prev) => (prev.outputPath.trim().length > 0 ? prev : { ...prev, outputPath: path }));
      })
      .catch(() => null);
    const timer = window.setInterval(() => {
      void pollEvents();
    }, 300);
    return () => window.clearInterval(timer);
  }, [pollEvents, syncRuntimeState]);

  // ── Event handlers ──────────────────────────────────────────────────────────

  const startJob = React.useCallback(async () => {
    if (validationErrors.length > 0) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.cannot_start", { detail: validationErrors[0] }),
      });
      return;
    }

    let allowOverwrite = form.allowOverwrite;
    try {
      const exists = await invoke<boolean>("path_exists", { path: form.outputPath.trim() });
      if (exists && !allowOverwrite) {
        const shouldOverwrite = await confirm(
          t(localeRef.current, "confirm.overwrite.body", { path: form.outputPath }),
          { title: t(localeRef.current, "confirm.overwrite.title"), kind: "warning" },
        );
        if (!shouldOverwrite) {
          dispatch({
            type: "set_message",
            message: t(localeRef.current, "message.start_canceled_by_user"),
          });
          return;
        }
        allowOverwrite = true;
      }
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.preflight_failed", { detail: String(err) }),
      });
      return;
    }

    const req = {
      req: {
        config: {
          inputs: parsedInputs,
          output: form.outputPath.trim(),
          allowOverwrite,
          outputSeparator: form.separator,
          interpretSeparatorEscapes: !form.rawSeparator,
          mode: form.mode,
          ordering: form.ordering,
          trim: form.trim,
          dropEmpty: form.dropEmpty,
          diskBuckets: form.diskBuckets,
          diskAlphabeticalMode: form.diskAlphabeticalMode,
          diskRunBytes: form.diskRunBytes,
        },
      },
    };

    try {
      dispatch({ type: "job_starting", message: t(localeRef.current, "message.starting") });
      const res = await invoke<{ jobId: number }>("start_job", req);
      dispatch({ type: "job_invoke_ok", jobId: res.jobId });
    } catch (err) {
      dispatch({
        type: "job_invoke_err",
        message: t(localeRef.current, "message.start_failed", { detail: String(err) }),
      });
    }
  }, [validationErrors, form, parsedInputs, dispatch]);

  const cancelJob = React.useCallback(async () => {
    if (activeJobId === null) {
      return;
    }
    try {
      const res = await invoke<{ acknowledged: boolean }>("cancel_job", {
        req: { jobId: activeJobId },
      });
      if (res.acknowledged) {
        dispatch({ type: "set_message", message: t(localeRef.current, "message.cancel_requested") });
      }
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.cancel_failed", { detail: String(err) }),
      });
    }
  }, [activeJobId, dispatch]);

  const pickInputFiles = React.useCallback(async () => {
    try {
      const selected = await open({
        multiple: true,
        directory: false,
        filters: [
          { name: "All supported", extensions: ["txt", "csv", "tsv", "log", "pdf"] },
          { name: "PDF", extensions: ["pdf"] },
          { name: "Text / CSV", extensions: ["txt", "csv", "tsv", "log"] },
        ],
      });
      if (!selected) {
        return;
      }
      const list = Array.isArray(selected) ? selected : [selected];
      if (list.length === 0) {
        return;
      }
      mergeInputs(list);
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.input_dialog_failed", { detail: String(err) }),
      });
    }
  }, [mergeInputs, dispatch]);

  const pickInputFolder = React.useCallback(async () => {
    try {
      const selected = await open({ multiple: false, directory: true });
      if (!selected) return;
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (!path || path.trim().length === 0) return;
      mergeInputs([path]);
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.folder_dialog_failed", { detail: String(err) }),
      });
    }
  }, [mergeInputs, dispatch]);

  const onInputsDragOver = React.useCallback((event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    if (runStatus === "running") {
      return;
    }
    event.dataTransfer.dropEffect = "copy";
    setInputsDragActive(true);
  }, [runStatus]);

  const onInputsDragLeave = React.useCallback((event: React.DragEvent<HTMLDivElement>) => {
    const related = event.relatedTarget as Node | null;
    if (related && event.currentTarget.contains(related)) {
      return;
    }
    setInputsDragActive(false);
  }, []);

  const onInputsDrop = React.useCallback((event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    if (runStatus === "running") {
      return;
    }
    setInputsDragActive(false);
    mergeInputs(extractDroppedPaths(event));
  }, [runStatus, mergeInputs]);

  const pickOutputFile = React.useCallback(async () => {
    try {
      const selected = await save({
        defaultPath: form.outputPath.trim() || undefined,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!selected) {
        return;
      }
      setForm((prev) => ({ ...prev, outputPath: selected }));
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.output_dialog_failed", { detail: String(err) }),
      });
    }
  }, [form.outputPath, dispatch]);

  const openSummaryOutput = React.useCallback(async () => {
    if (!lastSummary) {
      return;
    }
    try {
      await invoke("open_output", { req: { path: lastSummary.outputPath } });
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.open_output_failed", { detail: String(err) }),
      });
    }
  }, [lastSummary, dispatch]);

  const openSummaryFolder = React.useCallback(async () => {
    if (!lastSummary) {
      return;
    }
    try {
      await invoke("open_output_folder", { req: { path: lastSummary.outputPath } });
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.open_output_folder_failed", { detail: String(err) }),
      });
    }
  }, [lastSummary, dispatch]);

  const copySummaryReport = React.useCallback(async () => {
    if (!lastSummary) {
      return;
    }
    try {
      await navigator.clipboard.writeText(buildSummaryReport(lastSummary));
      dispatch({ type: "set_message", message: t(localeRef.current, "message.report_copied") });
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.copy_report_failed", { detail: String(err) }),
      });
    }
  }, [lastSummary, buildSummaryReport, dispatch]);

  const exportSummaryJson = React.useCallback(async () => {
    if (!lastSummary) {
      return;
    }
    try {
      const defaultName = `run-summary-${lastSummary.jobId}.json`;
      const selected = await save({
        defaultPath: defaultName,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!selected) {
        return;
      }
      await invoke("export_summary_json", {
        req: {
          path: selected,
          content: JSON.stringify(
            {
              app: {
                name: appInfo?.appName ?? "Dupli-Annihilator-G",
                appVersion: appInfo?.appVersion ?? "-",
                backendVersion: appInfo?.backendVersion ?? "-",
                updateChannel: appInfo?.updateChannel ?? "stable",
              },
              summary: lastSummary,
            },
            null,
            2,
          ),
        },
      });
      dispatch({ type: "set_message", message: t(localeRef.current, "message.summary_exported") });
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.export_summary_failed", { detail: String(err) }),
      });
    }
  }, [lastSummary, appInfo, dispatch]);

  const closeSummaryReport = React.useCallback(() => {
    dispatch({ type: "reset_summary" });
  }, [dispatch]);

  // checkForUpdates uses localeRef so it doesn't depend on tr — stable as long as appVersion is stable.
  const checkForUpdates = React.useCallback(async () => {
    if (!appInfo?.appVersion) {
      return;
    }

    setUpdateStatus("checking");
    dispatch({ type: "set_message", message: t(localeRef.current, "message.update_checking") });

    try {
      const response = await fetch("https://api.github.com/repos/bultodepapas/Dupli-Annihilator-G/releases/latest", {
        headers: {
          Accept: "application/vnd.github+json",
        },
      });
      if (!response.ok) {
        throw new Error(`http_${response.status}`);
      }

      const release = (await response.json()) as GitHubRelease;
      if (release.draft || release.prerelease) {
        setUpdateStatus("up_to_date");
        setAvailableUpdate(null);
        dispatch({
          type: "set_message",
          message: t(localeRef.current, "message.update_up_to_date", { version: appInfo.appVersion }),
        });
        return;
      }

      const latest = parseSemver(release.tag_name);
      const current = parseSemver(appInfo.appVersion);
      if (!latest || !current) {
        throw new Error("invalid_semver");
      }

      if (compareSemver(latest, current) > 0) {
        const normalizedVersion = `${latest.major}.${latest.minor}.${latest.patch}`;
        const majorUpgrade = isMajorUpgrade(latest, current);
        setAvailableUpdate({
          version: normalizedVersion,
          tag: `v${normalizedVersion}`,
          url: release.html_url,
          isMajor: majorUpgrade,
        });
        setUpdateStatus("available");
        setUpdaterInstallReady(false);
        if (majorUpgrade) {
          dispatch({
            type: "set_message",
            message: t(localeRef.current, "message.update_major_manual_only", { version: normalizedVersion }),
          });
        } else {
          void (async () => {
            try {
              const update = await check();
              setUpdaterInstallReady(Boolean(update));
            } catch {
              setUpdaterInstallReady(false);
            }
          })();
          dispatch({
            type: "set_message",
            message: t(localeRef.current, "message.update_available", { version: normalizedVersion }),
          });
        }
      } else {
        setAvailableUpdate(null);
        setUpdateStatus("up_to_date");
        setUpdaterInstallReady(false);
        dispatch({
          type: "set_message",
          message: t(localeRef.current, "message.update_up_to_date", { version: appInfo.appVersion }),
        });
      }
    } catch (err) {
      setUpdateStatus("error");
      setUpdaterInstallReady(false);
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.update_check_failed", { detail: String(err) }),
      });
    }
  }, [appInfo?.appVersion, dispatch]);

  const openLatestReleasePage = React.useCallback(async () => {
    if (!availableUpdate) {
      return;
    }
    try {
      await invoke("open_external_url", { req: { url: availableUpdate.url } });
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.open_release_failed", { detail: String(err) }),
      });
    }
  }, [availableUpdate, dispatch]);

  const installAvailableUpdate = React.useCallback(async () => {
    if (!availableUpdate) {
      return;
    }
    if (availableUpdate.isMajor) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.update_major_manual_only", { version: availableUpdate.version }),
      });
      await openLatestReleasePage();
      return;
    }
    if (!updaterInstallReady) {
      dispatch({ type: "set_message", message: t(localeRef.current, "message.update_auto_unavailable") });
      await openLatestReleasePage();
      return;
    }
    if (runStatus === "running") {
      dispatch({ type: "set_message", message: t(localeRef.current, "message.update_install_blocked_running") });
      return;
    }

    try {
      setUpdateStatus("downloading");
      setUpdateDownloadPct(0);
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.update_installing", { version: availableUpdate.version }),
      });

      const update = await check();
      if (!update) {
        throw new Error("updater_no_payload");
      }

      let downloaded = 0;
      let contentLength = 0;
      await update.downloadAndInstall((event: UpdaterProgressEvent) => {
        switch (event.event) {
          case "Started":
            contentLength = Number(event.data.contentLength ?? 0);
            downloaded = 0;
            setUpdateDownloadPct(0);
            break;
          case "Progress":
            downloaded += Number(event.data.chunkLength ?? 0);
            if (contentLength > 0) {
              setUpdateDownloadPct(Math.min(100, (downloaded / contentLength) * 100));
            }
            break;
          case "Finished":
            setUpdateDownloadPct(100);
            break;
          default:
            break;
        }
      });

      setUpdateStatus("ready_to_restart");
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.update_ready_restart", { version: availableUpdate.version }),
      });
    } catch (err) {
      setUpdateStatus("error");
      setUpdateDownloadPct(null);
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.update_install_failed", { detail: String(err) }),
      });
      await openLatestReleasePage();
    }
  }, [availableUpdate, updaterInstallReady, runStatus, dispatch, openLatestReleasePage]);

  const restartToApplyUpdate = React.useCallback(async () => {
    try {
      dispatch({ type: "set_message", message: t(localeRef.current, "message.update_restarting") });
      await relaunch();
    } catch (err) {
      dispatch({
        type: "set_message",
        message: t(localeRef.current, "message.update_restart_failed", { detail: String(err) }),
      });
    }
  }, [dispatch]);

  const pickCheckerFile = React.useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "All supported", extensions: ["txt", "csv", "tsv", "log"] }],
      });
      if (!selected) return;
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (path) {
        setCheckerPath(path);
        setCheckerStatus("idle");
        setCheckerResult(null);
        setCheckerMessage("");
      }
    } catch (err) {
      setCheckerMessage(String(err));
    }
  }, []);

  const loadWordlist = React.useCallback(async () => {
    const path = checkerPath.trim();
    if (!path) return;
    setCheckerStatus("loading");
    setCheckerResult(null);
    setCheckerMessage("");
    try {
      const res = await invoke<{ wordCount: number }>("load_wordlist_for_checker", { path });
      setCheckerWordCount(res.wordCount);
      setCheckerStatus("ready");
      setCheckerMessage(t(localeRef.current, "checker.loaded", { count: res.wordCount }));
    } catch (err) {
      setCheckerStatus("error");
      setCheckerMessage(t(localeRef.current, "checker.load_failed", { detail: String(err) }));
    }
  }, [checkerPath]);

  const checkWord = React.useCallback(async () => {
    const word = checkerWord.trim();
    if (!word || checkerStatus !== "ready") return;
    try {
      const res = await invoke<{ found: boolean; wordCount: number }>("check_word", { word });
      setCheckerResult(res.found ? "found" : "not_found");
    } catch (err) {
      setCheckerMessage(t(localeRef.current, "checker.check_failed", { detail: String(err) }));
    }
  }, [checkerWord, checkerStatus]);

  const onLocaleChange = React.useCallback((l: Locale) => {
    setLocale(l);
  }, []);

  const onCheckForUpdates = React.useCallback(() => {
    void checkForUpdates();
  }, [checkForUpdates]);

  React.useEffect(() => {
    if (!appInfo?.appVersion || updateAutoCheckedRef.current) {
      return;
    }
    updateAutoCheckedRef.current = true;
    const timer = window.setTimeout(() => {
      void checkForUpdates();
    }, 1200);
    return () => window.clearTimeout(timer);
  }, [appInfo?.appVersion, checkForUpdates]);

  // ── Render ──────────────────────────────────────────────────────────────────

  return (
    <div className="app">
      <Header
        locale={locale}
        updateStatus={updateStatus}
        availableUpdate={availableUpdate}
        runStatus={runStatus}
        tr={tr}
        onLocaleChange={onLocaleChange}
        onCheckForUpdates={onCheckForUpdates}
      />

      {availableUpdate ? (
        <UpdateBanner
          availableUpdate={availableUpdate}
          updateStatus={updateStatus}
          updateDownloadPct={updateDownloadPct}
          appInfo={appInfo}
          updaterInstallReady={updaterInstallReady}
          tr={tr}
          onInstall={() => void installAvailableUpdate()}
          onOpenRelease={() => void openLatestReleasePage()}
          onCheckForUpdates={onCheckForUpdates}
          onRestart={() => void restartToApplyUpdate()}
        />
      ) : null}

      <main className={showSummaryScreen ? "summary-main" : "grid"}>
        {showSummaryScreen && lastSummary && summaryBadge ? (
          <SummaryScreen
            summary={lastSummary}
            summaryBadge={summaryBadge}
            summaryTopStages={summaryTopStages}
            summaryStageRows={summaryStageRows}
            appInfo={appInfo}
            canRun={canRun}
            tr={tr}
            onOpenOutput={() => void openSummaryOutput()}
            onOpenFolder={() => void openSummaryFolder()}
            onCopyReport={() => void copySummaryReport()}
            onExportJson={() => void exportSummaryJson()}
            onClose={closeSummaryReport}
            onRunAgain={() => void startJob()}
          />
        ) : (
          <>
            <InputSection
              form={form}
              runStatus={runStatus}
              inputsDragActive={inputsDragActive}
              tr={tr}
              onFormChange={setForm}
              onPickInputFiles={() => void pickInputFiles()}
              onPickInputFolder={() => void pickInputFolder()}
              onPickOutputFile={() => void pickOutputFile()}
              onDragOver={onInputsDragOver}
              onDragLeave={onInputsDragLeave}
              onDrop={onInputsDrop}
            />
            <ProcessingSection
              form={form}
              tr={tr}
              onFormChange={setForm}
            />
            <OutputSection
              form={form}
              runStatus={runStatus}
              activeJobId={activeJobId}
              appInfo={appInfo}
              canRun={canRun}
              canCancel={canCancel}
              separatorPreview={separatorPreview}
              separatorPreviewVisible={separatorPreviewVisible}
              resolvedSeparator={resolvedSeparator}
              tr={tr}
              onFormChange={setForm}
              onStartJob={() => void startJob()}
              onCancelJob={() => void cancelJob()}
            />
            <WordCheckerPanel
              checkerPath={checkerPath}
              checkerStatus={checkerStatus}
              checkerWordCount={checkerWordCount}
              checkerWord={checkerWord}
              checkerResult={checkerResult}
              checkerMessage={checkerMessage}
              tr={tr}
              onPathChange={setCheckerPath}
              onPickFile={() => void pickCheckerFile()}
              onLoad={() => void loadWordlist()}
              onWordChange={setCheckerWord}
              onCheck={() => void checkWord()}
            />
          </>
        )}
      </main>

      {showSummaryScreen ? null : (
        <TelemetryFooter
          progress={progress}
          validationErrors={validationErrors}
          runStatus={runStatus}
          message={message}
          tr={tr}
        />
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
