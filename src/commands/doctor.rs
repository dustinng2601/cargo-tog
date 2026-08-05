use std::process::Command;

use anyhow::Result;

use crate::paths::home_dir;

pub fn run() -> Result<()> {
    println!("cargo-tog doctor\n");

    print_cmd_version("cargo", &["--version"]);
    print_cmd_version("rustc", &["--version"]);

    let engine_ok = match Command::new("sccache").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let line = String::from_utf8_lossy(&out.stdout);
            let first = line.lines().next().unwrap_or("installed");
            println!("compiler-cache engine: installed ({first})");
            true
        }
        _ => {
            println!(
                "compiler-cache engine: not installed (cargo-tog-rustc falls back to rustc)"
            );
            false
        }
    };

    let wrapper = std::env::var("RUSTC_WRAPPER").unwrap_or_else(|_| "(unset)".into());
    println!("RUSTC_WRAPPER: {wrapper}");

    let cargo_home = std::env::var("CARGO_HOME").unwrap_or_else(|_| {
        home_dir()
            .map(|h| h.join(".cargo").display().to_string() + " (default)")
            .unwrap_or_else(|| "(unknown)".into())
    });
    println!("CARGO_HOME: {cargo_home}");

    let target = std::env::var("CARGO_TARGET_DIR")
        .unwrap_or_else(|_| "(unset — per-project ./target)".into());
    println!("CARGO_TARGET_DIR: {target}");

    let cache_dir = std::env::var("CARGO_TOG_CACHE_DIR").unwrap_or_else(|_| {
        home_dir()
            .map(|h| {
                h.join(".cache/cargo-tog").display().to_string() + " (default)"
            })
            .unwrap_or_else(|| "(unknown)".into())
    });
    println!("CARGO_TOG_CACHE_DIR: {cache_dir}");

    let bucket =
        std::env::var("CARGO_TOG_BUCKET").unwrap_or_else(|_| "(unset — local/GHA objects only)".into());
    println!("CARGO_TOG_BUCKET: {bucket}");

    if std::env::var_os("CARGO_TARGET_DIR").is_some() {
        println!(
            "\nwarn: CARGO_TARGET_DIR is set. Use only for one workspace checkout, not every project."
        );
    }
    if wrapper != "cargo-tog-rustc" && engine_ok {
        println!(
            "\nhint: set RUSTC_WRAPPER=cargo-tog-rustc (install cargo-tog or put scripts/ on PATH)."
        );
    }

    println!("\nShare: registry + cargo-tog compiler cache. Not target/ across workspaces.");
    println!("Source sync: advanced optional only — caching does not require it.");
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
