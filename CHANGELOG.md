# Changelog

## 0.1.0 — 2026-08-06

### Performance

Tree commands walked directories and read manifests one file at a time, so cost
grew linearly with repository size — the case cargo-tog exists to serve. All
three phases now use the available cores: the directory walk goes breadth-first
only until it has enough independent subtrees to keep every core busy and then
splits, manifests are read and parsed in parallel, lockfiles are hashed in
parallel, and `dep-drift` scans its two trees side by side instead of back to
back. Measured on 8 cores, best of 3+:

| Command | Before | After | |
|---------|-------:|------:|-|
| `dep-drift` (2 × 3000 crates) | 967 ms | 187 ms | 5.2× |
| `dep-drift --json` | 1060 ms | 193 ms | 5.5× |
| `inventory` (3000 crates) | 574 ms | 109 ms | 5.3× |
| `lock-fingerprint` (3000-dir tree) | 143 ms | 83 ms | 1.7× |
| `lock-fingerprint` (200 × 472 KB locks) | 242 ms | 82 ms | 3.0× |

Sharding the walk on the top level alone would have achieved nothing: the usual
monorepo shape puts every crate under a single `crates/`, which is one shard.
Hence the breadth-first descent to a wide enough frontier before splitting.

Output is byte-identical to the serial implementation, verified end to end after
every step — over 6.5k lines of report across all four commands, plus 200
lockfile fingerprints, and spot-checked against `shasum -a 256`. Ordering is
preserved by construction rather than by luck: walk results are sorted, and
parallel reads are reassembled in input order, never completion order, because
callers zip manifests back against their paths and a fingerprint list that
reshuffled between runs would be useless as a cache key.

Behaviour at the edges is unchanged and now covered by tests: a walk rooted at a
pruned directory still yields nothing, symlinks are still not followed (so a
symlink loop cannot hang the walk), and `lock-fingerprint` keeps its own narrower
prune set — it has always descended into `dev_docs`, and since its output feeds
CI cache keys, silently dropping one would silently change a key. Small trees
keep the serial paths, so they pay nothing for thread setup.

No new dependency: this is `std::thread::scope`.

### Changed

- `actions/checkout` v4 → v5. v4 targets Node 20, which GitHub already forces
  onto Node 24 with a deprecation warning on every job.

### Fixed

- **`cargo-tog-rustc` no longer breaks builds when no object engine is installed.**
  Cargo calls a wrapper as `wrapper <rustc> <args…>`; the fallback passed rustc's
  own path back to rustc, which read it as an input filename (`error: multiple
  input filenames provided`). Every build on a machine without the engine failed.
- **Modes are enforced by the binary, not just the Action.** `registry-only` and
  `off` now really bypass the object engine, and a leftover `CARGO_TOG_BUCKET`
  can no longer turn `mode=local` into authenticated network uploads. The
  GitHub object backend is set per mode instead of inherited.
- **`dep-drift` and `inventory` no longer miss whole dependency sections.** The
  manifest reader saw only inline entries under `[dependencies]`, so drift in a
  `[dependencies.<name>]` table, a `[workspace.dependencies.<name>]` pin, or a
  `[target.'cfg(…)'.dependencies]` block was reported as clean.
- `rust-version` claimed 1.75, but the locked clap 4.6 family requires 1.85, so
  the crate could not build on the advertised floor. Corrected to 1.85 and
  pinned by a CI job that builds on exactly the declared version.
- Dropped `dep-drift --lock`, which defaulted to true and could never be
  disabled; `--no-lock` is the real control.
- **`scripts/cargo-tog-rustc` exec'd itself forever.** With `scripts/` on PATH —
  the setup `config/env.local.example` prescribed — `command -v cargo-tog-rustc`
  found the shim itself, so every compile hung with no output. The shim now
  refuses to resolve to itself, splits the compiler off argv correctly, and
  honors `CARGO_TOG_MODE` instead of exporting bucket credentials unconditionally.
- **`auto` no longer picks `github` on non-GitHub CI.** Any CI setting `CI=1`
  (GitLab, CircleCI, Jenkins) selected the GitHub object backend, which does not
  exist there. Now only `GITHUB_ACTIONS` does, matching what `docs/MODES.md` and
  `cache-plan` already documented. Same fix in the composite Action.
- **Sibling workspaces no longer answer for each other's pins.** In a tree with
  more than one workspace, all `[workspace.dependencies]` were merged into one
  map, so `workspace = true` resolved against whichever workspace was walked
  last. Pins now resolve against the workspace that owns the manifest.
- `find_primary_lock` picked a nested `Cargo.lock` in unspecified directory
  order, so drift reports could differ between runs on the same tree.
- **A workspace root spelled only as `[workspace.dependencies]` went
  unrecognized.** TOML creates the `workspace` table for that header just as a
  bare `[workspace]` does, but the manifest reader keyed off the bare header
  alone, so such a root was never indexed. Its members then resolved
  `workspace = true` against a *sibling* workspace's pins — the same crossover
  fixed for headed roots — and a genuinely unpinned dependency was reported as
  clean instead of unresolved.
- **Every copy-paste CI snippet named a repository that does not exist.** The
  README quick start, `docs/{GITHUB_ACTIONS,MODES,CROSS_OS}.md`, all four
  `examples/ci/*.yml`, and the `cache-plan` output shipped a `your-org/cargo-tog`
  placeholder, so following the docs produced a workflow that fails to resolve
  the Action. The README also contradicted itself, using the real repository in
  its matrix example and the placeholder directly above it.
- `Cargo.toml` published a `repository` URL under the crate author's former
  account name.
- `cargo-tog sync` with neither `--check` nor `--apply` reported drift but then
  claimed to require a flag; the read-only default is now the documented
  behavior rather than an unreachable error.

### Added

- Integration tests for the `RUSTC_WRAPPER` contract (argv splitting, exit-code
  propagation, engine-free fallback, per-mode environment) and for `dep-drift`
  over every dependency spelling.
- CI jobs: `cargo fmt --check` + `clippy -D warnings`, and an MSRV build.

### Cache modes (no bucket required)

- `github` | `local` | `registry-only` | `remote` | `off` (+ `auto`)
- Action input `mode:` and env `CARGO_TOG_MODE`
- docs/MODES.md — quick CI without R2/S3

### Cross-OS

- Native **`cargo-tog-rustc`** for macOS / Linux / Windows (not bash)
- Platform cache dirs: macOS `Library/Caches`, Linux XDG, Windows `LOCALAPPDATA`
- `cargo-tog host-key` — OS/arch/triple + suggested CI keys
- Deep reference: `docs/CROSS_OS.md`
- Self CI matrix: `ubuntu-22.04`, `macos-14`, `windows-2022`
- Action installs native wrapper when used from this repo

### Core

- Rust CLI: doctor, cache-plan, inventory, dep-drift, lock-fingerprint, host-key
- Composite Action: `CARGO_TOG_*` secrets, registry cache, optional nextest
- Multi-repo remote objects **within** each target triple
- Advanced optional `sync` (not required for cache)

### Docs

- RESEARCH, PRODUCTION, CROSS_OS, SECURITY, architecture
