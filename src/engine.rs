//! Bridge from public `CARGO_TOG_*` env to the compiler-cache engine.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::platform;

/// Apply CARGO_TOG_* → engine environment for the current process.
pub fn apply_public_env_to_engine() {
    let cache_dir = env::var_os("CARGO_TOG_CACHE_DIR")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(platform::default_object_cache_dir);
    let _ = std::fs::create_dir_all(&cache_dir);
    env::set_var("SCCACHE_DIR", &cache_dir);

    if let Ok(bucket) = env::var("CARGO_TOG_BUCKET") {
        if !bucket.is_empty() {
            env::set_var("SCCACHE_BUCKET", &bucket);
            let has_region = env::var_os("CARGO_TOG_REGION")
                .filter(|s| !s.is_empty())
                .or_else(|| env::var_os("SCCACHE_REGION").filter(|s| !s.is_empty()))
                .is_some();
            if !has_region {
                env::set_var("SCCACHE_REGION", "auto");
            }
        }
    }
    if let Ok(endpoint) = env::var("CARGO_TOG_ENDPOINT") {
        if !endpoint.is_empty() {
            env::set_var("SCCACHE_ENDPOINT", endpoint);
        }
    }
    if let Ok(region) = env::var("CARGO_TOG_REGION") {
        if !region.is_empty() {
            env::set_var("SCCACHE_REGION", region);
        }
    }
    if let Ok(key) = env::var("CARGO_TOG_ACCESS_KEY_ID") {
        if !key.is_empty() {
            env::set_var("AWS_ACCESS_KEY_ID", key);
        }
    }
    if let Ok(secret) = env::var("CARGO_TOG_SECRET_ACCESS_KEY") {
        if !secret.is_empty() {
            env::set_var("AWS_SECRET_ACCESS_KEY", secret);
        }
    }
}

/// Resolve the engine binary on PATH (Windows-aware via `which`).
pub fn find_engine() -> Option<PathBuf> {
    which::which("sccache").ok()
}

/// Run rustc through the engine if present, else bare rustc. Exits process.
pub fn exec_rustc_via_engine(args: &[OsString]) -> ! {
    apply_public_env_to_engine();

    let status = if let Some(engine) = find_engine() {
        Command::new(engine)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    } else {
        let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
        Command::new(rustc)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    };

    match status {
        Ok(s) => std::process::exit(s.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("cargo-tog-rustc: failed to run compiler: {e}");
            std::process::exit(127);
        }
    }
}

pub fn engine_version_line() -> Option<String> {
    let engine = find_engine()?;
    let out = Command::new(engine).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()?
        .lines()
        .next()
        .map(|s| s.to_string())
}
