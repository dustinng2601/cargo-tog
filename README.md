# cargo-tog

**Enterprise Cargo build-cache coordination** for monorepos, polyrepos, and
multi-OS CI (macOS · Linux · Windows)—without leaving Cargo.

```text
Primary:   registry cache + compiler object cache (per target triple)
Optional:  inventory, dep-drift, lock fingerprints, host-key
Advanced:  partial file mirrors (not required for cache)
```

## Cross-OS in one screen

| Layer | Share across macOS / Linux / Windows? |
|-------|----------------------------------------|
| crates.io / git downloads | **Yes** |
| Compiler objects | **No — per target triple only** |
| `target/` | **No** |
| One remote `CARGO_TOG_BUCKET` | **Yes** (engine partitions by triple) |

Deep dive: **[docs/CROSS_OS.md](docs/CROSS_OS.md)** · Production: **[docs/PRODUCTION.md](docs/PRODUCTION.md)** · Research: **[docs/RESEARCH.md](docs/RESEARCH.md)**

```text
     CARGO_TOG_BUCKET (optional remote)
              │
   ┌──────────┼──────────┐
   │          │          │
 linux     darwin     windows-msvc     ← separate object spaces
 objects   objects    objects
   │          │          │
 own target/ on each runner OS
   │          │          │
   └──────────┼──────────┘
        shared downloads (CARGO_HOME)
```

## Install (all OSes)

```sh
cargo install --path .
# installs two binaries:
#   cargo-tog
#   cargo-tog-rustc    ← set as RUSTC_WRAPPER
```

```sh
# Unix
export RUSTC_WRAPPER=cargo-tog-rustc

# Windows cmd
set RUSTC_WRAPPER=cargo-tog-rustc

cargo-tog doctor
cargo-tog host-key
```

### Default object cache dirs

| OS | `CARGO_TOG_CACHE_DIR` default |
|----|-------------------------------|
| macOS | `~/Library/Caches/cargo-tog` |
| Linux | `$XDG_CACHE_HOME/cargo-tog` or `~/.cache/cargo-tog` |
| Windows | `%LOCALAPPDATA%\cargo-tog` |

## CI (matrix)

```yaml
env:
  CARGO_TOG_BUCKET: ${{ secrets.CARGO_TOG_BUCKET }}
  CARGO_TOG_ENDPOINT: ${{ secrets.CARGO_TOG_ENDPOINT }}
  CARGO_TOG_REGION: ${{ secrets.CARGO_TOG_REGION }}
  CARGO_TOG_ACCESS_KEY_ID: ${{ secrets.CARGO_TOG_ACCESS_KEY_ID }}
  CARGO_TOG_SECRET_ACCESS_KEY: ${{ secrets.CARGO_TOG_SECRET_ACCESS_KEY }}

strategy:
  matrix:
    include:
      - os: ubuntu-22.04
        target: x86_64-unknown-linux-gnu
      - os: macos-14
        target: aarch64-apple-darwin
      - os: windows-2022
        target: x86_64-pc-windows-msvc

jobs:
  test:
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: your-org/cargo-tog/action@main
        with:
          key: test-${{ runner.os }}-${{ runner.arch }}-${{ matrix.target }}
          install-nextest: "true"
      - run: cargo nextest run --workspace
```

Empty bucket secrets → GitHub-hosted objects (still fine per OS).  
Set org secrets once → multi-**repo** reuse **within** each triple.

## Public contract (`CARGO_TOG_*`)

| Name | Role |
|------|------|
| `cargo-tog` | CLI |
| `cargo-tog-rustc` | Native `RUSTC_WRAPPER` (all OSes) |
| `CARGO_TOG_BUCKET` / `ENDPOINT` / `REGION` / keys | Remote objects |
| `CARGO_TOG_CACHE_DIR` | Local object directory |

## CLI

```text
cargo-tog doctor
cargo-tog host-key
cargo-tog cache-plan
cargo-tog inventory --root <path>
cargo-tog dep-drift --master <a> --other <b>
cargo-tog lock-fingerprint --root <path>
cargo-tog sync …          # advanced only
```

## Non-goals

- Sharing compile objects **across** OS/target triples  
- Replacing Cargo with Bazel  
- Mandatory multi-repo source sync  

## License

MIT — [LICENSE-MIT](LICENSE-MIT)
