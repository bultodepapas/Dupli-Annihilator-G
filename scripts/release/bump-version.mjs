#!/usr/bin/env node
import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = process.cwd();
const args = process.argv.slice(2);
const targetVersion = args[0];
const allowDirty = args.includes("--allow-dirty");
const dryRun = args.includes("--dry-run");

if (!targetVersion) {
  console.error("Usage: node scripts/release/bump-version.mjs <X.Y.Z> [--allow-dirty] [--dry-run]");
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+$/.test(targetVersion)) {
  console.error(`Invalid version '${targetVersion}'. Expected strict semver format X.Y.Z.`);
  process.exit(1);
}

if (!allowDirty) {
  const status = execSync("git status --porcelain", { encoding: "utf8" }).trim();
  if (status.length > 0) {
    console.error("Working tree is not clean. Commit or stash changes first, or use --allow-dirty.");
    process.exit(1);
  }
}

const jsonFiles = [
  "apps/desktop/package.json",
  "apps/desktop/package-lock.json",
  "apps/desktop/src-tauri/tauri.conf.json",
];

const tomlFiles = [
  "apps/desktop/src-tauri/Cargo.toml",
  "crates/core/Cargo.toml",
  "crates/job_runner/Cargo.toml",
  "crates/backend/Cargo.toml",
  "apps/cli/Cargo.toml",
];

function read(file) {
  return readFileSync(resolve(ROOT, file), "utf8");
}

function write(file, content) {
  if (dryRun) {
    return;
  }
  writeFileSync(resolve(ROOT, file), content, "utf8");
}

function updateJsonVersion(file) {
  const raw = read(file);
  const parsed = JSON.parse(raw);
  parsed.version = targetVersion;
  if (file.endsWith("package-lock.json") && parsed.packages?.[""]) {
    parsed.packages[""].version = targetVersion;
  }
  write(file, `${JSON.stringify(parsed, null, 2)}\n`);
}

function updateTomlVersion(file) {
  const raw = read(file);
  const match = raw.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error(`Could not update version in ${file}`);
  }
  const current = match[1];
  if (current === targetVersion) {
    return;
  }
  const next = raw.replace(/^version\s*=\s*"[^"]+"/m, `version = "${targetVersion}"`);
  write(file, next);
}

for (const file of jsonFiles) {
  updateJsonVersion(file);
}
for (const file of tomlFiles) {
  updateTomlVersion(file);
}

const readmePath = "README.md";
const readmeRaw = read(readmePath);
const readmeNext = readmeRaw.replace(/example: `v\d+\.\d+\.\d+`/g, `example: \`v${targetVersion}\``);
if (readmeNext !== readmeRaw) {
  write(readmePath, readmeNext);
}

console.log(`${dryRun ? "Dry run completed" : "Version bump completed"}: ${targetVersion}`);
console.log("Updated files:");
for (const file of [...jsonFiles, ...tomlFiles, readmePath]) {
  console.log(`- ${file}`);
}
console.log("Next steps:");
console.log("1) npm --prefix apps/desktop install --package-lock-only");
console.log("2) cargo test --workspace --locked || cargo test --workspace");
console.log(`3) git commit -am "chore(release): prepare v${targetVersion}"`);
console.log(`4) git tag -a v${targetVersion} -m "Release v${targetVersion}"`);
