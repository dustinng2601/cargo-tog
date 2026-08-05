//! Lightweight Cargo.toml scraping for inventory and dep-drift.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

#[derive(Debug, Default, Clone)]
pub struct DepSpec {
    /// Semver req if present (`"1"`, `"0.12"`).
    pub version: Option<String>,
    pub path: Option<String>,
    pub git: Option<String>,
    pub workspace: bool,
    /// Original kind for debugging
    pub raw: String,
}

impl DepSpec {
    pub fn summary(&self) -> String {
        if self.workspace {
            return "workspace".into();
        }
        if let Some(p) = &self.path {
            return format!("path={p}");
        }
        if let Some(g) = &self.git {
            return format!("git={g}");
        }
        if let Some(v) = &self.version {
            return v.clone();
        }
        self.raw.clone()
    }
}

#[derive(Debug, Default, Clone)]
pub struct ParsedManifest {
    pub package_name: Option<String>,
    pub package_version: Option<String>,
    pub version_workspace: bool,
    pub is_workspace: bool,
    pub members: Vec<String>,
    /// All dependency kinds: normal / dev / build collapsed for drift.
    pub deps: BTreeMap<String, DepSpec>,
    pub workspace_deps: BTreeMap<String, DepSpec>,
    pub workspace_package_version: Option<String>,
}

pub fn parse_manifest(text: &str) -> ParsedManifest {
    let mut out = ParsedManifest::default();
    let mut section = String::new();
    let mut in_members = false;

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            in_members = false;
            continue;
        }

        if section == "package" {
            if let Some(v) = string_assign(line, "name") {
                out.package_name = Some(v);
            }
            if let Some(v) = string_assign(line, "version") {
                out.package_version = Some(v);
            }
            if line.contains("version") && line.contains("workspace") && line.contains("true") {
                out.version_workspace = true;
            }
        }

        if section == "workspace.package" {
            if let Some(v) = string_assign(line, "version") {
                out.workspace_package_version = Some(v);
            }
        }

        if section == "workspace" {
            out.is_workspace = true;
            if in_members || line.starts_with("members") {
                if line.starts_with("members") && line.contains('[') && !line.contains(']') {
                    in_members = true;
                }
                for q in quoted_strings(line) {
                    out.members.push(q);
                }
                if line.contains(']') {
                    in_members = false;
                }
            }
        }

        if matches!(
            section.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) {
            if let Some((name, spec)) = parse_dep_line(line) {
                out.deps.insert(name, spec);
            }
        }

        if section == "workspace.dependencies" {
            if let Some((name, spec)) = parse_dep_line(line) {
                out.workspace_deps.insert(name, spec);
            }
        }
    }

    out
}

fn parse_dep_line(line: &str) -> Option<(String, DepSpec)> {
    let (name_side, rest) = line.split_once('=')?;
    let name_side = name_side.trim();
    let rest = rest.trim();

    // foo.workspace = true
    if let Some(name) = name_side.strip_suffix(".workspace") {
        let name = name.trim();
        if is_ident(name) && rest.contains("true") {
            return Some((
                name.to_string(),
                DepSpec {
                    workspace: true,
                    raw: rest.to_string(),
                    ..Default::default()
                },
            ));
        }
    }

    if !is_ident(name_side) {
        return None;
    }
    let name = name_side;
    let mut spec = DepSpec {
        raw: rest.to_string(),
        ..Default::default()
    };

    // foo = "1"
    if let Some(v) = unquote(rest) {
        spec.version = Some(v);
        return Some((name.to_string(), spec));
    }

    // foo = { ... }  (single-line table)
    if rest.starts_with('{') {
        if let Some(v) = table_field(rest, "version") {
            spec.version = Some(v);
        }
        if let Some(p) = table_field(rest, "path") {
            spec.path = Some(p);
        }
        if let Some(g) = table_field(rest, "git") {
            spec.git = Some(g);
        }
        if rest.contains("workspace") && rest.contains("true") {
            spec.workspace = true;
        }
        return Some((name.to_string(), spec));
    }

    None
}

fn table_field(table: &str, key: &str) -> Option<String> {
    // key = "value"
    let patterns = [
        format!("{key} = \""),
        format!("{key}=\""),
        format!("{key} = \""),
    ];
    for pat in &patterns {
        if let Some(idx) = table.find(pat) {
            let after = &table[idx + pat.len()..];
            if let Some(end) = after.find('"') {
                return Some(after[..end].to_string());
            }
        }
    }
    // also version.workspace style not a string field
    None
}

pub fn resolve_package_version(parsed: &ParsedManifest, workspace_pkg_ver: Option<&str>) -> String {
    if let Some(v) = &parsed.package_version {
        return v.clone();
    }
    if parsed.version_workspace {
        return workspace_pkg_ver.unwrap_or("workspace").to_string();
    }
    "?".into()
}

pub fn find_cargo_tomls(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != "target"
                && name != "node_modules"
                && name != ".git"
                && name != "dev_docs" // skip local scratch trees if present
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name() == "Cargo.toml" {
            out.push(entry.path().to_path_buf());
        }
    }
    out.sort();
    Ok(out)
}

pub fn load_manifest(path: &Path) -> Result<ParsedManifest> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(parse_manifest(&text))
}

fn string_assign(line: &str, key: &str) -> Option<String> {
    let (left, right) = line.split_once('=')?;
    if left.trim() != key {
        return None;
    }
    unquote(right.trim())
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    let s = s.strip_prefix('"')?.strip_suffix('"')?;
    Some(s.to_string())
}

fn quoted_strings(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workspace_version() {
        let m = parse_manifest(
            r#"
[package]
name = "ssk"
version.workspace = true
"#,
        );
        assert_eq!(m.package_name.as_deref(), Some("ssk"));
        assert!(m.version_workspace);
    }

    #[test]
    fn parses_explicit_and_workspace_deps() {
        let m = parse_manifest(
            r#"
[dependencies]
clap = { version = "4", features = ["derive"] }
tokio.workspace = true
anyhow = { workspace = true }
dirs = "6"
sdsk-client = { path = "../crates/sdsk-client" }
"#,
        );
        assert_eq!(m.deps.get("clap").and_then(|d| d.version.as_deref()), Some("4"));
        assert_eq!(m.deps.get("dirs").and_then(|d| d.version.as_deref()), Some("6"));
        assert!(m.deps.get("anyhow").is_some_and(|d| d.workspace));
        assert!(m.deps.get("tokio").is_some_and(|d| d.workspace));
        assert_eq!(
            m.deps.get("sdsk-client").and_then(|d| d.path.as_deref()),
            Some("../crates/sdsk-client")
        );
    }

    #[test]
    fn parses_workspace_dependency_pins() {
        let m = parse_manifest(
            r#"
[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = "1"
"#,
        );
        assert_eq!(
            m.workspace_deps.get("tokio").and_then(|d| d.version.as_deref()),
            Some("1")
        );
        assert_eq!(
            m.workspace_deps.get("serde").and_then(|d| d.version.as_deref()),
            Some("1")
        );
    }
}
