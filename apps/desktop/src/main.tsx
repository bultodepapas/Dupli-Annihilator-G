import React from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
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

function parseInputs(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

function validateForm(form: FormState): string[] {
  const errors: string[] = [];
  const inputs = parseInputs(form.inputsText);
  if (inputs.length === 0) {
    errors.push("At least one input file is required.");
  }
  if (form.outputPath.trim().length === 0) {
    errors.push("Output file path is required.");
  }
  if (form.separator.length === 0) {
    errors.push("Separator cannot be empty.");
  }
  if (form.mode === "disk") {
    if (!Number.isFinite(form.diskBuckets) || form.diskBuckets < 8) {
      errors.push("Disk buckets must be >= 8.");
    }
    if (!Number.isFinite(form.diskRunBytes) || form.diskRunBytes < 1_000_000) {
      errors.push("Disk run bytes must be >= 1,000,000.");
    }
  }
  return errors;
}

function App() {
  const [form, setForm] = React.useState<FormState>(DEFAULT_FORM);
  const [appInfo, setAppInfo] = React.useState<AppInfo | null>(null);
  const [runStatus, setRunStatus] = React.useState<RunStatus>("idle");
  const [activeJobId, setActiveJobId] = React.useState<number | null>(null);
  const [message, setMessage] = React.useState<string>("Idle");
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
  const validationErrors = React.useMemo(() => validateForm(form), [form]);

  const appendMessage = React.useCallback((text: string) => {
    setMessage(text);
  }, []);

  const syncRuntimeState = React.useCallback(async () => {
    const state = await invoke<RuntimeState>("get_runtime_state");
    setRunStatus(state.isRunning ? "running" : "idle");
    setActiveJobId(state.activeJobId ?? null);
  }, []);

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
            appendMessage("Job started");
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
            appendMessage("Done");
            break;
          }
          case "job://error": {
            setRunStatus("error");
            setActiveJobId(null);
            appendMessage(`Error: ${String(ev.payload.message ?? "unknown error")}`);
            break;
          }
          case "job://canceled": {
            setRunStatus("canceled");
            setActiveJobId(null);
            appendMessage("Canceled");
            break;
          }
          default:
            break;
        }
      }
    } finally {
      pollingRef.current = false;
    }
  }, [appendMessage]);

  React.useEffect(() => {
    void invoke<AppInfo>("get_app_info").then(setAppInfo).catch(() => null);
    void syncRuntimeState().catch(() => null);
    const timer = window.setInterval(() => {
      void pollEvents();
    }, 300);
    return () => window.clearInterval(timer);
  }, [pollEvents, syncRuntimeState]);

  const canRun = runStatus !== "running" && validationErrors.length === 0;
  const canCancel = runStatus === "running" && activeJobId !== null;

  const startJob = async () => {
    if (validationErrors.length > 0) {
      setMessage(`Cannot start: ${validationErrors[0]}`);
      return;
    }

    const req = {
      req: {
        config: {
          inputs: parsedInputs,
          output: form.outputPath.trim(),
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
      setMessage("Starting job...");
      const res = await invoke<{ jobId: number }>("start_job", req);
      setActiveJobId(res.jobId);
    } catch (err) {
      setRunStatus("error");
      setActiveJobId(null);
      setMessage(`Start failed: ${String(err)}`);
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
        appendMessage("Cancellation requested");
      }
    } catch (err) {
      appendMessage(`Cancel failed: ${String(err)}`);
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
      appendMessage(`Input dialog failed: ${String(err)}`);
    }
  };

  const pickOutputFile = async () => {
    try {
      const selected = await save({});
      if (!selected) {
        return;
      }
      setForm((prev) => ({ ...prev, outputPath: selected }));
    } catch (err) {
      appendMessage(`Output dialog failed: ${String(err)}`);
    }
  };

  const progressPercent =
    progress.filesTotal > 0 ? Math.min(100, (progress.filesDone / progress.filesTotal) * 100) : 0;

  return (
    <div className="app">
      <header className="topbar">
        <div>
          <h1>Dupli-Annihilator-G</h1>
          <p className="subtitle">Tauri Desktop Control Panel</p>
        </div>
        <div className={`status-chip status-${runStatus}`}>{runStatus.toUpperCase()}</div>
      </header>

      <main className="grid">
        <section className="card">
          <h2>Inputs</h2>
          <div className="button-row compact">
            <button className="secondary" disabled={runStatus === "running"} onClick={() => void pickInputFiles()}>
              ADD FILES
            </button>
          </div>
          <label className="field">
            <span>Input files (one absolute path per line)</span>
            <textarea
              value={form.inputsText}
              onChange={(e) => setForm((f) => ({ ...f, inputsText: e.target.value }))}
              rows={8}
              placeholder={"C:\\data\\in1.txt\nC:\\data\\in2.txt"}
            />
          </label>
          <label className="field">
            <span>Output file path</span>
            <input
              value={form.outputPath}
              onChange={(e) => setForm((f) => ({ ...f, outputPath: e.target.value }))}
              placeholder={"C:\\data\\out.txt"}
            />
          </label>
          <div className="button-row compact">
            <button className="secondary" disabled={runStatus === "running"} onClick={() => void pickOutputFile()}>
              PICK OUTPUT
            </button>
          </div>
        </section>

        <section className="card">
          <h2>Processing</h2>
          <div className="row">
            <label className="field">
              <span>Mode</span>
              <select
                value={form.mode}
                onChange={(e) => setForm((f) => ({ ...f, mode: e.target.value as FormState["mode"] }))}
              >
                <option value="auto">auto</option>
                <option value="ram">ram</option>
                <option value="disk">disk</option>
              </select>
            </label>
            <label className="field">
              <span>Ordering</span>
              <select
                value={form.ordering}
                onChange={(e) =>
                  setForm((f) => ({ ...f, ordering: e.target.value as FormState["ordering"] }))
                }
              >
                <option value="preserve_first_seen">preserve_first_seen</option>
                <option value="alphabetical">alphabetical</option>
                <option value="unordered_fast">unordered_fast</option>
              </select>
            </label>
          </div>

          <label className="field">
            <span>Disk alphabetical mode</span>
            <select
              value={form.diskAlphabeticalMode}
              onChange={(e) =>
                setForm((f) => ({
                  ...f,
                  diskAlphabeticalMode: e.target.value as FormState["diskAlphabeticalMode"],
                }))
              }
            >
              <option value="fast_bucket_local">fast_bucket_local</option>
              <option value="global_perfect">global_perfect</option>
            </select>
          </label>

          <div className="row">
            <label className="field">
              <span>Disk buckets</span>
              <input
                type="number"
                min={8}
                value={form.diskBuckets}
                onChange={(e) => setForm((f) => ({ ...f, diskBuckets: Number(e.target.value) }))}
              />
            </label>
            <label className="field">
              <span>Disk run bytes</span>
              <input
                type="number"
                min={1_000_000}
                value={form.diskRunBytes}
                onChange={(e) => setForm((f) => ({ ...f, diskRunBytes: Number(e.target.value) }))}
              />
            </label>
          </div>

          <div className="row flags">
            <label>
              <input
                type="checkbox"
                checked={form.trim}
                onChange={(e) => setForm((f) => ({ ...f, trim: e.target.checked }))}
              />
              trim
            </label>
            <label>
              <input
                type="checkbox"
                checked={form.dropEmpty}
                onChange={(e) => setForm((f) => ({ ...f, dropEmpty: e.target.checked }))}
              />
              drop_empty
            </label>
          </div>
        </section>

        <section className="card">
          <h2>Output</h2>
          <label className="field">
            <span>Separator</span>
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
            <span>Raw separator (do not parse escapes)</span>
          </label>

          <div className="button-row">
            <button className="primary" disabled={!canRun} onClick={() => void startJob()}>
              RUN
            </button>
            <button className="danger" disabled={!canCancel} onClick={() => void cancelJob()}>
              CANCEL
            </button>
          </div>

          <div className="meta">
            <div>
              app: {appInfo?.appName ?? "-"} {appInfo?.appVersion ?? ""}
            </div>
            <div>backend: {appInfo?.backendVersion ?? "-"}</div>
            <div>job_id: {activeJobId ?? "-"}</div>
          </div>
        </section>
      </main>

      <footer className="telemetry card">
        <h2>Telemetry</h2>
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
          <div>stage: {progress.stage}</div>
          <div>
            files: {progress.filesDone}/{progress.filesTotal}
          </div>
          <div>tokens: {progress.tokensSeen}</div>
          <div>unique: {progress.uniqueTokens}</div>
          <div>duplicates: {progress.duplicates}</div>
          <div>tps: {progress.throughputTps}</div>
          <div>elapsed_ms: {progress.elapsedMs}</div>
          <div>eta_ms: {progress.etaMs ?? "-"}</div>
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
