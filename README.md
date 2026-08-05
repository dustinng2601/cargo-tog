# cargo-tog

**Cargo, together.** Coordinate Rust build caches, target dirs, and dependency
pins across an organization that has a **master monorepo** plus **split
polyrepos** (the SoundSeek / `sd2ek` shape).

This is **not** “one giant shared `/target` for every repo.” Most of that is a
trap. It **is** a clear map of what *can* be shared, what must stay isolated, and
small tools/actions so CI and laptops stop re-downloading the same crates and
re-compiling the same pure-Rust deps over and over.

## The short answer

| Layer | Share across polyrepos? | Notes |
|-------|-------------------------|--------|
| **crates.io / git download cache** (`CARGO_HOME/registry`, `git`) | **Yes** | Safe. Biggest easy win on laptops + self-hosted runners. |
| **sccache** (compiler object cache) | **Yes, with a remote backend** | Best org-wide compile cache. Same rustc + similar flags ⇒ hits. |
| **GitHub Actions cache (Swatinem/rust-cache)** | **Per-repo by default** | Can *key* similarly, but GH cache is **not** org-global for private repos the way people hope. |
| **Full `target/` (`CARGO_TARGET_DIR`)** | **Usually no** | Different workspaces/features/profiles corrupt or thrash a shared dir. Share only inside one workspace or identical lock+flags. |
| **cargo-chef (Docker layers)** | **Per image / per repo** | Great for container builds of *one* graph, not a polyrepo bus. |
| **`[workspace.dependencies]` pins** | **Yes (policy + tooling)** | Master monorepo is source of truth; splits inherit or drift-check. |

Details: [docs/LAYERS.md](docs/LAYERS.md) · [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

## Who this is for

- Orgs with a **master** repo (full product) and **mirrors/splits** (CLI, workers,
  plugins, OSS shell) that each run Cargo CI.
- Teams already using **sccache** + **rust-cache** (like `sd-soundseek`) who want
  the *next* step without magical shared-target folklore.

## What’s in this repo

```text
docs/           why / what shares / CI design
config/         example cargo-tog.toml + sccache / env
action/         composite GitHub Action (sccache + sensible cargo env)
scripts/        inventory, dep-drift, cache-plan (Node, no build)
examples/ci/    drop-in workflow fragments
```

## Quick start (local)

```sh
# from this checkout
node scripts/cargo-tog.mjs doctor
node scripts/cargo-tog.mjs cache-plan
node scripts/cargo-tog.mjs inventory --root ~/Documents/soundseek
node scripts/cargo-tog.mjs dep-drift --master ~/Documents/soundseek --other ~/path/to/split
```

Recommended laptop env (see `config/env.local.example`):

```sh
export CARGO_HOME="$HOME/.cargo"                 # default; keep one
export SCCACHE_DIR="$HOME/.cache/sccache"        # or remote: S3/R2
export RUSTC_WRAPPER=sccache
# Do NOT set one CARGO_TARGET_DIR for every clone unless you know why.
```

## Quick start (GitHub Actions)

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: sd2ek/cargo-tog/action@main   # after this repo is public-ish or use path
        with:
          sccache: "true"
          # rust-cache still per-repo; sccache remote is the cross-repo win
      - run: cargo test --workspace
```

For **org-wide** compile hits, point sccache at **R2/S3** (shared bucket, prefix
per rustc version). GHA’s built-in sccache backend alone does not give you a
true multi-repo shared object store.

## Master + polyrepo model

```text
                    ┌─────────────────────────┐
                    │  master monorepo        │
                    │  (sd-soundseek)         │
                    │  workspace + lockfile   │  ──► source of dep pins
                    └───────────┬─────────────┘
                                │ partial mirrors / publish
          ┌─────────────────────┼─────────────────────┐
          ▼                     ▼                     ▼
   soundseek-cli         sd-cf-work-*          soundseek-plugins
   (thin Cargo)          (JS + tiny Rust?)     (crates + plugins)
          │                     │                     │
          └─────────────────────┴─────────────────────┘
                                │
              shared: registry cache + sccache remote
              not shared: each workspace target/
```

## Commands (`scripts/cargo-tog.mjs`)

| Command | Purpose |
|---------|---------|
| `doctor` | Check sccache, cargo, env, disk for cache dirs |
| `cache-plan` | Print recommended env + what *not* to share |
| `inventory` | Walk a tree; list packages, deps, workspace members |
| `dep-drift` | Compare dependency versions master vs another tree |
| `lock-fingerprint` | Hash Cargo.lock files for CI cache keys |

## Status

Scaffold for `sd2ek`: docs + scripts + composite action. Not a hosted cache
service. Wire R2 credentials in org secrets when you want remote sccache.

## License

MIT OR Apache-2.0 (tooling; copy freely into other orgs).
