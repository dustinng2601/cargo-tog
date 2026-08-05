//! Bridge from public `CARGO_TOG_*` env to the compiler-cache engine.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::mode::CacheMode;
use crate::platform;

/// Apply CARGO_TOG_* → engine environment for the current process.
///
/// The mode decides which tier the engine may talk to. Only `remote` exports
/// bucket coordinates and credentials: a stale `CARGO_TOG_BUCKET` in the
/// environment must never turn `mode=local` into network uploads.
pub fn apply_public_env_to_engine(mode: CacheMode) {
    let cache_dir = env::var_os("CARGO_TOG_CACHE_DIR")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(platform::default_object_cache_dir);
    let _ = std::fs::create_dir_all(&cache_dir);
    env::set_var("SCCACHE_DIR", &cache_dir);

    // GitHub-hosted objects are opt-in per mode, never inherited.
    env::set_var(
        "SCCACHE_GHA_ENABLED",
        if mode.uses_github_object_backend() {
            "true"
        } else {
            "false"
        },
    );

    if !mode.uses_remote_bucket() {
        return;
    }

    if let Some(bucket) = non_empty_var("CARGO_TOG_BUCKET") {
        env::set_var("SCCACHE_BUCKET", bucket);
        if non_empty_var("CARGO_TOG_REGION").is_none() && non_empty_var("SCCACHE_REGION").is_none()
        {
            env::set_var("SCCACHE_REGION", "auto");
        }
    }
    if let Some(endpoint) = non_empty_var("CARGO_TOG_ENDPOINT") {
        env::set_var("SCCACHE_ENDPOINT", endpoint);
    }
    if let Some(region) = non_empty_var("CARGO_TOG_REGION") {
        env::set_var("SCCACHE_REGION", region);
    }
    if let Some(key) = non_empty_var("CARGO_TOG_ACCESS_KEY_ID") {
        env::set_var("AWS_ACCESS_KEY_ID", key);
    }
    if let Some(secret) = non_empty_var("CARGO_TOG_SECRET_ACCESS_KEY") {
        env::set_var("AWS_SECRET_ACCESS_KEY", secret);
    }
}

fn non_empty_var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.is_empty())
}

/// Resolve the engine binary on PATH (Windows-aware via `which`).
pub fn find_engine() -> Option<PathBuf> {
    which::which("sccache").ok()
}

/// Split Cargo's wrapper argv into `(compiler, compiler_args)`.
///
/// Cargo invokes a `RUSTC_WRAPPER` as `wrapper <rustc> <args…>`, so the first
/// argument is the compiler to run — not something to forward to it. When the
/// wrapper is called directly (`cargo-tog-rustc --version`) there is no leading
/// compiler path, so fall back to `RUSTC`/`rustc` and forward everything.
fn split_compiler(args: &[OsString]) -> (OsString, &[OsString]) {
    let looks_like_flag = args
        .first()
        .and_then(|a| a.to_str())
        .is_some_and(|a| a.starts_with('-'));

    match args.first() {
        Some(first) if !looks_like_flag => (first.clone(), &args[1..]),
        _ => (default_rustc(), args),
    }
}

fn default_rustc() -> OsString {
    env::var_os("RUSTC")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| OsString::from("rustc"))
}

/// Run rustc through the engine if the mode allows it, else bare rustc. Exits process.
pub fn exec_rustc_via_engine(args: &[OsString]) -> ! {
    let mode = CacheMode::resolve();
    apply_public_env_to_engine(mode);

    let (compiler, compiler_args) = split_compiler(args);

    // `registry-only` and `off` deliberately bypass the object engine.
    let engine = mode.uses_compiler_engine().then(find_engine).flatten();

    let status = match engine {
        // The engine expects the compiler as its own first argument.
        Some(engine) => spawn(
            engine.as_os_str(),
            std::iter::once(compiler.as_os_str()).chain(iter(compiler_args)),
        ),
        None => spawn(compiler.as_os_str(), iter(compiler_args)),
    };

    match status {
        Ok(s) => std::process::exit(s.code().unwrap_or(1)),
        Err(e) => {
            eprintln!(
                "cargo-tog-rustc: failed to run {}: {e}",
                compiler.to_string_lossy()
            );
            std::process::exit(127);
        }
    }
}

fn iter(args: &[OsString]) -> impl Iterator<Item = &OsStr> {
    args.iter().map(OsString::as_os_str)
}

fn spawn<'a>(
    program: &OsStr,
    args: impl Iterator<Item = &'a OsStr>,
) -> std::io::Result<std::process::ExitStatus> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn os(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    #[test]
    fn cargo_style_argv_splits_compiler_from_its_args() {
        let args = os(&["/toolchain/bin/rustc", "--crate-name", "foo", "src/lib.rs"]);
        let (compiler, rest) = split_compiler(&args);
        assert_eq!(compiler, OsString::from("/toolchain/bin/rustc"));
        // The compiler path must not be forwarded as an input filename.
        assert_eq!(rest, &os(&["--crate-name", "foo", "src/lib.rs"])[..]);
    }

    #[test]
    fn direct_invocation_forwards_all_flags() {
        let args = os(&["--version"]);
        let (compiler, rest) = split_compiler(&args);
        assert!(!compiler.is_empty());
        assert_eq!(rest, &os(&["--version"])[..]);
    }

    #[test]
    fn empty_argv_falls_back_to_rustc() {
        let (compiler, rest) = split_compiler(&[]);
        assert!(!compiler.is_empty());
        assert!(rest.is_empty());
    }

    #[test]
    fn only_remote_mode_reaches_a_bucket() {
        for mode in [
            CacheMode::Local,
            CacheMode::Github,
            CacheMode::RegistryOnly,
            CacheMode::Off,
        ] {
            assert!(!mode.uses_remote_bucket(), "{mode} must not use a bucket");
        }
        assert!(CacheMode::Remote.uses_remote_bucket());
    }
}
