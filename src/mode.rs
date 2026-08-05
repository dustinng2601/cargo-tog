//! Cache modes — bucket is optional; most teams start without one.

use std::env;
use std::fmt;

/// How cargo-tog accelerates builds.
///
/// Resolved from `CARGO_TOG_MODE` or auto-detected from the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    /// S3-compatible remote objects (`CARGO_TOG_BUCKET`). Multi-repo / multi-machine.
    Remote,
    /// GitHub Actions object backend (no bucket, no cloud account). CI only.
    Github,
    /// Disk only (`CARGO_TOG_CACHE_DIR`). Laptop or self-hosted runner with persistence.
    Local,
    /// Downloads only (registry/git). No compiler object cache / no engine.
    RegistryOnly,
    /// No caching helpers — plain Cargo (clean forensic builds).
    Off,
}

impl CacheMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Remote => "remote",
            Self::Github => "github",
            Self::Local => "local",
            Self::RegistryOnly => "registry-only",
            Self::Off => "off",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "remote" | "bucket" | "s3" | "r2" => Some(Self::Remote),
            "github" | "gha" | "actions" => Some(Self::Github),
            "local" | "disk" => Some(Self::Local),
            "registry-only" | "registry" | "downloads" => Some(Self::RegistryOnly),
            "off" | "none" | "disable" | "disabled" => Some(Self::Off),
            "auto" => None, // caller resolves
            _ => None,
        }
    }

    /// Auto pick when `CARGO_TOG_MODE` is unset or `auto`.
    pub fn resolve() -> Self {
        if let Ok(raw) = env::var("CARGO_TOG_MODE") {
            if let Some(m) = Self::parse(&raw) {
                return m;
            }
            // "auto" or unknown → fall through
        }
        let bucket = env::var("CARGO_TOG_BUCKET").unwrap_or_default();
        if !bucket.trim().is_empty() {
            return Self::Remote;
        }
        // Only GitHub Actions provides the GitHub object backend. Other CI
        // systems also set `CI`, and picking `github` there would enable a
        // cache service that does not exist; disk caching works everywhere.
        if env::var_os("GITHUB_ACTIONS").is_some() {
            return Self::Github;
        }
        Self::Local
    }

    pub fn uses_compiler_engine(self) -> bool {
        matches!(self, Self::Remote | Self::Github | Self::Local)
    }

    pub fn uses_remote_bucket(self) -> bool {
        matches!(self, Self::Remote)
    }

    pub fn uses_github_object_backend(self) -> bool {
        matches!(self, Self::Github)
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Remote => "remote object store (CARGO_TOG_BUCKET) — multi-repo / multi-machine",
            Self::Github => "GitHub-hosted objects — no bucket; CI only; zero cloud setup",
            Self::Local => "local disk (CARGO_TOG_CACHE_DIR) — laptop / persistent runner",
            Self::RegistryOnly => "registry/git downloads only — no compiler object cache",
            Self::Off => "off — plain Cargo, no cargo-tog acceleration",
        }
    }

    pub fn outside_dependencies(self) -> &'static str {
        match self {
            Self::Remote => "S3-compatible bucket + credentials (R2/S3/MinIO)",
            Self::Github => "GitHub Actions only (built-in cache service)",
            Self::Local => "none beyond local disk",
            Self::RegistryOnly => "none (GH Actions cache API if in CI)",
            Self::Off => "none",
        }
    }
}

impl fmt::Display for CacheMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aliases() {
        assert_eq!(CacheMode::parse("gha"), Some(CacheMode::Github));
        assert_eq!(
            CacheMode::parse("registry-only"),
            Some(CacheMode::RegistryOnly)
        );
        assert_eq!(CacheMode::parse("auto"), None);
    }
}
