# Research notes: Rust multi-repo build acceleration

Landscape review for **cargo-tog** design decisions. Updated for 2025–2026 tooling.

## Problem at enterprise scale

| Symptom | Cost |
|---------|------|
| Cold CI recompiles the same deps every job | Minutes × matrix × PR volume |
| GitHub Actions cache filled with `target/` | Evictions, slow upload/download, quota burn |
| N repos, N copies of registry tarballs | Disk + network |
| “Share one `target/` across repos” | Fingerprint thrash, subtle breakages |

cargo-tog optimizes the **Cargo-native** path (keep Cargo + nextest). It does not
replace Cargo with Bazel unless you choose that path separately.

## Tooling landscape

### Compiler object caches (rustc wrapper)

| Tool | Role | Notes |
|------|------|--------|
| **[sccache](https://github.com/mozilla/sccache)** (Mozilla) | Production default | Local, S3/R2/GCS/Azure/Redis, multi-level; GHA backend; used widely in CI |
| **[kache](https://kunobi.ninja/blog/open-sourcing-kache)** | Newer alternative | Drop-in wrapper, S3 sync, GHA action; evaluate for greenfield |
| **cachepot** | Historical fork | Largely superseded by sccache ecosystem |
| **BuildCache** | Multi-language | Rust support exists; less standard for pure Rust shops |

**cargo-tog stance:** expose **cargo-tog** branding and config; install a mature
object-cache **engine** under the hood (today: sccache prebuilts in CI). Operators
do not configure third-party product names day-to-day.

**Limits (engine):** typically strong on `rlib` / dependency units; not magic for
every rustc invocation. Still large wins on dependency-heavy workspaces.

### Download / registry caches

| Tool | Role |
|------|------|
| **Cargo `CARGO_HOME`** | Source of truth for crates.io + git checkouts |
| **[Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)** | GHA: smart registry (+ optional target) cache |
| **Sparse registry** | Faster index (Cargo default for crates.io) |

**Best practice with object cache:** rust-cache with **`cache-targets: false`** —
object cache owns compile artifacts; GH cache holds downloads only. Avoids multi-GB
`target/` thrashing GH’s ~10 GB-class quotas.

### Workspace / monorepo graph tools (complementary)

| Tool | Role | vs cargo-tog |
|------|------|----------------|
| **[cargo-hakari](https://docs.rs/cargo_hakari)** | Workspace-hack: unify features, huge local check speedups | Orthogonal — graph, not remote objects |
| **[cargo-rail](https://crates.io/crates/cargo-rail)** | Unify deps, affected CI, releases, some split workflows | Heavier “workspace engine”; not a remote object store |
| **cargo workspaces / cargo-release** | Publish multi-crate trees | Release, not CI object cache |

Use hakari/rail **inside** a monorepo when feature unification hurts; use
**cargo-tog** across jobs and repos for object + registry reuse.

### Container layer caching

| Tool | Role |
|------|------|
| **[cargo-chef](https://github.com/LukeMathWalker/cargo-chef)** | Docker: cook deps layer separately from app sources |

Ideal for service images. Does not replace multi-repo object cache for matrix CI.

### Extreme monorepos

| Tool | Role |
|------|------|
| **Bazel + rules_rust + remote cache** | Hermetic, remote exec/cache at huge scale |
| **Buck2 / Pants** | Similar monorepo-platform tradeoffs |

Trade Cargo ergonomics for hermeticity. cargo-tog targets teams that **stay on Cargo**.

### Test runners

| Tool | Role |
|------|------|
| **[cargo-nextest](https://nexte.st/)** | Faster test execution |

nextest still compiles via Cargo/rustc → **same compiler cache**. No separate
protocol required.

## Production patterns that work

1. **Split caches by layer**  
   - Downloads → registry cache  
   - Objects → remote object store  
   - Never casually share `target/` across workspaces  

2. **CI: `CARGO_INCREMENTAL=0`** with a compiler wrapper  
   Incremental dirs are per-runner and fight object-cache hit rates.

3. **CI: prebuilt engine binaries**  
   Do not `cargo install` the cache engine on every job.

4. **Remote object store for multi-repo**  
   GHA-backed object cache is fine per-repo. Cross-repo / cross-machine reuse
   needs S3-compatible storage (R2, S3, MinIO).

5. **Trusted vs untrusted buckets (optional hardening)**  
   Industry pattern: CI writes a **trusted** cache; developer machines read
   trusted + write **untrusted** (or read-only trusted). Reduces risk of
   untrusted local pollution of CI.

6. **Latency awareness**  
   Object store in the same cloud region as runners. WAN object cache for laptops
   can be slower than local disk (local multi-level still helps).

7. **Toolchain pinning**  
   Hits require matching rustc. Pin `dtolnay/rust-toolchain` / `rust-toolchain.toml`.

8. **Matrix awareness**  
   Objects are per target triple (linux-gnu ≠ darwin-aarch64). Still share **within**
   each triple across repos.

## What cargo-tog deliberately is not

- Not a Bazel replacement  
- Not a full monorepo release engine (rail/hakari territory)  
- Not a mandatory multi-repo source mirror system  
- Not “one shared `target/` for the company”  

## References (primary)

- Mozilla sccache storage backends (local, S3/R2, multi-level)  
- Swatinem/rust-cache `cache-targets` semantics  
- cargo-chef Docker layer model  
- cargo-hakari workspace-hack model  
- Public writeups on distributed sccache + S3/R2 for CI seeding local builds  
