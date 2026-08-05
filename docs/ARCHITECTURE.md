# Architecture

## Problem

People run Cargo in many places:

- one large monorepo  
- several split repos that share crates/versions  
- unrelated projects on the same laptop or org runners  

Each CI job re-fetches crates and recompiles overlapping pure-Rust dependencies.
“Share `/target`” is the wrong first idea.

## Goals

1. Correctness first — no shared state that corrupts builds.  
2. Share **downloads** and **compiler objects** where keys are content-addressed.  
3. Optional **dependency drift** checks when you maintain related trees.  
4. Stay within CI cache budgets (full `target/` is huge).  
5. Same mental model locally and in CI.

## Non-goals

- One Cargo workspace spanning every repo in an org.  
- Replacing product-specific release pipelines.  
- Requiring any particular company or product naming.

## Components

| Piece | Role |
|-------|------|
| Docs | Policy: what to share |
| `scripts/cargo-tog.mjs` | Inventory, drift, fingerprints, doctor |
| `action/` | CI: sccache + registry cache; optional nextest install |
| Example YAML | Copy/paste for any GitHub repo |

## Cache key design

**Registry (rust-cache):** keyed by lockfile + OS — per repo is fine.  

**sccache remote:** content-addressed inside sccache — **no GH key needed**; this
is the multi-repo compile share.

**target/:** do not upload to GH Actions when sccache is enabled.

## Decision record

| Decision | Choice |
|----------|--------|
| Shared `target/` across repos | No (default) |
| Shared sccache remote | Yes when bucket configured |
| Shared `CARGO_HOME` on runners | Yes |
| nextest-specific cache protocol | No — use same sccache as cargo |
| cargo-chef for all CI | No — Docker graphs only |

## Roadmap

1. Docs + scripts + Action (now).  
2. Point CI at a shared R2/S3 bucket when ready.  
3. Optional pin-sync bot later.
