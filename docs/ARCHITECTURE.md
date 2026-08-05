# Architecture

## Product boundary

```text
cargo-tog CORE     = policy + CI Action + wrapper + ops docs
                     for registry + compiler-object caching

cargo-tog EDGE     = inventory, dep-drift, fingerprints
                     (observability / hygiene)

cargo-tog ADVANCED = sync (partial mirrors) — optional, non-core
```

## Design principles

1. **Cargo stays the build system**  
2. **Correctness over hit rate** — disable cache must still yield clean builds  
3. **Named for operators** — `CARGO_TOG_*`, not third-party product names  
4. **Layered caches** — downloads ≠ objects ≠ `target/`  
5. **Enterprise defaults** — no `target/` in GH cache; incremental off in CI  
6. **Sync is not cache** — never couple multi-repo acceleration to source copies  

## Runtime flow (CI)

```text
checkout → toolchain pin → cargo-tog Action
  → map CARGO_TOG_* → engine env
  → install engine (prebuilt)
  → install cargo-tog-rustc wrapper → RUSTC_WRAPPER
  → registry cache restore
  → cargo / nextest / clippy
  → objects read/write remote or GitHub-hosted backend
```

## Trust model

- Remote objects are addressed by compilation identity (engine keys).  
- Prefer CI write credentials scoped to one bucket.  
- Optional future: dual-bucket trusted/untrusted (see RESEARCH.md).  

## Related systems

See [RESEARCH.md](RESEARCH.md) for sccache/kache/hakari/chef/Bazel comparisons.
