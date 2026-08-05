# cargo-tog

**Enterprise-oriented Cargo build-cache coordination** for monorepos, polyrepos,
and multi-project CI—without abandoning Cargo.

```text
Primary:   registry cache + compiler object cache (local / GitHub-hosted / remote)
Optional:  inventory, dep-drift, lock fingerprints
Advanced:  partial file mirrors (not required for cache; most orgs never need this)
```

## Why it exists

Rust CI time is dominated by **re-downloading crates** and **recompiling the same
dependency graph**. Teams try to “share `target/`” across repos and hurt
correctness. cargo-tog encodes the production pattern used by mature Cargo shops:

1. **Share downloads** (`CARGO_HOME` / registry cache)  
2. **Share compiler objects** via a content-addressed object cache (remote bucket
   for multi-repo)  
3. **Keep `target/` per workspace**  
4. Stay on **Cargo + nextest**—no forced migration to Bazel  

Research background: [docs/RESEARCH.md](docs/RESEARCH.md)  
Production checklist: [docs/PRODUCTION.md](docs/PRODUCTION.md)

## What you configure (our names)

| Surface | Purpose |
|---------|---------|
| **cargo-tog** Action / CLI | Install, policy, observability |
| **`cargo-tog-rustc`** | `RUSTC_WRAPPER` public name |
| **`CARGO_TOG_BUCKET`** (+ endpoint, region, keys) | Remote multi-repo object store |
| **`CARGO_TOG_CACHE_DIR`** | Local object directory on laptops/runners |

Day-to-day ops use **CARGO_TOG_*** only. Cache engines are an implementation detail.

## Quick start (CI)

```yaml
env:
  CARGO_TOG_BUCKET: ${{ secrets.CARGO_TOG_BUCKET }}
  CARGO_TOG_ENDPOINT: ${{ secrets.CARGO_TOG_ENDPOINT }}
  CARGO_TOG_REGION: ${{ secrets.CARGO_TOG_REGION }}
  CARGO_TOG_ACCESS_KEY_ID: ${{ secrets.CARGO_TOG_ACCESS_KEY_ID }}
  CARGO_TOG_SECRET_ACCESS_KEY: ${{ secrets.CARGO_TOG_SECRET_ACCESS_KEY }}

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: your-org/cargo-tog/action@v0   # pin SHA in regulated envs
        with:
          key: test-${{ runner.os }}-${{ runner.arch }}
          install-nextest: "true"
      - run: cargo nextest run --workspace
```

| Secrets empty | Secrets set |
|---------------|-------------|
| GitHub-hosted object cache + registry cache | **Multi-repo** object reuse on same OS/target/rustc |

## Quick start (laptop)

```sh
# Install the Rust CLI
cargo install --path .
# or: cargo build --release && install -m 755 target/release/cargo-tog ~/.cargo/bin/

export PATH="/path/to/cargo-tog/scripts:$PATH"   # cargo-tog-rustc wrapper
export RUSTC_WRAPPER=cargo-tog-rustc
export CARGO_TOG_CACHE_DIR="$HOME/.cache/cargo-tog"

cargo-tog doctor
cargo-tog cache-plan
cargo-tog inventory --root /path/to/workspace
```

The CLI is **Rust** (this repo’s `cargo-tog` binary). The GitHub Action stays YAML
(composite actions are not written in Rust). A thin `scripts/cargo-tog.mjs` may
remain only as a fallback shim.

## CLI

```text
cargo-tog doctor              Health: toolchain, wrapper, cache env
cargo-tog cache-plan          Print share policy
cargo-tog inventory           Map packages / workspace deps
cargo-tog dep-drift           Compare pins across two trees
cargo-tog lock-fingerprint    Stable hashes for cache keys
cargo-tog sync                Advanced only — partial mirrors (see docs/SYNC.md)
```

## Architecture (one screen)

```text
                 ┌─────────────────────────────┐
                 │ Remote object store (opt.)  │  multi-repo compile units
                 │ CARGO_TOG_BUCKET            │
                 └──────────────▲──────────────┘
                                │
              ┌─────────────────┼─────────────────┐
              │                 │                 │
         Repo A CI         Repo B CI         Developer laptop
              │                 │                 │
         own target/       own target/       own target/
              │                 │                 │
              └─────────────────┼─────────────────┘
                                │
                     shared CARGO_HOME downloads
```

## Non-goals

| Out of scope | Prefer instead |
|--------------|----------------|
| Hermetic multi-lang monorepo platform | Bazel / Buck remote cache |
| Workspace-hack generation | cargo-hakari / cargo-rail |
| Docker layer cooking | cargo-chef |
| **Mandatory multi-repo source sync** | Don’t—only [advanced SYNC](docs/SYNC.md) |

## Quality bar

- Fail **safe** when remote cache is unset (still accelerate per-repo)  
- Never require uploading full `target/` to GH cache  
- nextest / cargo test / bench share one compiler wrapper  
- Document security, capacity, and failure modes ([PRODUCTION.md](docs/PRODUCTION.md))  
- Research-backed defaults ([RESEARCH.md](docs/RESEARCH.md))  

## Versioning

Pin the GitHub Action to a tag or commit SHA in regulated environments.  
See [CHANGELOG.md](CHANGELOG.md).

## License

MIT — [LICENSE-MIT](LICENSE-MIT)
