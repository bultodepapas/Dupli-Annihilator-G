#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const dryRun = process.argv.includes("--dry-run");
const root = process.cwd();
const configPath = resolve(root, "apps/desktop/src-tauri/tauri.conf.json");

const updaterPubkey = (process.env.TAURI_UPDATER_PUBKEY ?? "").trim();
const updaterEndpoint = (process.env.TAURI_UPDATER_ENDPOINT ?? "").trim();
const signingKey = (process.env.TAURI_SIGNING_PRIVATE_KEY ?? "").trim();
const windowsCertificateThumbprint = (process.env.WINDOWS_CERTIFICATE_THUMBPRINT ?? "").trim();
const windowsTimestampUrl = (process.env.WINDOWS_TIMESTAMP_URL ?? "http://timestamp.digicert.com").trim();
const enableUpdater = updaterPubkey.length > 0 && signingKey.length > 0;

const fallbackEndpoint =
  "https://github.com/bultodepapas/Dupli-Annihilator-G/releases/latest/download/latest.json";
const endpoint = updaterEndpoint.length > 0 ? updaterEndpoint : fallbackEndpoint;

const raw = readFileSync(configPath, "utf8");
const parsed = JSON.parse(raw);

if (!parsed.bundle || typeof parsed.bundle !== "object") {
  parsed.bundle = {};
}
parsed.bundle.windows = parsed.bundle.windows ?? {};
parsed.bundle.windows.nsis = parsed.bundle.windows.nsis ?? {};
parsed.bundle.windows.nsis.installMode = "currentUser";
if (windowsCertificateThumbprint.length > 0) {
  parsed.bundle.windows.certificateThumbprint = windowsCertificateThumbprint;
  parsed.bundle.windows.digestAlgorithm = "sha256";
  parsed.bundle.windows.timestampUrl = windowsTimestampUrl;
} else {
  delete parsed.bundle.windows.certificateThumbprint;
  delete parsed.bundle.windows.digestAlgorithm;
  delete parsed.bundle.windows.timestampUrl;
}

if (enableUpdater) {
  parsed.bundle.createUpdaterArtifacts = true;
  parsed.plugins = parsed.plugins ?? {};
  parsed.plugins.updater = {
    active: true,
    endpoints: [endpoint],
    pubkey: updaterPubkey,
    windows: {
      installMode: "passive",
    },
  };
  console.log(`Updater enabled with endpoint: ${endpoint}`);
} else {
  if (parsed.bundle && Object.prototype.hasOwnProperty.call(parsed.bundle, "createUpdaterArtifacts")) {
    delete parsed.bundle.createUpdaterArtifacts;
  }
  // tauri-plugin-updater 2.10 requires the config object to exist AND for
  // `pubkey` (a required String field with no serde default) to be present,
  // even when the updater is inactive. Omitting pubkey causes a startup panic:
  // "Error deserializing 'plugins.updater': missing field `pubkey`".
  parsed.plugins = parsed.plugins ?? {};
  parsed.plugins.updater = {
    active: false,
    pubkey: "",
    windows: {
      installMode: "passive",
    },
  };
  console.log("Updater disabled (missing TAURI_UPDATER_PUBKEY and/or TAURI_SIGNING_PRIVATE_KEY).");
}

if (dryRun) {
  console.log("Dry run: no file changes written.");
  process.exit(0);
}

writeFileSync(configPath, `${JSON.stringify(parsed, null, 2)}\n`, "utf8");
console.log(`Updated ${configPath}`);
