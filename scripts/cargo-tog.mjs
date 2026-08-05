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
  copyFileSync,
  mkdirSync,
} from "node:fs";
import { join, relative, resolve, dirname } from "node:path";
import { spawnSync } from "node:child_process";
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
  console.log(`cargo-tog — enterprise Cargo build-cache coordination

Core:
  cargo-tog doctor
  cargo-tog cache-plan
  cargo-tog inventory --root <path>
  cargo-tog dep-drift --master <path> --other <path>
  cargo-tog lock-fingerprint --root <path>

Advanced (optional; not required for caching):
  cargo-tog sync --config <toml> --check | --apply

Docs: README.md · docs/PRODUCTION.md · docs/RESEARCH.md`);
}

function cmdDoctor() {
  console.log("cargo-tog doctor\n");
  const cargo = spawnSync("cargo", ["--version"], { encoding: "utf8" });
  console.log(cargo.status === 0 ? `cargo: ${cargo.stdout.trim()}` : "cargo: NOT FOUND");
  const rustc = spawnSync("rustc", ["--version"], { encoding: "utf8" });
  console.log(rustc.status === 0 ? `rustc: ${rustc.stdout.trim()}` : "rustc: NOT FOUND");

  const engine = spawnSync("sccache", ["--version"], { encoding: "utf8" });
  console.log(
    engine.status === 0
      ? `compiler-cache engine: installed (${engine.stdout.trim().split("\n")[0]})`
      : "compiler-cache engine: not installed (cargo-tog-rustc falls back to rustc)",
  );

  const wrapper = process.env.RUSTC_WRAPPER || "(unset)";
  console.log(`RUSTC_WRAPPER: ${wrapper}`);
  console.log(`CARGO_HOME: ${process.env.CARGO_HOME || join(homedir(), ".cargo") + " (default)"}`);
  console.log(`CARGO_TARGET_DIR: ${process.env.CARGO_TARGET_DIR || "(unset — per-project ./target)"}`);
  console.log(
    `CARGO_TOG_CACHE_DIR: ${process.env.CARGO_TOG_CACHE_DIR || join(homedir(), ".cache/cargo-tog") + " (default)"}`,
  );
  console.log(`CARGO_TOG_BUCKET: ${process.env.CARGO_TOG_BUCKET || "(unset — local/GHA object cache only)"}`);

  if (process.env.CARGO_TARGET_DIR) {
    console.log(
      "\nwarn: CARGO_TARGET_DIR is set. Use only for one workspace checkout, not every project.",
    );
  }
  if (wrapper !== "cargo-tog-rustc" && engine.status === 0) {
    console.log("\nhint: set RUSTC_WRAPPER=cargo-tog-rustc (scripts/ on PATH).");
  }
  console.log("\nShare: registry + cargo-tog compiler cache. Not target/ across workspaces.");
  console.log("Code sync: optional — only if you maintain partial mirrors (docs/SYNC.md).");
}

function cmdCachePlan() {
  console.log(`# cargo-tog production cache plan
#
# SHARE
#   • CARGO_HOME registry + git downloads
#   • Compiler objects (CARGO_TOG_BUCKET for multi-repo remote)
#
# DO NOT SHARE
#   • target/ across unrelated workspaces
#   • full target/ in GitHub Actions cache
#
# CI DEFAULTS
#   CARGO_INCREMENTAL=0  CARGO_PROFILE_DEV_DEBUG=0  cache-targets=false
#   RUSTC_WRAPPER=cargo-tog-rustc
#   secrets: CARGO_TOG_BUCKET, CARGO_TOG_ENDPOINT, CARGO_TOG_REGION,
#            CARGO_TOG_ACCESS_KEY_ID, CARGO_TOG_SECRET_ACCESS_KEY
#
# LOCAL
#   export RUSTC_WRAPPER=cargo-tog-rustc
#   export CARGO_TOG_CACHE_DIR=$HOME/.cache/cargo-tog
#
# NOT REQUIRED FOR CACHE
#   source sync / partial mirrors (docs/SYNC.md — advanced only)
#
# SEE ALSO
#   docs/PRODUCTION.md  docs/RESEARCH.md  docs/LAYERS.md
`);
}

/** Minimal TOML table scrape for [[sync.mirrors]] blocks. */
function parseSyncConfig(text) {
  const mirrors = [];
  let cur = null;
  let inFiles = false;
  for (const raw of text.split("\n")) {
    const line = raw.replace(/#.*$/, "").trim();
    if (!line) continue;
    if (line === "[[sync.mirrors]]") {
      if (cur) mirrors.push(cur);
      cur = { name: "", source_root: "", target_root: "", files: [] };
      inFiles = false;
      continue;
    }
    if (!cur) continue;
    if (line.startsWith("[")) {
      mirrors.push(cur);
      cur = null;
      inFiles = false;
      continue;
    }
    const kv = line.match(/^([a-z_]+)\s*=\s*"([^"]*)"/);
    if (kv && !inFiles) {
      cur[kv[1]] = kv[2];
      continue;
    }
    if (/^files\s*=/.test(line)) inFiles = true;
    if (inFiles) {
      const pair = line.match(/\[\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\]/);
      if (pair) cur.files.push([pair[1], pair[2]]);
      if (line.includes("]") && !pair && !/^files\s*=\s*\[/.test(line)) {
        // end of multi-line array when we see a lone ]
      }
      if (line === "]") inFiles = false;
    }
  }
  if (cur) mirrors.push(cur);
  return mirrors.filter((m) => m.source_root && m.target_root);
}

function cmdSync() {
  console.log("cargo-tog sync is an advanced optional utility (not part of core cache).\n");
  const configPath = resolve(expandHome(flag("--config") || "cargo-tog.toml"));
  const check = has("--check") || !has("--apply");
  const apply = has("--apply");
  if (!existsSync(configPath)) {
    console.error(`config not found: ${configPath}`);
    console.error("Most teams never need sync. Caching works without it.");
    console.error("See docs/SYNC.md only if you maintain partial file mirrors.");
    process.exit(1);
  }
  const mirrors = parseSyncConfig(readFileSync(configPath, "utf8"));
  if (mirrors.length === 0) {
    console.log("no [[sync.mirrors]] entries — nothing to do (cache does not need sync).");
    return;
  }

  let drifted = 0;
  let copied = 0;
  for (const mirror of mirrors) {
    const srcRoot = resolve(dirname(configPath), expandHome(mirror.source_root));
    const dstRoot = resolve(dirname(configPath), expandHome(mirror.target_root));
    console.log(`mirror ${mirror.name || "(unnamed)"}`);
    console.log(`  source: ${srcRoot}`);
    console.log(`  target: ${dstRoot}`);
    for (const [from, to] of mirror.files) {
      const s = join(srcRoot, from);
      const d = join(dstRoot, to);
      if (!existsSync(s)) {
        console.log(`  MISSING source ${from}`);
        drifted += 1;
        continue;
      }
      if (!existsSync(d)) {
        console.log(`  missing target ${to}`);
        drifted += 1;
        if (apply) {
          mkdirSync(dirname(d), { recursive: true });
          copyFileSync(s, d);
          copied += 1;
          console.log(`  copied → ${to}`);
        }
        continue;
      }
      const sh = createHash("sha256").update(readFileSync(s)).digest("hex");
      const dh = createHash("sha256").update(readFileSync(d)).digest("hex");
      if (sh === dh) {
        console.log(`  ok  ${from} → ${to}`);
      } else {
        console.log(`  DIFF ${from} → ${to}`);
        drifted += 1;
        if (apply) {
          copyFileSync(s, d);
          copied += 1;
          console.log(`  copied → ${to}`);
        }
      }
    }
  }
  if (apply) console.log(`\napplied ${copied} file(s); commit/push in the target repo yourself.`);
  else if (drifted) console.log(`\n${drifted} path(s) drifted. Re-run with --apply to copy.`);
  else console.log("\nall listed files in sync.");
  if (check && !apply && drifted) process.exit(1);
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
  case "sync":
    cmdSync();
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
