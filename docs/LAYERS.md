# Cache and build layers

What “sharing” actually means in a Cargo polyrepo org.

## 1. Download cache — **share freely**

Paths under `CARGO_HOME` (default `~/.cargo`):

- `registry/index`, `registry/cache` — crates.io tarballs  
- `git/db`, `git/checkouts` — git dependencies  

**Safe across all repos and workspaces.** Same crate version = same bytes.

| Environment | How |
|-------------|-----|
| Laptop | One `CARGO_HOME` for the user (default). |
| CI (GHA) | `Swatinem/rust-cache` with `cache-targets: false` still caches registry; or cache `~/.cargo/registry` yourself. |
| Self-hosted runner | Persistent `CARGO_HOME` on the runner disk. |

**Do not** commit `CARGO_HOME` into git. **Do** persist it on runners.

## 2. Compiler cache (sccache) — **share with a remote backend**

`RUSTC_WRAPPER=sccache` stores **object-level** compilation results keyed by
compiler version, command line, and inputs.

| Backend | Cross-repo? | Notes |
|---------|-------------|--------|
| Local disk (`SCCACHE_DIR`) | Same machine only | Fine for a laptop or one runner. |
| GitHub Actions (`SCCACHE_GHA_ENABLED`) | **Mostly per-repo / per-workflow quirks** | Easy; not a true org object store. |
| S3 / R2 / GCS / Redis | **Yes — org-wide** | This is the real multi-repo win. |

**Hits require** roughly the same:

- rustc version (toolchain file / `dtolnay/rust-toolchain` pin)
- target triple
- similar `RUSTFLAGS` / profile (opt-level, LTO, debuginfo)
- same crate source fingerprint

Different feature sets still miss — that’s correct, not a bug.

### What sd-soundseek already does

- `RUSTC_WRAPPER=sccache`
- `mozilla-actions/sccache-action`
- `Swatinem/rust-cache` with **`cache-targets: false`** so GH’s 10 GB quota isn’t
  eaten by `target/` — sccache owns objects; rust-cache owns registry/git metadata.

cargo-tog’s action follows that split.

## 3. `target/` / `CARGO_TARGET_DIR` — **do not casually share**

`target/` holds:

- dependency build units **specific to that package graph + features + profile**
- fingerprint files that assume one workspace layout
- final bins/libs for *this* project

### When a shared target dir is OK

- **One monorepo workspace**, many crates — Cargo already uses one `target/`.  
- **Identical** lockfile + features + profile on the **same machine**, intentionally
  (rare; still easy to shoot yourself).

### When it is a bad idea

- Master monorepo + `soundseek-cli` + plugin repo all pointing at
  `~/shared-target` → fingerprint collisions, mysterious rebuilds, sometimes
  link errors.
- Mixing `debug` / `release` / different `RUSTFLAGS` in one dir without care.
- CI jobs for unrelated repos writing the same cache key’s `target/`.

**Rule:** shared **`CARGO_TARGET_DIR` only inside one workspace checkout.**  
Polyrepos each get their own `target/` (or `target-ci/`, `target-release/`).

### What to set instead

```sh
# monorepo checkout
export CARGO_TARGET_DIR="$PWD/target"

# optional: keep target off slow network drives
export CARGO_TARGET_DIR="/var/tmp/cargo-target/sd-soundseek"
```

Per-repo absolute dirs on a **fast local SSD** beat a shared network `target/`.

## 4. cargo-chef — **Docker, one graph**

[cargo-chef](https://github.com/LukeMathWalker/cargo-chef) optimizes **Docker
layer caching**:

1. `recipe.json` from the dependency graph  
2. Build deps layer  
3. Copy source, build app layer  

It does **not** replace sccache or org-wide caches. Use it when the artifact is
a container (e.g. some services). Desktop Tauri + multi-OS GHA matrices usually
stay on sccache + rust-cache.

## 5. Dependency pins — **share policy, not the whole graph blindly**

Master monorepo `[workspace.dependencies]` + `Cargo.lock` is the **source of
truth** for versions.

Polyrepos that only mirror a slice should either:

- use **path / git / workspace** deps back to master (private), or  
- **copy pins** and **drift-check** (`cargo-tog dep-drift`), or  
- publish crates to a registry and depend on semver (later).

“Manage all Cargo.toml” = inventory + drift + optional generate-pin PR, not one
mega-workspace spanning private splits (that re-creates a monorepo by accident).

## 6. Fast refactor across repos

| Goal | Tooling |
|------|---------|
| Rename API used in master + CLI mirror | Master first; `sync-cli-repos` / mirror scripts; CI drift check |
| Bump `tokio` everywhere | Change master workspace.dependencies; dep-drift fails until splits follow |
| Avoid rebuilding the world | sccache remote + don’t wipe `CARGO_HOME` |
| Avoid 2 GB `target` in GH cache | `cache-targets: false` + sccache (already your pattern) |

## Summary diagram

```text
          ┌──────────────────────────────────────┐
          │  Remote sccache (R2/S3)              │  compile objects
          └──────────────────────────────────────┘
                         ▲
          ┌──────────────┼──────────────┐
          │              │              │
     monorepo CI    cli CI         plugins CI
          │              │              │
          ▼              ▼              ▼
     own target/    own target/    own target/
          │              │              │
          └──────────────┼──────────────┘
                         ▼
              shared CARGO_HOME (registry/git)
```
