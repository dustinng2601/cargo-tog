//! Shared library for `cargo-tog` and `cargo-tog-rustc` binaries.

pub mod cargo_toml;
pub mod dep_graph;
pub mod engine;
pub mod lockfile;
pub mod mode;
pub mod platform;

// Command modules stay binary-only; paths re-export platform helpers for cmds.
pub mod paths {
    pub use crate::platform::{expand_user, home_dir, resolve_path, resolve_relative_to};
}
