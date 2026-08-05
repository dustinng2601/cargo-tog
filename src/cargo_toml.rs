//! Lightweight Cargo.toml scraping (not a full TOML semantic model).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

#[derive(Debug, Default, Clone)]
pub struct ParsedManifest {
    pub package_name: Option<String>,
    pub package_version: Option<String>,
    pub version_workspace: bool,
    pub is_workspace: bool,
    pub members: Vec<String>,
    pub deps: BTreeMap<String, String>,
    pub workspace_deps: BTreeMap<String, String>,
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
            if line.starts_with("version.workspace") && line.contains("true")
                || line.starts_with("version") && line.contains("workspace") && line.contains("true")
            {
                if line.contains("workspace") {
                    out.version_workspace = true;
                }
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
            if let Some((name, ver)) = simple_dep(line) {
                out.deps.insert(name, ver);
            } else if line.contains("workspace") {
                if let Some(name) = dep_name(line) {
                    out.deps.entry(name).or_insert_with(|| "workspace".into());
                }
            } else if let Some((name, ver)) = table_dep_version(line) {
                out.deps.insert(name, ver);
            }
        }

        if section == "workspace.dependencies" {
            if let Some((name, ver)) = simple_dep(line) {
                out.workspace_deps.insert(name, ver);
            } else if let Some((name, ver)) = table_dep_version(line) {
                out.workspace_deps.insert(name, ver);
            }
        }
    }

    out
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
            name != "target" && name != "node_modules" && name != ".git"
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
    let text = fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    Ok(parse_manifest(&text))
}

pub fn collect_explicit_deps(root: &Path) -> Result<BTreeMap<String, Vec<String>>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in find_cargo_tomls(root)? {
        let parsed = load_manifest(&path)?;
        for (name, ver) in parsed.workspace_deps.iter().chain(parsed.deps.iter()) {
            if ver == "workspace" {
                continue;
            }
            let entry = map.entry(name.clone()).or_default();
            if !entry.contains(ver) {
                entry.push(ver.clone());
            }
        }
    }
    for vers in map.values_mut() {
        vers.sort();
    }
    Ok(map)
}

fn string_assign(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} ");
    let prefix_eq = format!("{key}=");
    let rest = if let Some(r) = line.strip_prefix(&prefix) {
        r.trim().strip_prefix('=')?.trim()
    } else if let Some(r) = line.strip_prefix(&prefix_eq) {
        r.trim()
    } else {
        return None;
    };
    unquote(rest)
}

fn simple_dep(line: &str) -> Option<(String, String)> {
    let (name, rest) = line.split_once('=')?;
    let name = name.trim();
    if !is_ident(name) {
        return None;
    }
    let rest = rest.trim();
    let ver = unquote(rest)?;
    Some((name.to_string(), ver))
}

fn table_dep_version(line: &str) -> Option<(String, String)> {
    let name = dep_name(line)?;
    // version = "x" inside { }
    let idx = line.find("version")?;
    let after = line[idx + "version".len()..].trim().strip_prefix('=')?.trim();
    let ver = unquote(after.split(',').next()?.trim())?;
    Some((name, ver))
}

fn dep_name(line: &str) -> Option<String> {
    let name = line.split('=').next()?.trim();
    if is_ident(name) {
        Some(name.to_string())
    } else {
        None
    }
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
    fn parses_workspace_package_version() {
        let m = parse_manifest(
            r#"
[workspace.package]
version = "2026.1.1-beta.3"
"#,
        );
        assert_eq!(
            m.workspace_package_version.as_deref(),
            Some("2026.1.1-beta.3")
        );
    }
}
