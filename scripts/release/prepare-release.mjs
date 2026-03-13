#!/usr/bin/env node
import { execSync } from "node:child_process";
import { resolve } from "node:path";

const args = process.argv.slice(2);
const targetVersion = args[0];
const flags = new Set(args.slice(1));

const allowDirty = flags.has("--allow-dirty");
const dryRun = flags.has("--dry-run");
const skipBuild = flags.has("--skip-build");
const skipTests = flags.has("--skip-tests");
const doCommit = flags.has("--commit");
const doTag = flags.has("--tag");
const doPush = flags.has("--push");

if (!targetVersion) {
  console.error(
    "Usage: node scripts/release/prepare-release.mjs <X.Y.Z> [--allow-dirty] [--dry-run] [--skip-build] [--skip-tests] [--commit] [--tag] [--push]",
  );
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+$/.test(targetVersion)) {
  console.error(`Invalid version '${targetVersion}'. Expected strict semver format X.Y.Z.`);
  process.exit(1);
}

if (doPush && !doTag) {
  console.error("--push requires --tag.");
  process.exit(1);
}

function quoteArg(arg) {
  if (/^[a-zA-Z0-9_./:-]+$/.test(arg)) {
    return arg;
  }
  return `"${arg.replace(/"/g, '\\"')}"`;
}

function run(cmd, options = {}) {
  const printable = cmd.map((arg) => quoteArg(arg)).join(" ");
  console.log(`$ ${printable}`);
  if (dryRun) {
    return;
  }
  execSync(printable, { stdio: "inherit", ...options });
}

function runCargoTests() {
  try {
    run(["cargo", "test", "--workspace", "--locked"]);
  } catch (err) {
    console.warn("cargo test --workspace --locked failed, retrying without --locked");
    run(["cargo", "test", "--workspace"]);
  }
}

if (!allowDirty) {
  const status = execSync("git status --porcelain", { encoding: "utf8" }).trim();
  if (status.length > 0) {
    console.error("Working tree is not clean. Commit/stash changes first, or use --allow-dirty.");
    process.exit(1);
  }
}

const bumpArgs = ["node", "scripts/release/bump-version.mjs", targetVersion];
if (allowDirty) {
  bumpArgs.push("--allow-dirty");
}
if (dryRun) {
  bumpArgs.push("--dry-run");
}
run(bumpArgs);

run(["node", "scripts/release/configure-updater.mjs"]);
run(["npm", "--prefix", "apps/desktop", "install", "--package-lock-only"]);
run(["cargo", "generate-lockfile"], { cwd: resolve(process.cwd(), "apps/desktop/src-tauri") });

if (!skipBuild) {
  run(["npm", "--prefix", "apps/desktop", "run", "build"]);
}

if (!skipTests) {
  runCargoTests();
}

const releaseFiles = [
  "Cargo.lock",
  "apps/desktop/package.json",
  "apps/desktop/package-lock.json",
  "apps/desktop/src-tauri/tauri.conf.json",
  "apps/desktop/src-tauri/Cargo.toml",
  "apps/desktop/src-tauri/Cargo.lock",
  "crates/core/Cargo.toml",
  "crates/job_runner/Cargo.toml",
  "crates/backend/Cargo.toml",
  "apps/cli/Cargo.toml",
  "README.md",
];

if (doCommit) {
  run(["git", "add", ...releaseFiles]);
  run(["git", "commit", "-m", `chore(release): prepare v${targetVersion}`]);
}

if (doTag) {
  run(["git", "tag", "-a", `v${targetVersion}`, "-m", `Release v${targetVersion}`]);
}

if (doPush) {
  run(["git", "push", "origin", "main"]);
  run(["git", "push", "origin", `v${targetVersion}`]);
}

console.log("Release preparation completed.");
console.log("Summary:");
console.log(`- version: ${targetVersion}`);
console.log(`- build: ${skipBuild ? "skipped" : "ok"}`);
console.log(`- tests: ${skipTests ? "skipped" : "ok"}`);
console.log(`- commit: ${doCommit ? "requested" : "not requested"}`);
console.log(`- tag: ${doTag ? "requested" : "not requested"}`);
console.log(`- push: ${doPush ? "requested" : "not requested"}`);
