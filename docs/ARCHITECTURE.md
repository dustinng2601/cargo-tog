# Architecture: master monorepo + polyrepo Cargo

## Problem

An org like `sd2ek` has:

1. **Master** product monorepo (`sd-soundseek`) — full workspace, lockfile, CI matrix.  
2. **Polyrepos** that are partial mirrors or deploy slices (`soundseek-cli`,
   `sd-cf-work-*`, `soundseek-plugins`, `sd-core`, …).  
3. Each repo’s CI re-fetches crates and recompiles overlapping pure-Rust deps.  
4. People hear “share `/target`” and “one sccache” and mix them up.

## Goals

1. **Correctness first** — no shared state that corrupts builds.  
2. **Share downloads + compiler objects** where keys are content-addressed.  
3. **One source of dependency truth** (master) with **drift detection** on splits.  
4. **CI cache budget** under control (GH cache is small; `target/` is huge).  
5. **Local dev** as fast as CI: same mental model.

## Non-goals

- A single Cargo workspace spanning every private repo.  
- Hosting a crates.io mirror (can be phase 2; not required).  
- Replacing Tauri/desktop release pipelines.

## Components

### A. Policy (docs + config)

`config/cargo-tog.example.toml` declares:

- which path is **master**
- which **polyrepos** exist and how they relate (mirror / deploy / fork)
- sccache backend preference
- whether remote cache is required for CI

### B. Observation (scripts)

- `inventory` — packages and deps under a root  
- `dep-drift` — master vs other tree version mismatches  
- `lock-fingerprint` — stable hash for cache keys  
- `doctor` / `cache-plan` — env sanity  

### C. CI glue (composite action)

`action/` sets:

- `CARGO_TERM_COLOR`, optional `CARGO_PROFILE_DEV_DEBUG=0`
- sccache install + `RUSTC_WRAPPER`
- optional rust-cache with **targets off**
- documents env for R2/S3 when secrets exist

### D. Optional remote sccache (org secrets)

```text
SCCACHE_BUCKET=...
SCCACHE_REGION=...
SCCACHE_ENDPOINT=...        # R2
AWS_ACCESS_KEY_ID=...
AWS_SECRET_ACCESS_KEY=...
SCCACHE_S3_KEY_PREFIX=rustc-{hash}/   # optional
```

All repos use the **same bucket**; prefix by rustc version to avoid cross-compiler
poisoning.

## Cache key design (CI)

### Registry / git (Swatinem or manual)

```text
cargo-registry-${{ runner.os }}-${{ hashFiles('**/Cargo.lock') }}
```

Lockfile differs per repo → different keys → OK. Still helps **within** a repo.

### sccache remote

Keys are **internal to sccache** (content-addressed). No GH cache key needed.
This is why remote sccache is the only clean multi-repo compile share.

### target/

**Do not** put full `target/` in GH Actions cache for large workspaces if you
already use sccache (your monorepo already disables `cache-targets`).

## Dependency management model

```text
master [workspace.dependencies]
        │
        ├─ path members (always in lock)
        │
        ├─ partial mirror scripts ──► polyrepo files (source sync)
        │
        └─ dep-drift CI (weekly) ──► fail or open issue if split pins diverge
```

“Fast refactor”:

1. Land API change in master.  
2. Run mirror/sync for affected polyrepos.  
3. dep-drift + each polyrepo CI.  
4. sccache makes rebuild of unchanged crates cheap.

## Decision record

| Decision | Choice | Why |
|----------|--------|-----|
| Shared `target/` across repos | **No** | Fingerprint collisions; not worth it |
| Shared sccache remote | **Yes** | Real multi-repo compile share |
| Shared `CARGO_HOME` on runners | **Yes** | Free download win |
| cargo-chef default for all | **No** | Only containerized services |
| Master owns pins | **Yes** | One lockfile truth |

## Roadmap

1. **Now** — docs, scripts, composite action, example workflows.  
2. **Next** — org R2 bucket + secrets; enable remote sccache in master + 1–2 polyrepos.  
3. **Later** — optional `cargo-tog sync-pins` PR bot; sparse registry mirror if crates.io is slow.
