use std::path::Path;

use anyhow::{bail, Result};

use cargo_tog::cargo_toml::{
    find_cargo_tomls, load_manifest, resolve_package_version, ParsedManifest,
};
use cargo_tog::paths::resolve_path;

pub fn run(root: &str) -> Result<()> {
    let root = resolve_path(root);
    if !root.is_dir() {
        bail!("root not found: {}", root.display());
    }

    let tomls = find_cargo_tomls(&root)?;
    println!("inventory: {}", root.display());
    println!("Cargo.toml files: {}\n", tomls.len());

    let mut workspace_pkg_ver: Option<String> = None;
    let mut workspace_deps: std::collections::BTreeMap<String, cargo_tog::cargo_toml::DepSpec> =
        std::collections::BTreeMap::new();
    let mut root_members = Vec::new();
    let mut packages: Vec<(String, String, String)> = Vec::new();

    let parsed: Vec<(std::path::PathBuf, ParsedManifest)> = tomls
        .iter()
        .map(|p| load_manifest(p).map(|m| (p.clone(), m)))
        .collect::<Result<Vec<_>>>()?;

    for (path, m) in &parsed {
        if let Some(v) = &m.workspace_package_version {
            workspace_pkg_ver = Some(v.clone());
        }
        for (k, v) in &m.workspace_deps {
            workspace_deps.insert(k.clone(), v.clone());
        }
        if m.is_workspace && !m.members.is_empty() {
            root_members = m.members.clone();
            let rel = path.strip_prefix(&root).unwrap_or(path);
            println!(
                "[workspace] {} members={}",
                rel.display(),
                m.members.len()
            );
        }
    }

    let ws = workspace_pkg_ver.as_deref();
    for (path, m) in &parsed {
        if let Some(name) = &m.package_name {
            let ver = resolve_package_version(m, ws);
            let rel = path.strip_prefix(&root).unwrap_or(path);
            packages.push((name.clone(), ver, rel.display().to_string()));
        }
    }

    packages.sort_by(|a, b| a.0.cmp(&b.0));
    println!("packages:");
    for (name, ver, rel) in &packages {
        println!("  {name}@{ver}  ({rel})");
    }

    if !root_members.is_empty() {
        println!("\nworkspace members declared: {}", root_members.len());
    }
    if !workspace_deps.is_empty() {
        println!("\nworkspace.dependencies ({}):", workspace_deps.len());
        for (k, v) in &workspace_deps {
            println!("  {k} = {}", v.summary());
        }
    }

    let _ = Path::new(".");
    Ok(())
}
