# Cache layers (cargo-tog)

## 1. Registry cache — share freely

`CARGO_HOME` downloads (crates.io + git). Safe across all projects on a machine
or runner.

## 2. Compiler cache — share with a remote bucket

cargo-tog’s **compiler cache** stores object-level compile results. With a
remote bucket (`CARGO_TOG_BUCKET` + credentials), jobs in **different repos**
can hit the same objects (same rustc, target, similar flags).

| Mode | When |
|------|------|
| GitHub-hosted | Bucket secrets empty |
| Remote (S3-compatible / R2) | `CARGO_TOG_BUCKET` set |

Applies to `cargo test`, `cargo nextest`, `cargo build`, `cargo bench`.

CI tip: `CARGO_INCREMENTAL=0` so reuse comes from the compiler cache, not
throwaway incremental dirs on the runner.

## 3. `target/` — do not casually share

One `target/` (or `CARGO_TARGET_DIR`) **per workspace**. Not across unrelated
repos.

## 4. Code sync — separate product surface

Keeping file **bytes** identical across git repos is **not** a cache feature.
See [SYNC.md](SYNC.md). Optional; not required for any cache mode.
