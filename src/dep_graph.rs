//! Resolved dependency requirements from a tree of Cargo.toml files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::cargo_toml::{find_cargo_tomls, load_manifests, DepSpec, ParsedManifest};

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

        let manifests = load_manifests(&paths)?;
        for m in &manifests {
            for (k, spec) in &m.workspace_deps {
                g.workspace_pins.insert(k.clone(), spec.clone());
            }
        }
        let loaded: Vec<(PathBuf, ParsedManifest)> = paths.into_iter().zip(manifests).collect();

        // A tree can hold several independent workspaces (a polyrepo checkout
        // root). Resolve `workspace = true` against the pins of the workspace
        // that actually owns the manifest, so sibling workspaces cannot
        // silently answer for each other.
        let pins_by_dir = workspace_pin_index(&loaded);

        for (path, m) in &loaded {
            let pins = owning_pins(path, &pins_by_dir).unwrap_or(&g.workspace_pins);
            for (name, spec) in m.workspace_deps.iter().chain(m.deps.iter()) {
                let req = resolve(name, spec, pins);
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

/// Map each workspace root directory to the pins it declares.
fn workspace_pin_index(
    loaded: &[(PathBuf, ParsedManifest)],
) -> BTreeMap<PathBuf, BTreeMap<String, DepSpec>> {
    loaded
        .iter()
        .filter(|(_, m)| m.is_workspace)
        .filter_map(|(path, m)| Some((path.parent()?.to_path_buf(), m.workspace_deps.clone())))
        .collect()
}

/// Pins of the nearest ancestor workspace, i.e. the one Cargo would consult.
fn owning_pins<'a>(
    manifest: &Path,
    pins_by_dir: &'a BTreeMap<PathBuf, BTreeMap<String, DepSpec>>,
) -> Option<&'a BTreeMap<String, DepSpec>> {
    let mut dir = manifest.parent();
    while let Some(current) = dir {
        if let Some(pins) = pins_by_dir.get(current) {
            return Some(pins);
        }
        dir = current.parent();
    }
    None
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

    #[test]
    fn sibling_workspaces_resolve_against_their_own_pins() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cargo-tog-siblings-{stamp}"));
        let _ = std::fs::remove_dir_all(&dir);

        // Two independent workspaces under one tree pin serde differently.
        for (repo, pin) in [("repo-a", "1.0"), ("repo-b", "2.0")] {
            std::fs::create_dir_all(dir.join(repo).join("app")).unwrap();
            std::fs::write(
                dir.join(repo).join("Cargo.toml"),
                format!(
                    "[workspace]\nmembers = [\"app\"]\n\n\
                     [workspace.dependencies]\nserde = {{ version = \"{pin}\" }}\n"
                ),
            )
            .unwrap();
            std::fs::write(
                dir.join(repo).join("app/Cargo.toml"),
                format!(
                    "[package]\nname = \"{repo}-app\"\nversion = \"0.1.0\"\n\n\
                     [dependencies]\nserde = {{ workspace = true }}\n"
                ),
            )
            .unwrap();
        }

        let g = DepGraph::collect(&dir).unwrap();
        let keys = g.drift_keys("serde");

        // Each member must see its own workspace's pin — not whichever tree
        // happened to be walked last.
        assert!(
            keys.contains("ver:1.0") && keys.contains("ver:2.0"),
            "sibling workspaces collapsed onto one pin: {keys:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_headerless_workspace_root_owns_its_members() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cargo-tog-headerless-{stamp}"));
        let _ = std::fs::remove_dir_all(&dir);

        // repo-a is a workspace root spelled *only* as `[workspace.dependencies]`,
        // and it pins no serde. repo-b, a sibling, pins serde 2.0.
        std::fs::create_dir_all(dir.join("repo-a/app")).unwrap();
        std::fs::write(
            dir.join("repo-a/Cargo.toml"),
            "[workspace.dependencies]\nanyhow = { version = \"1\" }\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("repo-a/app/Cargo.toml"),
            "[package]\nname = \"a-app\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\nserde = { workspace = true }\n",
        )
        .unwrap();

        std::fs::create_dir_all(dir.join("repo-b/app")).unwrap();
        std::fs::write(
            dir.join("repo-b/Cargo.toml"),
            "[workspace]\nmembers = [\"app\"]\n\n\
             [workspace.dependencies]\nserde = { version = \"2.0\" }\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("repo-b/app/Cargo.toml"),
            "[package]\nname = \"b-app\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\nserde = { workspace = true }\n",
        )
        .unwrap();

        let g = DepGraph::collect(&dir).unwrap();
        let keys = g.drift_keys("serde");

        // repo-a pins no serde, so its member is genuinely unresolved. Missing
        // the headerless root made it borrow repo-b's 2.0 and report clean.
        assert!(
            keys.contains("workspace?"),
            "member of a headerless root borrowed a sibling's pin: {keys:?}"
        );
        assert!(
            keys.contains("ver:2.0"),
            "repo-b lost its own pin: {keys:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
