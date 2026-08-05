use std::env;
use std::process::Command;

use anyhow::Result;
use cargo_tog::engine::{engine_version_line, find_engine};
use cargo_tog::mode::CacheMode;
use cargo_tog::platform::{
    cache_key_host_fragment, default_cargo_home_display, default_object_cache_dir,
    host_triple_hint, rustc_wrapper_bin_name, OsFamily,
};

pub fn run() -> Result<()> {
    println!("cargo-tog doctor\n");

    print_cmd_version("cargo", &["--version"]);
    print_cmd_version("rustc", &["--version"]);

    let mode = CacheMode::resolve();
    println!("mode: {} — {}", mode.as_str(), mode.description());
    println!("outside_deps: {}", mode.outside_dependencies());
    if let Ok(raw) = env::var("CARGO_TOG_MODE") {
        println!("CARGO_TOG_MODE: {raw}");
    } else {
        println!("CARGO_TOG_MODE: (unset → auto)");
    }

    println!("host_os: {}", OsFamily::current().as_str());
    println!("host_triple_hint: {}", host_triple_hint());
    println!("cache_key_host: {}", cache_key_host_fragment());

    if mode.uses_compiler_engine() {
        match engine_version_line() {
            Some(line) => println!("compiler-cache engine: installed ({line})"),
            None => println!(
                "compiler-cache engine: not installed (cargo-tog-rustc falls back to rustc)"
            ),
        }
        if let Some(path) = find_engine() {
            println!("compiler-cache engine path: {}", path.display());
        }
    } else {
        println!("compiler-cache engine: skipped (mode={})", mode.as_str());
    }

    let wrapper = env::var("RUSTC_WRAPPER").unwrap_or_else(|_| "(unset)".into());
    println!("RUSTC_WRAPPER: {wrapper}");
    println!("wrapper_bin_name: {}", rustc_wrapper_bin_name());
    println!("CARGO_HOME: {}", default_cargo_home_display());

    let target = env::var("CARGO_TARGET_DIR")
        .unwrap_or_else(|_| "(unset — per-project ./target)".into());
    println!("CARGO_TARGET_DIR: {target}");

    let cache_dir = default_object_cache_dir();
    println!(
        "CARGO_TOG_CACHE_DIR: {}{}",
        cache_dir.display(),
        if env::var_os("CARGO_TOG_CACHE_DIR").is_some() {
            ""
        } else {
            " (platform default)"
        }
    );

    let bucket = env::var("CARGO_TOG_BUCKET").unwrap_or_default();
    if bucket.is_empty() {
        println!("CARGO_TOG_BUCKET: (unset)");
    } else {
        println!("CARGO_TOG_BUCKET: {bucket}");
    }

    println!("\nMode guide (no bucket required for most):");
    println!("  github         — quick CI, GitHub-only, zero cloud account");
    println!("  local          — disk only, laptop / persistent runner");
    println!("  registry-only  — downloads only, no object engine");
    println!("  remote         — multi-repo objects (needs bucket)");
    println!("  off            — plain Cargo");
    println!("  See docs/MODES.md");

    if env::var_os("CARGO_TARGET_DIR").is_some() {
        println!(
            "\nwarn: CARGO_TARGET_DIR is set. Use only for one workspace checkout."
        );
    }

    if mode.uses_compiler_engine() {
        let wrapper_ok = wrapper.contains("cargo-tog-rustc");
        if !wrapper_ok && find_engine().is_some() {
            println!(
                "\nhint: set RUSTC_WRAPPER=cargo-tog-rustc (cargo install --path .)"
            );
        }
    }

    if mode == CacheMode::Remote && bucket.is_empty() {
        println!("\nwarn: mode wants remote but bucket is empty — set CARGO_TOG_BUCKET or use mode=github");
    }

    println!("\nCross-OS: objects are per triple; downloads share. docs/CROSS_OS.md");
    Ok(())
}

fn print_cmd_version(bin: &str, args: &[&str]) {
    match Command::new(bin).args(args).output() {
        Ok(out) if out.status.success() => {
            print!("{}", String::from_utf8_lossy(&out.stdout));
        }
        _ => println!("{bin}: NOT FOUND"),
    }
}
