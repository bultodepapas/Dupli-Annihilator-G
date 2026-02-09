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

function App() {
  const [locale, setLocale] = React.useState<Locale>(INITIAL_LOCALE);
  const tr = React.useCallback(
    (key: I18nKey, params?: Record<string, string | number>) => t(locale, key, params),
    [locale],
  );

  const [form, setForm] = React.useState<FormState>(DEFAULT_FORM);
  const [appInfo, setAppInfo] = React.useState<AppInfo | null>(null);
  const [runStatus, setRunStatus] = React.useState<RunStatus>("idle");
  const [activeJobId, setActiveJobId] = React.useState<number | null>(null);
  const [message, setMessage] = React.useState<string>(() => t(INITIAL_LOCALE, "message.idle"));
  const [progress, setProgress] = React.useState<ProgressSnapshot>(EMPTY_PROGRESS);
  const [inputsDragActive, setInputsDragActive] = React.useState(false);
  const [lastSummary, setLastSummary] = React.useState<RunSummary | null>(null);
  const [updateStatus, setUpdateStatus] = React.useState<UpdateStatus>("idle");
  const [availableUpdate, setAvailableUpdate] = React.useState<AvailableUpdate | null>(null);
  const [updateDownloadPct, setUpdateDownloadPct] = React.useState<number | null>(null);
  const [updaterInstallReady, setUpdaterInstallReady] = React.useState(false);

  const pollingRef = React.useRef(false);
  const updateAutoCheckedRef = React.useRef(false);
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

  const appendMessage = React.useCallback((text: string) => {
    setMessage(text);
  }, []);

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
  const syncRuntimeState = React.useCallback(async () => {
    const state = await invoke<RuntimeState>("get_runtime_state");
    setRunStatus(state.isRunning ? "running" : "idle");
    setActiveJobId(state.activeJobId ?? null);
  }, []);

  React.useEffect(() => {
    try {
      window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    } catch {
      // Ignore unavailable localStorage.
    }
  }, [locale]);

  const pollEvents = React.useCallback(async () => {
    if (pollingRef.current) {
      return;
    }
    pollingRef.current = true;
    try {
      const batch = await invoke<EmittedEvent[]>("next_events", {
        req: { maxEvents: 64, timeoutMs: 250 },
      });
      if (batch.length === 0) {
        return;
      }

      for (const ev of batch) {
        switch (ev.topic) {
          case "job://started": {
            setRunStatus("running");
            const id = Number(ev.payload.job_id ?? 0);
            if (Number.isFinite(id) && id > 0) {
              setActiveJobId(id);
            }
            appendMessage(tr("message.job_started"));
            break;
          }
          case "job://stage": {
            const stage = String(ev.payload.stage ?? "-");
            setProgress((prev) => ({ ...prev, stage }));
            break;
          }
          case "job://progress": {
            setProgress((prev) => ({
              ...prev,
              stage: String(ev.payload.stage ?? prev.stage),
              filesDone: Number(ev.payload.files_done ?? prev.filesDone),
              filesTotal: Number(ev.payload.files_total ?? prev.filesTotal),
              tokensSeen: Number(ev.payload.tokens_seen ?? prev.tokensSeen),
              uniqueTokens: Number(ev.payload.unique_tokens ?? prev.uniqueTokens),
              duplicates: Number(ev.payload.duplicates ?? prev.duplicates),
              throughputTps: Number(ev.payload.throughput_tps ?? prev.throughputTps),
              elapsedMs: Number(ev.payload.elapsed_ms ?? prev.elapsedMs),
              etaMs:
                ev.payload.eta_ms === null || ev.payload.eta_ms === undefined
                  ? null
                  : Number(ev.payload.eta_ms),
            }));
            break;
          }
          case "job://done": {
            setRunStatus("done");
            setActiveJobId(null);
            appendMessage(tr("message.done"));
            break;
          }
          case "job://error": {
            setRunStatus("error");
            setActiveJobId(null);
            appendMessage(
              tr("message.error", {
                detail: String(ev.payload.message ?? tr("fallback.unknown_error")),
              }),
            );
            break;
          }
          case "job://canceled": {
            setRunStatus("canceled");
            setActiveJobId(null);
            appendMessage(tr("message.canceled"));
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
            setLastSummary(parsed);
            appendMessage(tr("message.summary_ready"));
            break;
          }
          default:
            break;
        }
      }
    } finally {
      pollingRef.current = false;
    }
  }, [appendMessage, tr]);

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

  const canRun = runStatus !== "running" && validationErrors.length === 0;
  const canCancel = runStatus === "running" && activeJobId !== null;

  const startJob = async () => {
    if (validationErrors.length > 0) {
      setMessage(tr("message.cannot_start", { detail: validationErrors[0] }));
      return;
    }

    let allowOverwrite = form.allowOverwrite;
    try {
      const exists = await invoke<boolean>("path_exists", { path: form.outputPath.trim() });
      if (exists && !allowOverwrite) {
        const shouldOverwrite = await confirm(tr("confirm.overwrite.body", { path: form.outputPath }), {
          title: tr("confirm.overwrite.title"),
          kind: "warning",
        });
        if (!shouldOverwrite) {
          setRunStatus("idle");
          setMessage(tr("message.start_canceled_by_user"));
          return;
        }
        allowOverwrite = true;
      }
    } catch (err) {
      setMessage(tr("message.preflight_failed", { detail: String(err) }));
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
      setLastSummary(null);
      setRunStatus("running");
      setMessage(tr("message.starting"));
      setProgress(EMPTY_PROGRESS);
      const res = await invoke<{ jobId: number }>("start_job", req);
      setActiveJobId(res.jobId);
    } catch (err) {
      setRunStatus("error");
      setActiveJobId(null);
      setMessage(tr("message.start_failed", { detail: String(err) }));
    }
  };

  const cancelJob = async () => {
    if (activeJobId === null) {
      return;
    }
    try {
      const res = await invoke<{ acknowledged: boolean }>("cancel_job", {
        req: { jobId: activeJobId },
      });
      if (res.acknowledged) {
        appendMessage(tr("message.cancel_requested"));
      }
    } catch (err) {
      appendMessage(tr("message.cancel_failed", { detail: String(err) }));
    }
  };

  const pickInputFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        directory: false,
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
      appendMessage(tr("message.input_dialog_failed", { detail: String(err) }));
    }
  };

  const onInputsDragOver = (event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    if (runStatus === "running") {
      return;
    }
    event.dataTransfer.dropEffect = "copy";
    if (!inputsDragActive) {
      setInputsDragActive(true);
    }
  };

  const onInputsDragLeave = (event: React.DragEvent<HTMLDivElement>) => {
    const related = event.relatedTarget as Node | null;
    if (related && event.currentTarget.contains(related)) {
      return;
    }
    setInputsDragActive(false);
  };

  const onInputsDrop = (event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    if (runStatus === "running") {
      return;
    }
    setInputsDragActive(false);
    mergeInputs(extractDroppedPaths(event));
  };

  const pickOutputFile = async () => {
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
      appendMessage(tr("message.output_dialog_failed", { detail: String(err) }));
    }
  };

  const applySeparatorPreset = (preset: SeparatorPreset) => {
    setForm((prev) => ({
      ...prev,
      separator: preset.value,
      rawSeparator: preset.raw,
    }));
  };

  const openSummaryOutput = async () => {
    if (!lastSummary) {
      return;
    }
    try {
      await invoke("open_output", { req: { path: lastSummary.outputPath } });
    } catch (err) {
      appendMessage(tr("message.open_output_failed", { detail: String(err) }));
    }
  };

  const openSummaryFolder = async () => {
    if (!lastSummary) {
      return;
    }
    try {
      await invoke("open_output_folder", { req: { path: lastSummary.outputPath } });
    } catch (err) {
      appendMessage(tr("message.open_output_folder_failed", { detail: String(err) }));
    }
  };

  const copySummaryReport = async () => {
    if (!lastSummary) {
      return;
    }
    try {
      await navigator.clipboard.writeText(buildSummaryReport(lastSummary));
      appendMessage(tr("message.report_copied"));
    } catch (err) {
      appendMessage(tr("message.copy_report_failed", { detail: String(err) }));
    }
  };

  const exportSummaryJson = async () => {
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
      appendMessage(tr("message.summary_exported"));
    } catch (err) {
      appendMessage(tr("message.export_summary_failed", { detail: String(err) }));
    }
  };

  const closeSummaryReport = () => {
    setLastSummary(null);
  };

  const checkForUpdates = React.useCallback(async () => {
    if (!appInfo?.appVersion) {
      return;
    }

    setUpdateStatus("checking");
    appendMessage(tr("message.update_checking"));

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
        appendMessage(tr("message.update_up_to_date", { version: appInfo.appVersion }));
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
          appendMessage(tr("message.update_major_manual_only", { version: normalizedVersion }));
        } else {
          void (async () => {
            try {
              const update = await check();
              setUpdaterInstallReady(Boolean(update));
            } catch {
              setUpdaterInstallReady(false);
            }
          })();
          appendMessage(tr("message.update_available", { version: normalizedVersion }));
        }
      } else {
        setAvailableUpdate(null);
        setUpdateStatus("up_to_date");
        setUpdaterInstallReady(false);
        appendMessage(tr("message.update_up_to_date", { version: appInfo.appVersion }));
      }
    } catch (err) {
      setUpdateStatus("error");
      setUpdaterInstallReady(false);
      appendMessage(tr("message.update_check_failed", { detail: String(err) }));
    }
  }, [appInfo?.appVersion, appendMessage, tr]);

  const openLatestReleasePage = async () => {
    if (!availableUpdate) {
      return;
    }
    try {
      await invoke("open_external_url", { req: { url: availableUpdate.url } });
    } catch (err) {
      appendMessage(tr("message.open_release_failed", { detail: String(err) }));
    }
  };

  const installAvailableUpdate = async () => {
    if (!availableUpdate) {
      return;
    }
    if (availableUpdate.isMajor) {
      appendMessage(tr("message.update_major_manual_only", { version: availableUpdate.version }));
      await openLatestReleasePage();
      return;
    }
    if (!updaterInstallReady) {
      appendMessage(tr("message.update_auto_unavailable"));
      await openLatestReleasePage();
      return;
    }
    if (runStatus === "running") {
      appendMessage(tr("message.update_install_blocked_running"));
      return;
    }

    try {
      setUpdateStatus("downloading");
      setUpdateDownloadPct(0);
      appendMessage(tr("message.update_installing", { version: availableUpdate.version }));

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
      appendMessage(tr("message.update_ready_restart", { version: availableUpdate.version }));
    } catch (err) {
      setUpdateStatus("error");
      setUpdateDownloadPct(null);
      appendMessage(tr("message.update_install_failed", { detail: String(err) }));
      await openLatestReleasePage();
    }
  };

  const restartToApplyUpdate = async () => {
    try {
      appendMessage(tr("message.update_restarting"));
      await relaunch();
    } catch (err) {
      appendMessage(tr("message.update_restart_failed", { detail: String(err) }));
    }
  };

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

  const progressPercent =
    progress.filesTotal > 0 ? Math.min(100, (progress.filesDone / progress.filesTotal) * 100) : 0;

  return (
    <div className="app">
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
            onClick={() => void checkForUpdates()}
          >
            {updateStatus === "checking" ? tr("button.checking_updates") : tr("button.check_updates")}
          </button>
          <label className="lang-control">
            <span>{tr("field.language")}</span>
            <select value={locale} onChange={(e) => setLocale(e.target.value as Locale)}>
              {supportedLocales.map((loc) => (
                <option key={loc} value={loc}>
                  {t(loc, "lang.name")}
                </option>
              ))}
            </select>
          </label>
          {availableUpdate ? <div className="update-pill">{tr("update.available_pill", { version: availableUpdate.tag })}</div> : null}
          <div className={`status-chip status-${runStatus}`}>{tr(statusKey(runStatus))}</div>
        </div>
      </header>

      {availableUpdate ? (
        <section className="card update-banner">
          <h2>{tr("update.banner_title", { version: availableUpdate.tag })}</h2>
          <p>{tr("update.banner_body", { current: appInfo?.appVersion ?? "-", latest: availableUpdate.tag })}</p>
          {availableUpdate.isMajor ? <p>{tr("update.major_notice")}</p> : null}
          {updateStatus === "downloading" ? (
            <p>{tr("update.download_progress", { pct: Math.round(updateDownloadPct ?? 0) })}</p>
          ) : null}
          <div className="update-actions">
            {updateStatus === "ready_to_restart" ? (
              <button className="primary" type="button" onClick={() => void restartToApplyUpdate()}>
                {tr("button.restart_to_apply")}
              </button>
            ) : updaterInstallReady && !availableUpdate.isMajor ? (
              <button
                className="primary"
                type="button"
                disabled={updateStatus === "downloading"}
                onClick={() => void installAvailableUpdate()}
              >
                {updateStatus === "downloading" ? tr("button.installing_update") : tr("button.install_update")}
              </button>
            ) : null}
            <button
              className="secondary"
              type="button"
              disabled={updateStatus === "downloading"}
              onClick={() => void openLatestReleasePage()}
            >
              {tr("button.download_update")}
            </button>
            <button
              className="secondary"
              type="button"
              disabled={updateStatus === "checking" || updateStatus === "downloading"}
              onClick={() => void checkForUpdates()}
            >
              {tr("button.check_updates")}
            </button>
          </div>
        </section>
      ) : null}

      <main className={showSummaryScreen ? "summary-main" : "grid"}>
        {showSummaryScreen && lastSummary && summaryBadge ? (
          <section className="summary-shell">
            <header className="card summary-header">
              <div>
                <h2 className="summary-kicker">{tr("summary.mission_report")}</h2>
                <h3>{tr(summaryTitleKey(lastSummary.status))}</h3>
                <p className="subtitle">
                  {tr("summary.subline", {
                    jobId: lastSummary.jobId,
                    timestamp: formatIsoLocal(lastSummary.finishedAt),
                    mode: prettyMode(lastSummary),
                    ordering: lastSummary.ordering,
                  })}
                </p>
              </div>
              <div className={`summary-badge ${summaryBadge.cls}`}>{summaryBadge.text}</div>
            </header>

            <div className="summary-grid">
              <section className="card summary-card">
                <h3>{tr("summary.section.key_results")}</h3>
                <div className="summary-hero-value">{formatInt(lastSummary.uniqueTokens)}</div>
                <div className="summary-hero-label">{tr("summary.metric.unique_output")}</div>
                <div className="summary-kpis">
                  <div>
                    <strong>{formatInt(lastSummary.tokensSeen)}</strong>
                    <span>{tr("summary.metric.total_scanned")}</span>
                  </div>
                  <div>
                    <strong>{formatInt(lastSummary.duplicates)}</strong>
                    <span>{tr("summary.metric.duplicates_removed")}</span>
                  </div>
                  <div>
                    <strong>{formatPct(lastSummary.reductionPct)}</strong>
                    <span>{tr("summary.metric.reduction")}</span>
                  </div>
                  <div>
                    <strong>{formatPct(lastSummary.uniqPct)}</strong>
                    <span>{tr("summary.metric.uniq_rate")}</span>
                  </div>
                </div>
                <div className="summary-gauge-wrap">
                  <div className="summary-gauge-label">{tr("summary.metric.reduction_ratio")}</div>
                  <div className="bar-wrap">
                    <div className="bar" style={{ width: `${Math.min(100, Math.max(0, lastSummary.reductionPct))}%` }} />
                  </div>
                </div>

                <h4>{tr("summary.section.output")}</h4>
                <div className="summary-meta-grid">
                  <div>{tr("summary.metric.output_path")}</div>
                  <code>{lastSummary.outputPath || "-"}</code>
                  <div>{tr("summary.metric.output_bytes")}</div>
                  <div>{formatBytes(lastSummary.outputBytes)}</div>
                  <div>{tr("summary.metric.separator")}</div>
                  <div>
                    <code>{lastSummary.outputSeparatorRaw || "-"}</code> ({lastSummary.outputSeparatorPreview || "-"})
                  </div>
                </div>
              </section>

              <section className="card summary-card">
                <h3>{tr("summary.section.performance")}</h3>
                <div className="summary-meta-grid">
                  <div>{tr("summary.metric.elapsed")}</div>
                  <div>{formatElapsed(lastSummary.elapsedMs)}</div>
                  <div>{tr("summary.metric.avg_tps")}</div>
                  <div>{formatInt(lastSummary.avgThroughputTps)}</div>
                  <div>{tr("summary.metric.peak_tps")}</div>
                  <div>{lastSummary.peakThroughputTps ? formatInt(lastSummary.peakThroughputTps) : "-"}</div>
                  <div>{tr("summary.metric.input_bytes")}</div>
                  <div>{formatBytes(lastSummary.inputBytesTotal)}</div>
                  <div>{tr("summary.metric.mode_ordering")}</div>
                  <div>{prettyMode(lastSummary)} / {lastSummary.ordering}</div>
                  <div>{tr("summary.metric.normalization")}</div>
                  <div>
                    trim={lastSummary.trim ? "on" : "off"} | drop_empty={lastSummary.dropEmpty ? "on" : "off"}
                  </div>
                  <div>{tr("summary.metric.app_version")}</div>
                  <div>{appInfo?.appVersion ?? "-"}</div>
                  <div>{tr("summary.metric.backend_version")}</div>
                  <div>{appInfo?.backendVersion ?? "-"}</div>
                  <div>{tr("summary.metric.update_channel")}</div>
                  <div>{appInfo?.updateChannel ?? "stable"}</div>
                  {lastSummary.diskAlphabeticalMode ? (
                    <>
                      <div>{tr("summary.metric.disk_mode")}</div>
                      <div>{lastSummary.diskAlphabeticalMode}</div>
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
                {lastSummary.warnings.length > 0 ? (
                  <div className="warnings">
                    {lastSummary.warnings.map((warning) => (
                      <div key={warning}>{warning}</div>
                    ))}
                  </div>
                ) : (
                  <div className="summary-empty">{tr("summary.no_warnings")}</div>
                )}
                {lastSummary.errorMessage ? (
                  <div className="errors">
                    <div>{lastSummary.errorMessage}</div>
                  </div>
                ) : null}
              </section>
            </div>

            <footer className="card summary-footer">
              <div className="button-row">
                <button className="secondary" onClick={() => void openSummaryOutput()}>{tr("button.open_output")}</button>
                <button className="secondary" onClick={() => void openSummaryFolder()}>{tr("button.open_folder")}</button>
                <button className="secondary" onClick={() => void copySummaryReport()}>{tr("button.copy_report")}</button>
                <button className="secondary" onClick={() => void exportSummaryJson()}>{tr("button.export_json")}</button>
                <button className="secondary" onClick={closeSummaryReport}>{tr("button.close_report")}</button>
                <button className="primary" disabled={!canRun} onClick={() => void startJob()}>{tr("button.run_again")}</button>
              </div>
            </footer>
          </section>
        ) : (
          <>
            <section className="card">
              <h2>{tr("section.inputs")}</h2>
              <div className="button-row compact">
                <button className="secondary" disabled={runStatus === "running"} onClick={() => void pickInputFiles()}>
                  {tr("button.add_files")}
                </button>
              </div>
              <label className="field">
                <span>{tr("field.inputs")}</span>
                <div
                  className={`drop-zone ${inputsDragActive ? "drop-zone-active" : ""}`}
                  onDragOver={onInputsDragOver}
                  onDragLeave={onInputsDragLeave}
                  onDrop={onInputsDrop}
                >
                  <textarea
                    value={form.inputsText}
                    onChange={(e) => setForm((f) => ({ ...f, inputsText: e.target.value }))}
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
                  onChange={(e) => setForm((f) => ({ ...f, outputPath: e.target.value }))}
                  placeholder={tr("placeholder.output")}
                />
              </label>
              <div className="button-row compact">
                <button className="secondary" disabled={runStatus === "running"} onClick={() => void pickOutputFile()}>
                  {tr("button.pick_output")}
                </button>
              </div>
            </section>

            <section className="card">
              <h2>{tr("section.processing")}</h2>
              <div className="row">
                <label className="field" title={tr("tooltip.processing.mode")}>
                  <span title={tr("tooltip.processing.mode")}>{tr("field.mode")}</span>
                  <select
                    title={tr("tooltip.processing.mode")}
                    value={form.mode}
                    onChange={(e) => setForm((f) => ({ ...f, mode: e.target.value as FormState["mode"] }))}
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
                      setForm((f) => ({ ...f, ordering: e.target.value as FormState["ordering"] }))
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
                    setForm((f) => ({
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
                    onChange={(e) => setForm((f) => ({ ...f, diskBuckets: Number(e.target.value) }))}
                  />
                </label>
                <label className="field" title={tr("tooltip.processing.disk_run_bytes")}>
                  <span title={tr("tooltip.processing.disk_run_bytes")}>{tr("field.disk_run_bytes")}</span>
                  <input
                    title={tr("tooltip.processing.disk_run_bytes")}
                    type="number"
                    min={1_000_000}
                    value={form.diskRunBytes}
                    onChange={(e) => setForm((f) => ({ ...f, diskRunBytes: Number(e.target.value) }))}
                  />
                </label>
              </div>

              <div className="row flags">
                <label title={tr("tooltip.processing.trim")}>
                  <input
                    title={tr("tooltip.processing.trim")}
                    type="checkbox"
                    checked={form.trim}
                    onChange={(e) => setForm((f) => ({ ...f, trim: e.target.checked }))}
                  />
                  {tr("flag.trim")}
                </label>
                <label title={tr("tooltip.processing.drop_empty")}>
                  <input
                    title={tr("tooltip.processing.drop_empty")}
                    type="checkbox"
                    checked={form.dropEmpty}
                    onChange={(e) => setForm((f) => ({ ...f, dropEmpty: e.target.checked }))}
                  />
                  {tr("flag.drop_empty")}
                </label>
              </div>
            </section>

            <section className="card">
              <h2>{tr("section.output")}</h2>
              <label className="field">
                <span>{tr("field.separator")}</span>
                <input
                  value={form.separator}
                  onChange={(e) => setForm((f) => ({ ...f, separator: e.target.value }))}
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
                      onClick={() => applySeparatorPreset(preset)}
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
                  onChange={(e) => setForm((f) => ({ ...f, rawSeparator: e.target.checked }))}
                />
                <span>{tr("field.raw_separator")}</span>
              </label>
              <label className="field checkbox">
                <input
                  type="checkbox"
                  checked={form.allowOverwrite}
                  onChange={(e) => setForm((f) => ({ ...f, allowOverwrite: e.target.checked }))}
                />
                <span>{tr("field.allow_overwrite")}</span>
              </label>

              <div className="button-row">
                <button className="primary" disabled={!canRun} onClick={() => void startJob()}>
                  {tr(runButtonKey(runStatus))}
                </button>
                <button className="danger" disabled={!canCancel} onClick={() => void cancelJob()}>
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
          </>
        )}
      </main>

      {showSummaryScreen ? null : (
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
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
