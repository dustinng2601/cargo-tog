//! `cargo-tog-rustc` — public `RUSTC_WRAPPER` for cargo-tog (all OSes).
//!
//! Cargo invokes: `cargo-tog-rustc <rustc> <args…>`
//! We map CARGO_TOG_* env, then run the compiler-cache engine (or rustc).

use std::env;
use std::ffi::OsString;

fn main() {
    // argv[0] is this wrapper; everything after it is `<rustc> <args…>`.
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    cargo_tog::engine::exec_rustc_via_engine(&args);
}
