# Changelog

## 0.1.0

### Core

- **Rust CLI** (`cargo-tog` binary): doctor, cache-plan, inventory, dep-drift,
  lock-fingerprint  
- Composite GitHub Action: compiler cache + registry cache, no `target/` upload  
- Public wrapper `cargo-tog-rustc` and `CARGO_TOG_*` configuration surface  
- Remote object store via bucket secrets; GitHub-hosted fallback when unset  
- CI defaults: `CARGO_INCREMENTAL=0`, slim debuginfo  
- Optional `install-nextest` (same compiler cache)  

### Advanced (non-core)

- Optional partial file `sync` — documented as advanced only  

### Docs

- RESEARCH, PRODUCTION, layered architecture, enterprise runbook  

### Notes

- Node `scripts/cargo-tog.mjs` is a thin shim that execs the Rust binary only.  

