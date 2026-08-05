//! Minimal Cargo.lock parser for package name → version (and multi-version sets).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Exact versions present in a lockfile for each package name.
#[derive(Debug, Default, Clone)]
pub struct LockIndex {
    /// package name → sorted unique versions (duplicates possible with different sources)
    pub versions: BTreeMap<String, Vec<String>>,
    pub path: std::path::PathBuf,
}

impl LockIndex {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("read lockfile {}", path.display()))?;
        Ok(Self {
            versions: parse_lockfile(&text),
            path: path.to_path_buf(),
        })
    }

    pub fn get(&self, name: &str) -> Option<&[String]> {
        self.versions.get(name).map(|v| v.as_slice())
    }
}

/// Find the primary Cargo.lock under root (prefer root, else first sorted).
pub fn find_primary_lock(root: &Path) -> Option<std::path::PathBuf> {
    let root_lock = root.join("Cargo.lock");
    if root_lock.is_file() {
        return Some(root_lock);
    }
    // Shallow search one level for nested workspaces. Directory order is
    // unspecified, so sort before picking: which lockfile a drift report
    // compares against must not change between runs on the same tree.
    let mut nested: Vec<std::path::PathBuf> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("Cargo.lock"))
        .filter(|p| p.is_file())
        .collect();
    nested.sort();
    nested.into_iter().next()
}

fn parse_lockfile(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut in_package = false;
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line == "[[package]]" {
            flush(&mut map, &mut name, &mut version);
            in_package = true;
            continue;
        }
        if line.starts_with('[') && line != "[[package]]" {
            flush(&mut map, &mut name, &mut version);
            in_package = false;
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(v) = strip_assign(line, "name") {
            name = Some(v);
        } else if let Some(v) = strip_assign(line, "version") {
            version = Some(v);
        }
    }
    flush(&mut map, &mut name, &mut version);
    for vers in map.values_mut() {
        vers.sort();
        vers.dedup();
    }
    map
}

fn flush(
    map: &mut BTreeMap<String, Vec<String>>,
    name: &mut Option<String>,
    version: &mut Option<String>,
) {
    // Both are taken unconditionally, so a partial entry is simply dropped.
    if let (Some(n), Some(v)) = (name.take(), version.take()) {
        let e = map.entry(n).or_default();
        if !e.contains(&v) {
            e.push(v);
        }
    }
}

fn strip_assign(line: &str, key: &str) -> Option<String> {
    let line = line.trim();
    let prefix = format!("{key} = ");
    let rest = line.strip_prefix(&prefix)?;
    let rest = rest.trim().strip_prefix('"')?.strip_suffix('"')?;
    Some(rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_packages() {
        let idx = parse_lockfile(
            r#"
version = 4

[[package]]
name = "tokio"
version = "1.40.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "serde"
version = "1.0.200"
"#,
        );
        assert_eq!(
            idx.get("tokio").map(|v| v.to_vec()),
            Some(vec!["1.40.0".into()])
        );
        assert_eq!(
            idx.get("serde").map(|v| v.to_vec()),
            Some(vec!["1.0.200".into()])
        );
    }
}
