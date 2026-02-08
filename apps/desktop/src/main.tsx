import React from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { confirm, open, save } from "@tauri-apps/plugin-dialog";
import { isSupportedLocale, type I18nKey, type Locale, supportedLocales, t } from "./i18n";
import "./styles.css";

type AppInfo = {
  appName: string;
  appVersion: string;
  backendVersion: string;
};

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

function parseInputs(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
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
  const [progress, setProgress] = React.useState<ProgressSnapshot>({
    stage: "-",
    filesDone: 0,
    filesTotal: 0,
    tokensSeen: 0,
    uniqueTokens: 0,
    duplicates: 0,
    throughputTps: 0,
    elapsedMs: 0,
    etaMs: null,
  });

  const pollingRef = React.useRef(false);
  const parsedInputs = React.useMemo(() => parseInputs(form.inputsText), [form.inputsText]);
  const validationErrors = React.useMemo(() => validateForm(form, tr), [form, tr]);

  const appendMessage = React.useCallback((text: string) => {
    setMessage(text);
  }, []);

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
      setRunStatus("running");
      setMessage(tr("message.starting"));
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
      setForm((prev) => {
        const merged = [...parseInputs(prev.inputsText), ...list];
        const deduped = Array.from(new Set(merged));
        return { ...prev, inputsText: deduped.join("\n") };
      });
    } catch (err) {
      appendMessage(tr("message.input_dialog_failed", { detail: String(err) }));
    }
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
          <div className={`status-chip status-${runStatus}`}>{tr(statusKey(runStatus))}</div>
        </div>
      </header>

      <main className="grid">
        <section className="card">
          <h2>{tr("section.inputs")}</h2>
          <div className="button-row compact">
            <button className="secondary" disabled={runStatus === "running"} onClick={() => void pickInputFiles()}>
              {tr("button.add_files")}
            </button>
          </div>
          <label className="field">
            <span>{tr("field.inputs")}</span>
            <textarea
              value={form.inputsText}
              onChange={(e) => setForm((f) => ({ ...f, inputsText: e.target.value }))}
              rows={8}
              placeholder={tr("placeholder.inputs")}
            />
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
              {tr("button.run")}
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
              {tr("meta.job_id")}: {activeJobId ?? "-"}
            </div>
          </div>
        </section>
      </main>

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
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
