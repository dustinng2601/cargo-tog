# Changelog

## 0.1.0

### Core

- Composite GitHub Action: compiler cache + registry cache, no `target/` upload  
- Public wrapper `cargo-tog-rustc` and `CARGO_TOG_*` configuration surface  
- Remote object store via bucket secrets; GitHub-hosted fallback when unset  
- CI defaults: `CARGO_INCREMENTAL=0`, slim debuginfo  
- Optional `install-nextest` (same compiler cache)  

### Observability / hygiene

- `doctor`, `cache-plan`, `inventory` (incl. `version.workspace`), `dep-drift`,
  `lock-fingerprint`  

### Advanced (non-core)

- Optional partial file `sync` — documented as advanced only  

### Docs

- RESEARCH, PRODUCTION, layered architecture, enterprise runbook  
