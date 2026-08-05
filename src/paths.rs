use std::path::{Path, PathBuf};

/// Expand a leading `~/` to the home directory.
pub fn expand_user(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    if path == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn resolve_path(path: &str) -> PathBuf {
    let p = expand_user(path);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

pub fn resolve_relative_to(base_file: &Path, path: &str) -> PathBuf {
    let p = expand_user(path);
    if p.is_absolute() {
        p
    } else if let Some(parent) = base_file.parent() {
        parent.join(p)
    } else {
        p
    }
}
