# Local development

## One cargo home

```sh
echo "${CARGO_HOME:-$HOME/.cargo}"
```

Multiple homes mean multiple downloads of the same crates.

## sccache

```sh
# macOS
brew install sccache
# or: cargo install sccache --locked

export RUSTC_WRAPPER=sccache
export SCCACHE_DIR="${SCCACHE_DIR:-$HOME/.cache/sccache}"
sccache --start-server
sccache -s
```

After building project A, building project B with the same crate versions and
rustc should show **cache hits** in `sccache -s`.

Optional: set the same S3/R2 env vars as CI to share objects with CI runners.

## Target dirs

```sh
cd ~/code/project-a && cargo build    # ./target
cd ~/code/project-b && cargo build    # its own ./target
# both still use ~/.cargo + sccache
```

Never point unrelated workspaces at one `CARGO_TARGET_DIR`.

## nextest

```sh
cargo install cargo-nextest --locked
cargo nextest run --workspace
# uses RUSTC_WRAPPER=sccache automatically when set
```
