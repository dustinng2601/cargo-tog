# cargo-tog

**Cargo, together.** Coordinate Rust **build caches**, **target dirs**, and
**dependency pins** across monorepos, polyrepos, and multi-project machines.

Not “one shared `/target` for every repo.” That usually hurts. This is:

- what is **safe to share** (downloads, compiler objects)
- what must stay **isolated** (per-workspace `target/`)
- small tools + a composite GitHub Action so CI and laptops stop re-downloading
  and recompiling the same crates over and over

Works for **any** Rust org or personal multi-repo setup — not tied to one product.

## Share matrix

| Layer | Share across repos? | Notes |
|-------|---------------------|--------|
| **crates.io / git cache** (`CARGO_HOME`) | **Yes** | Safe. Easy win on laptops + self-hosted runners. |
| **sccache** | **Yes (remote S3/R2/GCS)** | Best cross-repo / cross-job compile reuse. |
| **GHA sccache / rust-cache** | Per repo by default | Fine baseline; remote sccache is the real multi-repo win. |
| **Full `target/`** | **Usually no** | Different graphs/features thrash or break a shared dir. |
| **cargo-chef** | Per Docker image | One dependency graph per image, not org-wide. |
| **workspace dependency pins** | Policy + drift checks | One “master” tree owns versions; others follow or fail CI. |

Details: [docs/LAYERS.md](docs/LAYERS.md) · [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

## What’s here

```text
docs/           design: layers, architecture, GHA, local
config/         example cargo-tog.toml + env
action/         composite Action: sccache + registry cache (no target/ upload)
scripts/        doctor, cache-plan, inventory, dep-drift, lock-fingerprint
examples/ci/    drop-in workflow fragments
```

`cargo test`, `cargo nextest`, `cargo bench`, and `cargo build` all go through
`rustc` — with `RUSTC_WRAPPER=sccache` they **reuse the same object cache**.
Nextest does not need a special protocol; optional install is supported in the
Action if you want one less step in your workflow.

## Quick start (local)

```sh
node scripts/cargo-tog.mjs doctor
node scripts/cargo-tog.mjs cache-plan
node scripts/cargo-tog.mjs inventory --root /path/to/your/workspace
node scripts/cargo-tog.mjs dep-drift --master /path/to/main --other /path/to/split
```

```sh
# recommended laptop env (see config/env.local.example)
export RUSTC_WRAPPER=sccache
export SCCACHE_DIR="$HOME/.cache/sccache"
# one CARGO_HOME for all clones; do NOT one CARGO_TARGET_DIR for all projects
```

## Quick start (GitHub Actions)

```yaml
env:
  # Optional remote sccache — leave secrets empty to use GHA backend
  SCCACHE_BUCKET: ${{ secrets.SCCACHE_BUCKET }}
  SCCACHE_ENDPOINT: ${{ secrets.SCCACHE_ENDPOINT }}
  SCCACHE_REGION: ${{ secrets.SCCACHE_REGION }}
  AWS_ACCESS_KEY_ID: ${{ secrets.SCCACHE_AWS_ACCESS_KEY_ID }}
  AWS_SECRET_ACCESS_KEY: ${{ secrets.SCCACHE_AWS_SECRET_ACCESS_KEY }}

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: your-org/cargo-tog/action@main   # or copy action/ into the repo
        with:
          key: test-${{ runner.os }}-${{ runner.arch }}
          # install-nextest: "true"   # optional; same sccache as cargo test
      - run: cargo nextest run --workspace
        # or: cargo test --workspace
```

When `SCCACHE_BUCKET` is set, jobs across **different repos** on the same
OS/target/rustc share compile objects. When unset, behavior falls back to the
GitHub Actions sccache backend (no config required to start).

## Design rules

1. **Share downloads + sccache objects.**  
2. **Never casually share `target/` across workspaces.**  
3. **Turn off GH upload of `target/`** if you use sccache (`cache-targets: false`).  
4. **Pin dependency versions in one place**; drift-check the rest.  
5. **CI: prefer `CARGO_INCREMENTAL=0`** so sccache (not incremental) owns reuse.

## Commands

| Command | Purpose |
|---------|---------|
| `doctor` | cargo / rustc / sccache / env sanity |
| `cache-plan` | what to share vs not |
| `inventory` | packages + deps under a tree |
| `dep-drift` | version mismatches between two trees |
| `lock-fingerprint` | hashes for cache keys |

## Status

Docs + scripts + composite Action. Not a hosted cache service — bring your own
R2/S3 bucket when you want org-wide hits.

## License

MIT (see `LICENSE-MIT`).
