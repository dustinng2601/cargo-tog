use std::env;
use std::process::Command;

use anyhow::Result;
use cargo_tog::engine::{engine_version_line, find_engine};
use cargo_tog::platform::{
    cache_key_host_fragment, default_cargo_home_display, default_object_cache_dir,
    host_triple_hint, rustc_wrapper_bin_name, OsFamily,
};

pub fn run() -> Result<()> {
    println!("cargo-tog doctor\n");

    print_cmd_version("cargo", &["--version"]);
    print_cmd_version("rustc", &["--version"]);

    println!("host_os: {}", OsFamily::current().as_str());
    println!("host_triple_hint: {}", host_triple_hint());
    println!("cache_key_host: {}", cache_key_host_fragment());

    match engine_version_line() {
        Some(line) => println!("compiler-cache engine: installed ({line})"),
        None => println!(
            "compiler-cache engine: not installed (cargo-tog-rustc falls back to rustc)"
        ),
    }
    if let Some(path) = find_engine() {
        println!("compiler-cache engine path: {}", path.display());
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

    let bucket = env::var("CARGO_TOG_BUCKET")
        .unwrap_or_else(|_| "(unset — local or GitHub-hosted objects only)".into());
    println!("CARGO_TOG_BUCKET: {bucket}");

    if env::var_os("CARGO_TARGET_DIR").is_some() {
        println!(
            "\nwarn: CARGO_TARGET_DIR is set. Use only for one workspace checkout, not every project."
        );
    }

    let wrapper_ok = wrapper.contains("cargo-tog-rustc");
    if !wrapper_ok && find_engine().is_some() {
        println!(
            "\nhint: set RUSTC_WRAPPER=cargo-tog-rustc (install both bins: cargo install --path .)"
        );
    }

    println!("\nCross-OS notes:");
    println!("  • Registry downloads: shareable across OS");
    println!("  • Compiler objects: per target triple only (linux ≠ darwin ≠ windows-msvc)");
    println!("  • Same remote bucket is fine; keys partition by triple automatically");
    println!("  • See docs/CROSS_OS.md");

    println!("\nSource sync: advanced optional only — caching does not require it.");
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
