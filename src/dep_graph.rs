//! Resolved dependency requirements from a tree of Cargo.toml files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::cargo_toml::{find_cargo_tomls, load_manifest, DepSpec};

/// How a dependency appears after resolving workspace pins in-tree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResolvedReq {
    /// Semver requirement string from manifest
    Version(String),
    /// workspace = true resolved to workspace.dependencies version
    Workspace(String),
    /// workspace = true but this tree has no pin
    UnresolvedWorkspace,
    Path(String),
    Git(String),
    Other(String),
}

impl ResolvedReq {
    pub fn label(&self) -> String {
        match self {
            Self::Version(v) => v.clone(),
            Self::Workspace(v) => format!("workspace→{v}"),
            Self::UnresolvedWorkspace => "workspace?(unpinned)".into(),
            Self::Path(p) => format!("path:{p}"),
            Self::Git(u) => format!("git:{u}"),
            Self::Other(s) => s.clone(),
        }
    }

    /// For equality of "what we expect to build against" — paths compared by basename.
    pub fn drift_key(&self) -> String {
        match self {
            Self::Version(v) | Self::Workspace(v) => format!("ver:{v}"),
            Self::UnresolvedWorkspace => "workspace?".into(),
            Self::Path(p) => {
                let base = Path::new(p)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(p);
                format!("path:{base}")
            }
            Self::Git(u) => format!("git:{u}"),
            Self::Other(s) => format!("other:{s}"),
        }
    }

    pub fn is_path(&self) -> bool {
        matches!(self, Self::Path(_))
    }
}

#[derive(Debug, Default)]
pub struct DepGraph {
    pub root: PathBuf,
    pub manifests: usize,
    /// crate → resolved requirement forms
    pub deps: BTreeMap<String, BTreeSet<ResolvedReq>>,
    /// `[workspace.dependencies]` entries (version and/or path/git).
    pub workspace_pins: BTreeMap<String, DepSpec>,
}

impl DepGraph {
    pub fn collect(root: &Path) -> Result<Self> {
        let mut g = Self {
            root: root.to_path_buf(),
            ..Default::default()
        };
        let paths = find_cargo_tomls(root)?;
        g.manifests = paths.len();

        let mut loaded = Vec::new();
        for p in &paths {
            let m = load_manifest(p)?;
            for (k, spec) in &m.workspace_deps {
                g.workspace_pins.insert(k.clone(), spec.clone());
            }
            loaded.push(m);
        }

        for m in &loaded {
            for (name, spec) in m.workspace_deps.iter().chain(m.deps.iter()) {
                let req = resolve(name, spec, &g.workspace_pins);
                g.deps.entry(name.clone()).or_default().insert(req);
            }
        }
        Ok(g)
    }

    pub fn summary(&self, name: &str) -> Option<String> {
        let set = self.deps.get(name)?;
        let mut v: Vec<_> = set.iter().map(|r| r.label()).collect();
        v.sort();
        v.dedup();
        Some(v.join(" | "))
    }

    pub fn drift_keys(&self, name: &str) -> BTreeSet<String> {
        self.deps
            .get(name)
            .map(|s| s.iter().map(|r| r.drift_key()).collect())
            .unwrap_or_default()
    }
}

fn resolve(name: &str, spec: &DepSpec, pins: &BTreeMap<String, DepSpec>) -> ResolvedReq {
    // Prefer fields on the dep line itself.
    if let Some(p) = &spec.path {
        return ResolvedReq::Path(p.clone());
    }
    if let Some(g) = &spec.git {
        return ResolvedReq::Git(g.clone());
    }
    if spec.workspace {
        return match pins.get(name) {
            Some(pin) => resolve_pin(pin),
            None => ResolvedReq::UnresolvedWorkspace,
        };
    }
    if let Some(v) = &spec.version {
        return ResolvedReq::Version(v.clone());
    }
    // Workspace.dependencies entry used as a dep of itself
    if let Some(pin) = pins.get(name) {
        return resolve_pin(pin);
    }
    ResolvedReq::Other(spec.raw.clone())
}

fn resolve_pin(pin: &DepSpec) -> ResolvedReq {
    if let Some(p) = &pin.path {
        return ResolvedReq::Path(p.clone());
    }
    if let Some(g) = &pin.git {
        return ResolvedReq::Git(g.clone());
    }
    if let Some(v) = &pin.version {
        return ResolvedReq::Workspace(v.clone());
    }
    ResolvedReq::Other(pin.summary())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolves_workspace_pin() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cargo-tog-depgraph-{stamp}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut root = std::fs::File::create(dir.join("Cargo.toml")).unwrap();
        write!(
            root,
            r#"
[workspace]
members = ["app"]
[workspace.dependencies]
tokio = {{ version = "1", features = ["full"] }}
"#
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("app")).unwrap();
        let mut app = std::fs::File::create(dir.join("app/Cargo.toml")).unwrap();
        write!(
            app,
            r#"
[package]
name = "app"
version = "0.1.0"
[dependencies]
tokio = {{ workspace = true }}
clap = "4"
"#
        )
        .unwrap();

        let g = DepGraph::collect(&dir).unwrap();
        let keys = g.drift_keys("tokio");
        assert!(
            keys.iter().any(|k| k == "ver:1"),
            "expected workspace pin 1, got {keys:?}"
        );
        assert!(g.drift_keys("clap").contains("ver:4"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
