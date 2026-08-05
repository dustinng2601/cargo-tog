# Local development

## Registry

One `CARGO_HOME` per user (default `~/.cargo`) for all clones.

## Compiler cache

```sh
# engine binary used under the hood by cargo-tog-rustc — install once
# (implementation detail; day-to-day you set the wrapper name below)
cargo install sccache --locked   # or brew install sccache

# our wrapper name on PATH (from this repo)
export PATH="/path/to/cargo-tog/scripts:$PATH"
export RUSTC_WRAPPER=cargo-tog-rustc
export CARGO_TOG_CACHE_DIR="${CARGO_TOG_CACHE_DIR:-$HOME/.cache/cargo-tog}"
# optional remote:
# export CARGO_TOG_BUCKET=...
# export CARGO_TOG_ENDPOINT=...
# export CARGO_TOG_ACCESS_KEY_ID=...
# export CARGO_TOG_SECRET_ACCESS_KEY=...
```

`scripts/cargo-tog-rustc` maps `CARGO_TOG_*` into the engine and execs it.

## Target dirs

Per project checkout only. Never one global `CARGO_TARGET_DIR` for all repos.

## Code sync

Only if you maintain partial mirrors — see [SYNC.md](SYNC.md).
