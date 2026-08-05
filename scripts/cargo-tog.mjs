#!/usr/bin/env node
/**
 * cargo-tog — small CLI for polyrepo Cargo cache/dependency coordination.
 *
 *   node scripts/cargo-tog.mjs doctor
 *   node scripts/cargo-tog.mjs cache-plan
 *   node scripts/cargo-tog.mjs inventory --root <path>
 *   node scripts/cargo-tog.mjs dep-drift --master <path> --other <path>
 *   node scripts/cargo-tog.mjs lock-fingerprint --root <path>
 */

import { createHash } from "node:crypto";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  realpathSync,
} from "node:fs";
import { join, relative, resolve } from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { homedir } from "node:os";

const args = process.argv.slice(2);
const cmd = args[0] || "help";

function flag(name) {
  const i = args.indexOf(name);
  if (i === -1) return null;
  return args[i + 1] ?? "";
}

function has(name) {
  return args.includes(name);
}

function expandHome(p) {
  if (!p) return p;
  if (p.startsWith("~/")) return join(homedir(), p.slice(2));
  return p;
}

function walkFiles(root, pred, acc = []) {
  let entries;
  try {
    entries = readdirSync(root, { withFileTypes: true });
  } catch {
    return acc;
  }
  for (const ent of entries) {
    const name = ent.name;
    if (name === "target" || name === "node_modules" || name === ".git") continue;
    const full = join(root, name);
    if (ent.isDirectory()) walkFiles(full, pred, acc);
    else if (ent.isFile() && pred(full, name)) acc.push(full);
  }
  return acc;
}

/** Minimal TOML extraction — good enough for inventory / drift (not a full parser). */
function parseCargoTomlRough(text) {
  const out = {
    packageName: null,
    packageVersion: null,
    versionWorkspace: false,
    isWorkspace: false,
    members: [],
    deps: {},
    workspaceDeps: {},
    workspacePackageVersion: null,
  };
  let section = "";
  let inMembersArray = false;

  for (const rawLine of text.split("\n")) {
    const line = rawLine.replace(/#.*$/, "").trim();
    if (!line) continue;
    const sec = line.match(/^\[([^\]]+)\]$/);
    if (sec) {
      section = sec[1].trim();
      inMembersArray = false;
      continue;
    }

    if (section === "package") {
      const m = line.match(/^name\s*=\s*"([^"]+)"/);
      if (m) out.packageName = m[1];
      const v = line.match(/^version\s*=\s*"([^"]+)"/);
      if (v) out.packageVersion = v[1];
      if (/^version\.workspace\s*=\s*true/.test(line) || /^version\s*=\s*\{\s*workspace\s*=\s*true/.test(line)) {
        out.versionWorkspace = true;
      }
    }

    if (section === "workspace.package") {
      const v = line.match(/^version\s*=\s*"([^"]+)"/);
      if (v) out.workspacePackageVersion = v[1];
    }

    if (section === "workspace") {
      out.isWorkspace = true;
      if (inMembersArray || /^members\s*=/.test(line)) {
        if (/^members\s*=\s*\[/.test(line) && !line.includes("]")) inMembersArray = true;
        for (const q of line.matchAll(/"([^"]+)"/g)) out.members.push(q[1]);
        if (line.includes("]")) inMembersArray = false;
      }
    }

    if (
      section === "dependencies" ||
      section === "dev-dependencies" ||
      section === "build-dependencies"
    ) {
      const simple = line.match(/^([A-Za-z0-9_-]+)\s*=\s*"([^"]+)"/);
      if (simple) {
        out.deps[simple[1]] = simple[2];
        continue;
      }
      const ver = line.match(/^([A-Za-z0-9_-]+)\s*=\s*\{[^}]*version\s*=\s*"([^"]+)"/);
      if (ver) out.deps[ver[1]] = ver[2];
      if (/workspace\s*=\s*true/.test(line)) {
        const name = line.match(/^([A-Za-z0-9_-]+)\s*=/);
        if (name) out.deps[name[1]] = out.deps[name[1]] || "workspace";
      }
    }

    if (section === "workspace.dependencies") {
      const simple = line.match(/^([A-Za-z0-9_-]+)\s*=\s*"([^"]+)"/);
      if (simple) {
        out.workspaceDeps[simple[1]] = simple[2];
        continue;
      }
      const ver = line.match(/^([A-Za-z0-9_-]+)\s*=\s*\{[^}]*version\s*=\s*"([^"]+)"/);
      if (ver) out.workspaceDeps[ver[1]] = ver[2];
    }
  }
  return out;
}

function resolvePackageVersion(parsed, workspacePackageVersion) {
  if (parsed.packageVersion) return parsed.packageVersion;
  if (parsed.versionWorkspace) {
    return workspacePackageVersion || "workspace";
  }
  return "?";
}

function cmdHelp() {
  console.log(`cargo-tog — coordinate Cargo caches & deps across polyrepos

Usage:
  cargo-tog doctor
  cargo-tog cache-plan
  cargo-tog inventory --root <path>
  cargo-tog dep-drift --master <path> --other <path>
  cargo-tog lock-fingerprint --root <path>

See README.md and docs/LAYERS.md.`);
}

function cmdDoctor() {
  console.log("cargo-tog doctor\n");
  const cargo = spawnSync("cargo", ["--version"], { encoding: "utf8" });
  console.log(cargo.status === 0 ? `cargo: ${cargo.stdout.trim()}` : "cargo: NOT FOUND");
  const rustc = spawnSync("rustc", ["--version"], { encoding: "utf8" });
  console.log(rustc.status === 0 ? `rustc: ${rustc.stdout.trim()}` : "rustc: NOT FOUND");
  const sc = spawnSync("sccache", ["--version"], { encoding: "utf8" });
  console.log(sc.status === 0 ? `sccache: ${sc.stdout.trim()}` : "sccache: not installed (optional but recommended)");

  const wrapper = process.env.RUSTC_WRAPPER || "(unset)";
  console.log(`RUSTC_WRAPPER: ${wrapper}`);
  console.log(`CARGO_HOME: ${process.env.CARGO_HOME || join(homedir(), ".cargo") + " (default)"}`);
  console.log(`CARGO_TARGET_DIR: ${process.env.CARGO_TARGET_DIR || "(unset — per-project ./target)"}`);
  console.log(`SCCACHE_DIR: ${process.env.SCCACHE_DIR || "(sccache default)"}`);
  console.log(`SCCACHE_BUCKET: ${process.env.SCCACHE_BUCKET || "(unset — no remote)"}`);

  if (process.env.CARGO_TARGET_DIR) {
    console.log(
      "\nwarn: CARGO_TARGET_DIR is set globally. Use only for one workspace checkout, not all polyrepos.",
    );
  }
  if (wrapper !== "sccache" && sc.status === 0) {
    console.log("\nhint: sccache is installed but RUSTC_WRAPPER is not sccache.");
  }
  console.log("\nShare: CARGO_HOME + sccache. Do not share target/ across different workspaces.");
}

function cmdCachePlan() {
  console.log(`# cargo-tog cache plan
#
# SHARE (any multi-project / polyrepo setup)
# -----------------------------------------
# 1. CARGO_HOME registry + git          (downloads)
# 2. sccache + remote S3/R2 when ready  (compiler objects across repos)
# 3. Optional: one tree owns dependency pins; dep-drift the rest
#
# DO NOT SHARE
# ------------
# 1. One CARGO_TARGET_DIR for unrelated workspaces
# 2. Full target/ upload to GitHub Actions when using sccache
# 3. cargo-chef layers across different apps (Docker only)
#
# Laptop
# ------
export RUSTC_WRAPPER=sccache
export SCCACHE_DIR="$HOME/.cache/sccache"
# export CARGO_HOME="$HOME/.cargo"
#
# CI
# --
# - cargo-tog Action OR sccache-action + rust-cache (cache-targets: false)
# - CARGO_PROFILE_DEV_DEBUG=0  CARGO_INCREMENTAL=0
# - cargo test / cargo nextest / cargo bench all hit the same sccache
# - set SCCACHE_BUCKET (+ endpoint/keys) for org-wide reuse
`);
}

function cmdInventory() {
  const root = resolve(expandHome(flag("--root") || "."));
  if (!existsSync(root)) {
    console.error(`root not found: ${root}`);
    process.exit(1);
  }
  const tomls = walkFiles(root, (_, name) => name === "Cargo.toml");
  console.log(`inventory: ${root}`);
  console.log(`Cargo.toml files: ${tomls.length}\n`);

  const workspaceDeps = {};
  const packages = [];
  let workspacePackageVersion = null;
  let rootMembers = [];
  const parsedFiles = tomls.map((file) => ({
    file,
    rel: relative(root, file),
    parsed: parseCargoTomlRough(readFileSync(file, "utf8")),
  }));

  for (const { rel, parsed } of parsedFiles) {
    if (parsed.workspacePackageVersion) {
      workspacePackageVersion = parsed.workspacePackageVersion;
    }
    Object.assign(workspaceDeps, parsed.workspaceDeps);
    if (parsed.isWorkspace && parsed.members.length) {
      rootMembers = parsed.members;
      console.log(`[workspace] ${rel} members=${parsed.members.length}`);
    }
  }

  for (const { rel, parsed } of parsedFiles) {
    if (!parsed.packageName) continue;
    packages.push({
      path: rel,
      name: parsed.packageName,
      version: resolvePackageVersion(parsed, workspacePackageVersion),
      deps: parsed.deps,
    });
  }

  console.log("packages:");
  for (const p of packages.sort((a, b) => a.name.localeCompare(b.name))) {
    console.log(`  ${p.name}@${p.version || "?"}  (${p.path})`);
  }
  if (rootMembers.length) {
    console.log(`\nworkspace members declared: ${rootMembers.length}`);
  }

  const wd = Object.keys(workspaceDeps).sort();
  if (wd.length) {
    console.log(`\nworkspace.dependencies (${wd.length}):`);
    for (const k of wd) console.log(`  ${k} = ${workspaceDeps[k]}`);
  }
}

function collectExplicitDeps(root) {
  const tomls = walkFiles(root, (_, name) => name === "Cargo.toml");
  /** @type {Map<string, Set<string>>} */
  const map = new Map();
  for (const file of tomls) {
    const parsed = parseCargoTomlRough(readFileSync(file, "utf8"));
    const all = { ...parsed.workspaceDeps, ...parsed.deps };
    for (const [name, ver] of Object.entries(all)) {
      if (ver === "workspace") continue;
      if (!map.has(name)) map.set(name, new Set());
      map.get(name).add(ver);
    }
  }
  return map;
}

function cmdDepDrift() {
  const master = resolve(expandHome(flag("--master") || ""));
  const other = resolve(expandHome(flag("--other") || ""));
  if (!master || !other || !existsSync(master) || !existsSync(other)) {
    console.error("usage: cargo-tog dep-drift --master <path> --other <path>");
    process.exit(1);
  }
  const a = collectExplicitDeps(master);
  const b = collectExplicitDeps(other);
  let drifts = 0;
  const names = new Set([...a.keys(), ...b.keys()]);
  console.log(`dep-drift\n  master: ${master}\n  other:  ${other}\n`);
  for (const name of [...names].sort()) {
    const av = a.get(name);
    const bv = b.get(name);
    if (!av || !bv) continue; // only compare names present in both
    const as = [...av].sort().join("|");
    const bs = [...bv].sort().join("|");
    if (as !== bs) {
      drifts += 1;
      console.log(`  ${name}: master=[${as}] other=[${bs}]`);
    }
  }
  if (drifts === 0) console.log("  no overlapping explicit version drifts found");
  else console.log(`\n${drifts} drifted crate(s)`);
  process.exit(drifts > 0 ? 1 : 0);
}

function cmdLockFingerprint() {
  const root = resolve(expandHome(flag("--root") || "."));
  const locks = walkFiles(root, (_, name) => name === "Cargo.lock");
  if (locks.length === 0) {
    console.log("no Cargo.lock under", root);
    return;
  }
  for (const file of locks) {
    const body = readFileSync(file);
    const hash = createHash("sha256").update(body).digest("hex").slice(0, 16);
    console.log(`${hash}  ${relative(root, file)}`);
  }
}

switch (cmd) {
  case "doctor":
    cmdDoctor();
    break;
  case "cache-plan":
    cmdCachePlan();
    break;
  case "inventory":
    cmdInventory();
    break;
  case "dep-drift":
    cmdDepDrift();
    break;
  case "lock-fingerprint":
    cmdLockFingerprint();
    break;
  case "help":
  case "-h":
  case "--help":
    cmdHelp();
    break;
  default:
    console.error(`unknown command: ${cmd}`);
    cmdHelp();
    process.exit(1);
}
