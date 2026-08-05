//! `cargo-tog-rustc` — public `RUSTC_WRAPPER` for cargo-tog (all OSes).
//!
//! Cargo invokes: `cargo-tog-rustc <rustc> <args…>`
//! We map CARGO_TOG_* env, then run the compiler-cache engine (or rustc).

use std::env;
use std::ffi::OsString;

fn main() {
    // Re-export library modules via path — bin can't use crate name easily for
    // both bins without lib. Duplicate thin entry that calls shared logic by
    // including the same modules… Better: make a lib.rs.
    //
    // For a single package with two bins, put shared code in lib.rs.
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    cargo_tog::engine::exec_rustc_via_engine(&args);
}
