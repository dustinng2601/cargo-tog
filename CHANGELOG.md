# Changelog

## 0.1.0

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
