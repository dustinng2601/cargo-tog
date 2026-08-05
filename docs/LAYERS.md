# Cache and build layers

What “sharing” means for multi-repo Rust work (monorepo, polyrepo, or several
unrelated projects on one machine).

## 1. Download cache — **share freely**

Under `CARGO_HOME` (default `~/.cargo`):

- `registry/index`, `registry/cache` — crates.io  
- `git/db`, `git/checkouts` — git dependencies  

**Safe across all projects.** Same crate version = same bytes.

| Environment | How |
|-------------|-----|
| Laptop | One user `CARGO_HOME` (default). |
| CI | rust-cache (registry/git) or persistent runner disk. |
| Self-hosted | Keep `CARGO_HOME` on the runner between jobs. |

## 2. Compiler cache (sccache) — **share with a remote backend**

`RUSTC_WRAPPER=sccache` stores **object-level** results keyed by compiler,
flags, and inputs.

| Backend | Cross-repo? | Notes |
|---------|-------------|--------|
| Local disk | Same machine | Laptop / one runner. |
| GitHub Actions | Mostly per-repo | Easy default. |
| S3 / R2 / GCS / Redis | **Yes** | Real multi-repo / multi-workflow reuse. |

Hits need roughly the same rustc, target triple, and similar `RUSTFLAGS` /
profile. Different features correctly miss.

Works with **`cargo test`**, **`cargo nextest`**, **`cargo build`**,
**`cargo bench`** — anything that invokes `rustc` through Cargo.

### CI tip

Set `CARGO_INCREMENTAL=0` in CI so reuse comes from sccache, not per-job
incremental directories that never leave the runner.

## 3. `target/` / `CARGO_TARGET_DIR` — **do not casually share**

OK: one workspace, many crates (Cargo’s normal single `target/`).  
Bad: several unrelated workspaces writing one shared `target/` → thrash or
fingerprint bugs.

**Rule:** one `target/` (or `CARGO_TARGET_DIR`) **per workspace checkout**.

## 4. cargo-chef — **Docker only**

Optimizes **image layers** for one app graph. Complements sccache; does not
replace org-wide object caches.

## 5. Dependency pins

If you have a “main” workspace and thinner split repos:

- main owns `[workspace.dependencies]` + lockfile  
- splits path/git depend or copy pins  
- `cargo-tog dep-drift` catches divergence  

Optional — single-repo users can ignore drift entirely.

## Summary

```text
     remote sccache (S3/R2)     ← compile objects (multi-repo)
              ▲
     repo A / repo B / laptop
              │
         own target/ each
              │
     shared CARGO_HOME          ← downloads
```
