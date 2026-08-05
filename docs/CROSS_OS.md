# Cross-OS design (macOS · Linux · Windows)

This document is the deep reference for **what is shared**, **what is partitioned**,
and **how cargo-tog behaves on each OS**.

---

## 1. Mental model

```text
                    ┌──────────────────────────────────┐
                    │  Remote object store (optional)  │
                    │  CARGO_TOG_BUCKET                │
                    │  (one bucket, many key spaces)   │
                    └────────────────▲─────────────────┘
                                     │
          ┌──────────────────────────┼──────────────────────────┐
          │                          │                          │
   linux-gnu objects         darwin objects              windows-msvc objects
   (cannot run on Mac)       (cannot run on Win)         (cannot run on Linux)
          │                          │                          │
     ubuntu runners             macos runners              windows runners
          │                          │                          │
     own target/                own target/                 own target/
          │                          │                          │
          └──────────────────────────┼──────────────────────────┘
                                     │
                          shared registry downloads
                          (crates.io / git bytes)
```

| Artifact | Shared across OS? | Why |
|----------|-------------------|-----|
| crates.io `.crate` files | **Yes** | OS-agnostic source archives |
| git dependency checkouts | **Mostly yes** | Source; build still per-OS |
| Compiler objects (`rlib` units) | **No** | Linked for one triple only |
| Final binaries / `target/` | **No** | Different ABI, paths, linkers |
| cargo-tog config / policy | **Yes** | Same `CARGO_TOG_*` contract |

**There is no world where a Linux compile object is “reused” on Windows.**  
Enterprise multi-OS caching means:

1. **Don’t re-download** the same crates on every OS job.  
2. **Do reuse** compile objects **within** each OS/arch/triple across jobs & repos.  
3. **One remote bucket** is fine — the engine keys objects by compile identity.

---

## 2. Target triples (partition key)

| Host | Typical triple | GHA runner examples |
|------|----------------|---------------------|
| macOS Apple silicon | `aarch64-apple-darwin` | `macos-14` |
| macOS Intel | `x86_64-apple-darwin` | `macos-13` / cross |
| Linux gnu | `x86_64-unknown-linux-gnu` | `ubuntu-22.04` |
| Linux aarch64 | `aarch64-unknown-linux-gnu` | arm runners |
| Windows MSVC | `x86_64-pc-windows-msvc` | `windows-2022` |
| Windows gnu | `x86_64-pc-windows-gnu` | less common in enterprise |

Also partition on:

- **rustc version** (pin `rust-toolchain.toml`)  
- **profile / RUSTFLAGS / features** (different flags → different objects)  
- **MSVC vs GNU** on Windows (never mix casually)

Print this host:

```sh
cargo-tog host-key
cargo-tog doctor
```

---

## 3. Platform defaults (cargo-tog)

### Object cache directory (`CARGO_TOG_CACHE_DIR`)

| OS | Default |
|----|---------|
| **macOS** | `~/Library/Caches/cargo-tog` |
| **Linux** | `$XDG_CACHE_HOME/cargo-tog` or `~/.cache/cargo-tog` |
| **Windows** | `%LOCALAPPDATA%\cargo-tog` |

Override anytime with `CARGO_TOG_CACHE_DIR`.

### Home detection

| OS | Variables |
|----|-----------|
| Unix | `HOME` |
| Windows | `USERPROFILE`, or `HOMEDRIVE`+`HOMEPATH` |

### Wrapper binary

| OS | Name on PATH |
|----|----------------|
| Unix | `cargo-tog-rustc` |
| Windows | `cargo-tog-rustc.exe` |

Both are **real Rust binaries** (not bash). Install:

```sh
cargo install --path .    # installs cargo-tog + cargo-tog-rustc
set RUSTC_WRAPPER=cargo-tog-rustc   # Windows cmd
export RUSTC_WRAPPER=cargo-tog-rustc
```

Cargo on Windows resolves `.exe` via `PATHEXT` when `RUSTC_WRAPPER=cargo-tog-rustc`.

---

## 4. GitHub Actions matrix (production pattern)

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - os: ubuntu-22.04
        target: x86_64-unknown-linux-gnu
      - os: macos-14
        target: aarch64-apple-darwin
      - os: windows-2022
        target: x86_64-pc-windows-msvc

env:
  CARGO_TOG_BUCKET: ${{ secrets.CARGO_TOG_BUCKET }}
  CARGO_TOG_ENDPOINT: ${{ secrets.CARGO_TOG_ENDPOINT }}
  CARGO_TOG_REGION: ${{ secrets.CARGO_TOG_REGION }}
  CARGO_TOG_ACCESS_KEY_ID: ${{ secrets.CARGO_TOG_ACCESS_KEY_ID }}
  CARGO_TOG_SECRET_ACCESS_KEY: ${{ secrets.CARGO_TOG_SECRET_ACCESS_KEY }}

steps:
  - uses: actions/checkout@v4
  - uses: dtolnay/rust-toolchain@stable
    with:
      targets: ${{ matrix.target }}
  - uses: your-org/cargo-tog/action@main
    with:
      # Registry cache must be OS/arch/target scoped
      key: test-${{ runner.os }}-${{ runner.arch }}-${{ matrix.target }}
      install-nextest: "true"
  - run: cargo nextest run --workspace
```

### Why the key includes OS + arch + target

- **Registry cache** (rust-cache) stores platform-specific build scripts / proc-macro
  artifacts if any path is wrong — scoping avoids restore corruption.  
- **Object cache** self-partitions; the key mainly protects the **registry** layer.

---

## 5. What the Action does on each OS

| Step | ubuntu | macos | windows |
|------|:------:|:-----:|:-------:|
| Map `CARGO_TOG_*` → engine env | ✓ | ✓ | ✓ |
| Install engine (prebuilt action) | ✓ | ✓ | ✓ |
| Install **Rust** `cargo-tog-rustc` from this repo | ✓ | ✓ | ✓ |
| Set `RUSTC_WRAPPER` | ✓ | ✓ | ✓ |
| Registry cache restore | ✓ | ✓ | ✓ |
| nextest install (optional) | ✓ | ✓ | ✓ |

GHA provides bash on Windows runners; the Action may use bash for glue, but the
**wrapper Cargo invokes is a native binary**, not a `.sh` file.

---

## 6. Linkers and OS-specific compile cost

Object cache helps **rustc**. Final link still pays:

| OS | Typical linker |
|----|----------------|
| Linux | ld / lld / mold (user choice) |
| macOS | ld64 / zld |
| Windows | `link.exe` (MSVC) |

Link outputs are rarely as cacheable as dependency `rlib`s. Expect:

- **High hit rate** on pure-Rust dependency graphs  
- **Lower hit rate** on heavy `build.rs`, bindgen, or always-link steps  

---

## 7. Self-hosted runners

| Practice | Detail |
|----------|--------|
| Persistent `CARGO_HOME` | Per runner OS image |
| Persistent `CARGO_TOG_CACHE_DIR` | Local SSD; multi-level with remote bucket |
| One runner OS family | Don’t reuse a Windows `CARGO_HOME` disk on Linux |
| Toolchain pin | Same as CI images |

---

## 8. Failure modes (cross-OS)

| Symptom | Cause | Fix |
|---------|-------|-----|
| “Cache hits” on Linux, cold on Windows | Expected separate object space | Warm each triple once |
| Restored registry breaks Windows | Key missing `runner.os` | Fix cache key |
| `RUSTC_WRAPPER` ignored on Windows | Wrapper not on PATH / wrong name | `cargo install --path .`; open new shell |
| Slow remote hits | Cross-region bucket | Co-locate bucket with runners |
| MSVC vs gnu mismatch | Different triples | Standardize on `*-msvc` in enterprise |

---

## 9. Security (multi-OS CI)

- Same secret names on all OS jobs (`CARGO_TOG_*`).  
- Bucket private; no public ACLs.  
- Prefer CI-only write keys for production object stores.  
- Windows runners: secret masking still applies; don’t echo env.

---

## 10. Verification checklist

```sh
# On each OS
cargo install --path .
cargo-tog doctor
cargo-tog host-key
export RUSTC_WRAPPER=cargo-tog-rustc   # or setx on Windows
cargo build -p some_crate
cargo build -p some_crate              # second build: dep compile should drop
```

In CI: open logs for `cargo-tog: mode=remote-object-store` or `github-hosted-objects`,
and engine stats after a warm run.
