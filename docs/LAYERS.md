# Cache layers

## 1. Registry / git downloads — share

`CARGO_HOME` (crates.io sparse index + git deps). Safe across all projects.

**CI:** rust-cache style registry persistence, **without** packing `target/`.

## 2. Compiler objects — share with policy

Content-addressed compile units via **cargo-tog** (`cargo-tog-rustc` wrapper).

| Backend | Scope |
|---------|--------|
| Local disk (`CARGO_TOG_CACHE_DIR`) | Machine |
| GitHub-hosted | Per-repo / workflow convenience |
| Remote bucket (`CARGO_TOG_BUCKET`) | Multi-repo, multi-job, multi-machine |

**CI:** set `CARGO_INCREMENTAL=0` so object cache owns reuse.

Works with: `cargo build`, `test`, `nextest`, `bench`, `clippy` (compile path).

## 3. `target/` — isolate

One directory per workspace checkout. Do not share across unrelated graphs.

## 4. Complementary tools (not cargo-tog)

| Need | Tool |
|------|------|
| Feature-unified workspace hack | cargo-hakari |
| Docker dep layers | cargo-chef |
| Hermetic remote exec | Bazel |
| Fast test runner | nextest (uses layer 2 automatically) |

## 5. Source mirrors

**Not a cache layer.** See [SYNC.md](SYNC.md) (advanced, optional, not required).
