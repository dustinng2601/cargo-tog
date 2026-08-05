# Local development

## One cargo home

Leave the default unless you have a reason:

```sh
echo $CARGO_HOME   # usually empty → ~/.cargo
```

Multiple `CARGO_HOME`s mean multiple downloads of the same crates.

## sccache on the laptop

```sh
brew install sccache   # or cargo install sccache --locked
export RUSTC_WRAPPER=sccache
export SCCACHE_DIR="$HOME/.cache/sccache"
sccache --start-server
sccache -s             # stats
```

After a full monorepo build, building a **polyrepo** that depends on the same
crates (same version, same rustc) should show **cache hits** if using the same
local `SCCACHE_DIR`.

## Target directories

```sh
# monorepo
cd ~/Documents/soundseek
cargo build -p soundseek-cli   # uses ./target

# separate clone of a split — separate target
cd ~/Documents/soundseek-cli-checkout
cargo build                    # uses its own ./target
# still benefits from ~/.cargo + sccache
```

Optional: put monorepo target on a fast disk:

```sh
export CARGO_TARGET_DIR=/Volumes/Fast/cargo-target/sd-soundseek
```

Never point two different workspaces at the same `CARGO_TARGET_DIR`.

## cargo-chef

Only if you build Docker images for a service. Example sketch:

```dockerfile
FROM lukemathwalker/cargo-chef:latest-rust-1 AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM lukemathwalker/cargo-chef:latest-rust-1 AS cacher
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust:1 AS builder
WORKDIR /app
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
COPY . .
RUN cargo build --release -p some-service
```

Desktop / Tauri workflows usually skip this.

## Refactor loop

1. Change code in master monorepo.  
2. `cargo test -p …` (sccache warms).  
3. Sync partial mirror to polyrepo if needed.  
4. In polyrepo: `cargo check` — registry + sccache hits, tiny local `target/`.  
5. `node cargo-tog.mjs dep-drift` if pins might have moved.
