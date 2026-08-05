#!/usr/bin/env node
/**
 * Compatibility shim — cargo-tog is implemented in Rust.
 *
 * Prefer:
 *   cargo install --path .
 *   cargo-tog <command>
 *
 * This script forwards to a cargo-tog binary on PATH or ./target/release|debug.
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const candidates = [
  process.env.CARGO_TOG_BIN,
  "cargo-tog",
  join(root, "target/release/cargo-tog"),
  join(root, "target/debug/cargo-tog"),
].filter(Boolean);

const args = process.argv.slice(2);
for (const bin of candidates) {
  const isPath = bin.includes("/") || bin.includes("\\");
  if (isPath && !existsSync(bin)) continue;
  const r = spawnSync(bin, args, { stdio: "inherit" });
  if (r.error && r.error.code === "ENOENT") continue;
  process.exit(r.status ?? 1);
}

console.error(`cargo-tog: Rust binary not found.

Build or install it:

  cd ${root}
  cargo build --release
  cargo install --path .

Then run: cargo-tog ${args.join(" ")}
`);
process.exit(127);
