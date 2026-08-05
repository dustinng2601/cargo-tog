//! Cross-platform paths and host identity for cargo-tog.
//!
//! Cache objects are never shared across target triples. This module documents
//! and implements *host* layout (where state lives on each OS), not triple sharing.

use std::env;
use std::path::PathBuf;

/// Operating system family for defaults and CI keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsFamily {
    Macos,
    Linux,
    Windows,
    Other,
}

impl OsFamily {
    pub fn current() -> Self {
        match env::consts::OS {
            "macos" => Self::Macos,
            "linux" => Self::Linux,
            "windows" => Self::Windows,
            _ => Self::Other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::Other => "other",
        }
    }
}

/// CPU arch label aligned with common rustc / GHA naming.
pub fn host_arch() -> &'static str {
    match env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        "x86" => "x86",
        other => other,
    }
}

/// Host triple guess for display (not a full target_list probe).
pub fn host_triple_hint() -> String {
    let arch = host_arch();
    match OsFamily::current() {
        OsFamily::Macos => format!("{arch}-apple-darwin"),
        OsFamily::Linux => format!("{arch}-unknown-linux-gnu"),
        OsFamily::Windows => format!("{arch}-pc-windows-msvc"),
        OsFamily::Other => format!("{arch}-unknown-unknown"),
    }
}

/// Suggested rust-cache / matrix key fragment: `{os}-{arch}`.
pub fn cache_key_host_fragment() -> String {
    format!("{}-{}", OsFamily::current().as_str(), host_arch())
}

pub fn home_dir() -> Option<PathBuf> {
    // Windows: USERPROFILE / HOMEDRIVE+HOMEPATH; Unix: HOME
    if let Some(h) = env::var_os("HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(h));
    }
    if let Some(h) = env::var_os("USERPROFILE").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(h));
    }
    let drive = env::var_os("HOMEDRIVE");
    let path = env::var_os("HOMEPATH");
    match (drive, path) {
        (Some(d), Some(p)) => {
            let mut out = PathBuf::from(d);
            out.push(p);
            Some(out)
        }
        _ => None,
    }
}

/// Default directory for local compiler-object cache (`CARGO_TOG_CACHE_DIR`).
///
/// | OS      | Default |
/// |---------|---------|
/// | macOS   | `~/Library/Caches/cargo-tog` |
/// | Linux   | `$XDG_CACHE_HOME/cargo-tog` or `~/.cache/cargo-tog` |
/// | Windows | `%LOCALAPPDATA%\cargo-tog` |
pub fn default_object_cache_dir() -> PathBuf {
    if let Ok(explicit) = env::var("CARGO_TOG_CACHE_DIR") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    match OsFamily::current() {
        OsFamily::Macos => home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Caches/cargo-tog"),
        OsFamily::Windows => {
            if let Some(local) = env::var_os("LOCALAPPDATA").filter(|s| !s.is_empty()) {
                PathBuf::from(local).join("cargo-tog")
            } else {
                home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("AppData/Local/cargo-tog")
            }
        }
        OsFamily::Linux | OsFamily::Other => {
            if let Some(xdg) = env::var_os("XDG_CACHE_HOME").filter(|s| !s.is_empty()) {
                PathBuf::from(xdg).join("cargo-tog")
            } else {
                home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".cache/cargo-tog")
            }
        }
    }
}

/// Default `CARGO_HOME` display (Cargo’s own rules; we only report).
pub fn default_cargo_home_display() -> String {
    if let Ok(h) = env::var("CARGO_HOME") {
        if !h.is_empty() {
            return h;
        }
    }
    match home_dir() {
        Some(home) => home.join(".cargo").display().to_string() + " (default)",
        None => "(unknown)".into(),
    }
}

/// Expand `~/…` using platform home. Leaves other paths unchanged.
/// Also supports `%VAR%` light expansion on Windows for common vars.
pub fn expand_user(path: &str) -> PathBuf {
    let path = path.trim();
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    // Windows-style ~\
    if let Some(rest) = path.strip_prefix("~\\") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    if cfg!(windows) && path.contains('%') {
        return PathBuf::from(expand_windows_env(path));
    }
    PathBuf::from(path)
}

fn expand_windows_env(path: &str) -> String {
    let mut out = String::new();
    let mut rest = path;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('%') {
            let name = &rest[..end];
            rest = &rest[end + 1..];
            if let Ok(val) = env::var(name) {
                out.push_str(&val);
            } else {
                out.push('%');
                out.push_str(name);
                out.push('%');
            }
        } else {
            out.push('%');
            out.push_str(rest);
            return out;
        }
    }
    out.push_str(rest);
    out
}

pub fn resolve_path(path: &str) -> PathBuf {
    let p = expand_user(path);
    if p.is_absolute() {
        p
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

pub fn resolve_relative_to(base_file: &std::path::Path, path: &str) -> PathBuf {
    let p = expand_user(path);
    if p.is_absolute() {
        p
    } else if let Some(parent) = base_file.parent() {
        parent.join(p)
    } else {
        p
    }
}

/// Whether path looks like it needs `PATHEXT` / `.exe` awareness (for messages).
pub fn rustc_wrapper_bin_name() -> &'static str {
    if cfg!(windows) {
        "cargo-tog-rustc.exe"
    } else {
        "cargo-tog-rustc"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_fragment_nonempty() {
        let f = cache_key_host_fragment();
        assert!(f.contains('-'));
    }

    #[test]
    fn expand_tilde() {
        if home_dir().is_some() {
            let p = expand_user("~/foo");
            assert!(p.is_absolute() || cfg!(windows));
            assert!(p.ends_with("foo"));
        }
    }
}
