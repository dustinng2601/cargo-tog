# Local development

## Recommended profile

```sh
export PATH="/opt/cargo-tog/scripts:$PATH"   # or repo checkout
export RUSTC_WRAPPER=cargo-tog-rustc
export CARGO_TOG_CACHE_DIR="${CARGO_TOG_CACHE_DIR:-$HOME/.cache/cargo-tog}"
```

Install a compiler-cache **engine** once (prebuilt package preferred over compiling
from source on every laptop). `cargo-tog-rustc` will use it when present and fall
back to plain `rustc` if not.

## Registry

Keep a single user-level `CARGO_HOME` for all clones.

## Remote object store from laptops

Optional. Prefer **same-region** endpoints. Cross-continent object GET latency can
exceed local recompile for small crates—multi-level (local + remote) is ideal
when the engine supports it.

## Target directories

Per project. Fast local SSD paths are fine:

```sh
# only inside one repo’s shell
export CARGO_TARGET_DIR="/var/tmp/cargo-target/my-app"
```

## Verification

```sh
node scripts/cargo-tog.mjs doctor
# build twice; second should be faster on deps
cargo build -p my-crate
cargo build -p my-crate
```
