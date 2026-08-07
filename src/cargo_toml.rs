//! Lightweight Cargo.toml scraping for inventory and dep-drift.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
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

/// Which table the parser is currently inside.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Section {
    Package,
    WorkspacePackage,
    Workspace,
    /// `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`
    Deps,
    /// `[workspace.dependencies]`
    WorkspaceDeps,
    /// `[dependencies.<name>]` — a dependency spelled as its own table
    DepEntry(String),
    /// `[workspace.dependencies.<name>]`
    WorkspaceDepEntry(String),
    Other,
}

pub fn parse_manifest(text: &str) -> ParsedManifest {
    let mut out = ParsedManifest::default();
    let mut section = Section::Other;
    let mut in_members = false;

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let header = line[1..line.len() - 1].trim();
            section = classify_section(header);
            in_members = false;
            // Any `[workspace…]` table marks a workspace root. TOML creates the
            // `workspace` table for `[workspace.dependencies]` exactly as a bare
            // `[workspace]` does, and Cargo reads pins from both — so keying off
            // the bare header alone left such a root unindexed, and its members
            // resolved `workspace = true` against a sibling workspace's pins.
            if matches!(
                section,
                Section::Workspace
                    | Section::WorkspacePackage
                    | Section::WorkspaceDeps
                    | Section::WorkspaceDepEntry(_)
            ) {
                out.is_workspace = true;
            }
            // A dependency table exists even if every line in it is a field we
            // ignore (`features`, `optional`, …), so register it eagerly.
            match &section {
                Section::DepEntry(name) => {
                    out.deps
                        .entry(name.clone())
                        .or_insert_with(|| table_spec(header));
                }
                Section::WorkspaceDepEntry(name) => {
                    out.workspace_deps
                        .entry(name.clone())
                        .or_insert_with(|| table_spec(header));
                }
                _ => {}
            }
            continue;
        }

        match &section {
            Section::Package => {
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
            Section::WorkspacePackage => {
                if let Some(v) = string_assign(line, "version") {
                    out.workspace_package_version = Some(v);
                }
            }
            Section::Workspace => {
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
            Section::Deps => {
                if let Some((name, spec)) = parse_dep_line(line) {
                    out.deps.insert(name, spec);
                }
            }
            Section::WorkspaceDeps => {
                if let Some((name, spec)) = parse_dep_line(line) {
                    out.workspace_deps.insert(name, spec);
                }
            }
            Section::DepEntry(name) => {
                if let Some(spec) = out.deps.get_mut(name) {
                    apply_entry_field(spec, line);
                }
            }
            Section::WorkspaceDepEntry(name) => {
                if let Some(spec) = out.workspace_deps.get_mut(name) {
                    apply_entry_field(spec, line);
                }
            }
            Section::Other => {}
        }
    }

    out
}

fn table_spec(header: &str) -> DepSpec {
    DepSpec {
        raw: format!("[{header}]"),
        ..Default::default()
    }
}

/// Map a table header to the section it denotes.
///
/// Target predicates are transparent: `[target.'cfg(unix)'.dependencies]`
/// contributes to the same dependency set as `[dependencies]`, because drift
/// in a platform-gated pin is still drift.
fn classify_section(header: &str) -> Section {
    let mut parts = split_header(header);
    if parts.len() >= 3 && parts[0] == "target" {
        parts.drain(..2);
    }
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    match parts.as_slice() {
        ["package"] => Section::Package,
        ["workspace"] => Section::Workspace,
        ["workspace", "package"] => Section::WorkspacePackage,
        ["workspace", "dependencies"] => Section::WorkspaceDeps,
        ["workspace", "dependencies", name] if is_ident(name) => {
            Section::WorkspaceDepEntry((*name).to_string())
        }
        [kind] if is_dep_kind(kind) => Section::Deps,
        [kind, name] if is_dep_kind(kind) && is_ident(name) => {
            Section::DepEntry((*name).to_string())
        }
        _ => Section::Other,
    }
}

fn is_dep_kind(s: &str) -> bool {
    matches!(
        s,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    )
}

/// Split a table header on `.`, treating quoted spans as opaque.
///
/// `target.'cfg(target_os = "linux")'.dependencies` must not split inside the
/// predicate, which contains both dots and quotes of the other kind.
fn split_header(header: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;

    for c in header.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '\'' || c == '"' => quote = Some(c),
            None if c == '.' => {
                parts.push(cur.trim().to_string());
                cur.clear();
            }
            None => cur.push(c),
        }
    }
    parts.push(cur.trim().to_string());
    parts
}

/// Fold one `key = value` line of a `[dependencies.<name>]` table into its spec.
fn apply_entry_field(spec: &mut DepSpec, line: &str) {
    let Some((key, value)) = line.split_once('=') else {
        return;
    };
    let (key, value) = (key.trim(), value.trim());
    match key {
        "version" => spec.version = unquote(value).or_else(|| spec.version.take()),
        "path" => spec.path = unquote(value).or_else(|| spec.path.take()),
        "git" => spec.git = unquote(value).or_else(|| spec.git.take()),
        "workspace" => spec.workspace = value.contains("true"),
        _ => {}
    }
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

/// Read `key = "value"` out of a single-line inline table.
///
/// The key must start a field, so looking up `path` does not match the `path`
/// inside a key like `default-path` or a value like `git = "…/path"`.
fn table_field(table: &str, key: &str) -> Option<String> {
    let mut from = 0usize;
    while let Some(rel) = table[from..].find(key) {
        let start = from + rel;
        from = start + key.len();

        let starts_field = table[..start]
            .chars()
            .next_back()
            .is_none_or(|c| c == '{' || c == ',' || c.is_whitespace());
        if !starts_field {
            continue;
        }
        let after = table[from..].trim_start();
        let Some(after) = after.strip_prefix('=') else {
            continue;
        };
        if let Some(value) = after.trim_start().strip_prefix('"') {
            if let Some(end) = value.find('"') {
                return Some(value[..end].to_string());
            }
        }
    }
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

/// Directories a manifest walk never descends into.
///
/// `dev_docs` skips local scratch trees if present. Note that the lockfile walk
/// deliberately keeps its own, narrower set — see `find_cargo_locks`.
pub fn is_pruned(name: &str) -> bool {
    matches!(name, "target" | "node_modules" | ".git" | "dev_docs")
}

/// Every `Cargo.toml` under `root`, sorted.
pub fn find_cargo_tomls(root: &Path) -> Result<Vec<PathBuf>> {
    find_files_named(root, "Cargo.toml", is_pruned)
}

/// Every `Cargo.lock` under `root`, sorted.
///
/// The prune set is narrower than `is_pruned` on purpose: `lock-fingerprint`
/// has always descended into `dev_docs`, and these fingerprints feed CI cache
/// keys, so quietly dropping one would silently change a key.
pub fn find_cargo_locks(root: &Path) -> Result<Vec<PathBuf>> {
    find_files_named(root, "Cargo.lock", |name| {
        matches!(name, "target" | "node_modules" | ".git")
    })
}

/// Every file called `filename` under `root`, sorted.
///
/// A directory walk is inherently sequential, and on a large monorepo it is the
/// single slowest step left. So walk breadth-first only until there are enough
/// independent subtrees to keep every core busy, then walk those in parallel.
/// Sharding on the top level alone would not do: the usual monorepo shape puts
/// every crate under one `crates/`, which is a single shard and no faster.
///
/// The result is sorted, so it does not depend on which thread finished first.
pub fn find_files_named(
    root: &Path,
    filename: &str,
    prune: fn(&str) -> bool,
) -> Result<Vec<PathBuf>> {
    // A walk rooted at a pruned directory yields nothing — matching the
    // filter that used to be applied to the walk's own root entry.
    if root
        .file_name()
        .is_some_and(|n| prune(&n.to_string_lossy()))
    {
        return Ok(Vec::new());
    }

    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let mut out = Vec::new();
    let mut frontier = vec![root.to_path_buf()];

    if workers > 1 {
        // Stop splitting once there is enough work to go round; deeper
        // breadth-first scanning is itself serial and would eat the win.
        let want = workers * 4;
        while !frontier.is_empty() && frontier.len() < want {
            let mut next = Vec::new();
            for dir in &frontier {
                read_children(dir, filename, prune, &mut out, &mut next);
            }
            frontier = next;
        }
    }

    if workers > 1 && frontier.len() > 1 {
        let chunk = frontier.len().div_ceil(workers);
        std::thread::scope(|scope| -> Result<()> {
            let handles: Vec<_> = frontier
                .chunks(chunk)
                .map(|slice| {
                    scope.spawn(move || {
                        let mut acc = Vec::new();
                        for dir in slice {
                            walk_subtree(dir, filename, prune, &mut acc);
                        }
                        acc
                    })
                })
                .collect();
            for handle in handles {
                out.extend(
                    handle
                        .join()
                        .map_err(|_| anyhow!("directory walker thread panicked"))?,
                );
            }
            Ok(())
        })?;
    } else {
        for dir in &frontier {
            walk_subtree(dir, filename, prune, &mut out);
        }
    }

    out.sort();
    Ok(out)
}

/// One directory level: manifests directly inside, and subdirectories to visit.
///
/// `file_type` here does not follow symlinks, so a symlinked directory is not
/// descended and a symlinked `Cargo.toml` is not collected — the same choice
/// `WalkDir` makes by default, and what keeps a symlink loop from hanging.
fn read_children(
    dir: &Path,
    filename: &str,
    prune: fn(&str) -> bool,
    found: &mut Vec<PathBuf>,
    subdirs: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return; // unreadable directories are skipped, as before
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if !prune(&name) {
                subdirs.push(entry.path());
            }
        } else if file_type.is_file() && name == filename {
            found.push(entry.path());
        }
    }
}

fn walk_subtree(dir: &Path, filename: &str, prune: fn(&str) -> bool, out: &mut Vec<PathBuf>) {
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| !prune(&e.file_name().to_string_lossy()))
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name() == filename {
            out.push(entry.path().to_path_buf());
        }
    }
}

pub fn load_manifest(path: &Path) -> Result<ParsedManifest> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(parse_manifest(&text))
}

/// Below this many manifests, threads cost more than the read they save.
const PARALLEL_MIN: usize = 64;

/// Read and parse every manifest, in the order given.
///
/// Reading and parsing manifests dominates every tree-walking command, and each
/// file is independent, so hand each core a contiguous slice. Output order
/// matches `paths` regardless of thread scheduling — drift reports key off it,
/// and a report that reorders between runs on one tree is not a report.
pub fn load_manifests(paths: &[PathBuf]) -> Result<Vec<ParsedManifest>> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(paths.len());

    if workers <= 1 || paths.len() < PARALLEL_MIN {
        return paths.iter().map(|p| load_manifest(p)).collect();
    }

    let chunk = paths.len().div_ceil(workers);
    let mut out = Vec::with_capacity(paths.len());
    std::thread::scope(|scope| -> Result<()> {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|slice| {
                scope.spawn(move || {
                    slice
                        .iter()
                        .map(|p| load_manifest(p))
                        .collect::<Result<Vec<_>>>()
                })
            })
            .collect();
        for handle in handles {
            let parsed = handle
                .join()
                .map_err(|_| anyhow!("manifest reader thread panicked"))??;
            out.extend(parsed);
        }
        Ok(())
    })?;
    Ok(out)
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
        assert_eq!(
            m.deps.get("clap").and_then(|d| d.version.as_deref()),
            Some("4")
        );
        assert_eq!(
            m.deps.get("dirs").and_then(|d| d.version.as_deref()),
            Some("6")
        );
        assert!(m.deps.get("anyhow").is_some_and(|d| d.workspace));
        assert!(m.deps.get("tokio").is_some_and(|d| d.workspace));
        assert_eq!(
            m.deps.get("sdsk-client").and_then(|d| d.path.as_deref()),
            Some("../crates/sdsk-client")
        );
    }

    #[test]
    fn parses_dependency_tables() {
        let m = parse_manifest(
            r#"
[dependencies.serde]
version = "1.0"
features = ["derive"]

[dev-dependencies.tempfile]
version = "3"

[build-dependencies.cc]
workspace = true

[dependencies.local]
path = "../local"
"#,
        );
        assert_eq!(
            m.deps.get("serde").and_then(|d| d.version.as_deref()),
            Some("1.0")
        );
        assert_eq!(
            m.deps.get("tempfile").and_then(|d| d.version.as_deref()),
            Some("3")
        );
        assert!(m.deps.get("cc").is_some_and(|d| d.workspace));
        assert_eq!(
            m.deps.get("local").and_then(|d| d.path.as_deref()),
            Some("../local")
        );
    }

    #[test]
    fn parses_target_specific_dependencies() {
        let m = parse_manifest(
            r#"
[target.'cfg(unix)'.dependencies]
libc = "0.2"

[target.'cfg(target_os = "linux")'.dependencies]
inotify = "0.10"

[target.'cfg(windows)'.dependencies.windows-sys]
version = "0.59"

[target.x86_64-pc-windows-msvc.dev-dependencies]
winapi = "0.3"
"#,
        );
        // A dotted, quote-bearing cfg predicate must not split the header.
        assert_eq!(
            m.deps.get("libc").and_then(|d| d.version.as_deref()),
            Some("0.2")
        );
        assert_eq!(
            m.deps.get("inotify").and_then(|d| d.version.as_deref()),
            Some("0.10")
        );
        assert_eq!(
            m.deps.get("windows-sys").and_then(|d| d.version.as_deref()),
            Some("0.59")
        );
        assert_eq!(
            m.deps.get("winapi").and_then(|d| d.version.as_deref()),
            Some("0.3")
        );
    }

    #[test]
    fn parses_workspace_dependency_tables() {
        let m = parse_manifest(
            r#"
[workspace.dependencies.tokio]
version = "1.40"
features = ["full"]
"#,
        );
        assert_eq!(
            m.workspace_deps
                .get("tokio")
                .and_then(|d| d.version.as_deref()),
            Some("1.40")
        );
        // The pin must not leak into the package's own dependency set.
        assert!(!m.deps.contains_key("tokio"));
    }

    #[test]
    fn table_field_requires_a_field_boundary() {
        // `path` occurs inside the git URL and inside `default-path`, but
        // neither starts a field, so neither may be read as `path =`.
        let m = parse_manifest(
            r#"
[dependencies]
a = { git = "https://example.com/path", default-path = "x", version = "2" }
"#,
        );
        let a = m.deps.get("a").expect("dep a");
        assert_eq!(a.path, None);
        assert_eq!(a.version.as_deref(), Some("2"));
        assert_eq!(a.git.as_deref(), Some("https://example.com/path"));
    }

    #[test]
    fn non_dependency_tables_are_ignored() {
        let m = parse_manifest(
            r#"
[package]
name = "app"
version = "0.1.0"

[[bin]]
name = "app"
path = "src/main.rs"

[profile.release]
version = "not-a-dep"

[features]
default = []
"#,
        );
        assert_eq!(m.package_name.as_deref(), Some("app"));
        assert_eq!(m.package_version.as_deref(), Some("0.1.0"));
        assert!(m.deps.is_empty(), "found stray deps: {:?}", m.deps.keys());
    }

    /// Reference walk: the straightforward single-threaded version, kept to
    /// hold the parallel one to account on whatever tree a test builds.
    fn serial_find(root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| !is_pruned(&e.file_name().to_string_lossy()))
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() && entry.file_name() == "Cargo.toml" {
                out.push(entry.path().to_path_buf());
            }
        }
        out.sort();
        out
    }

    fn scratch(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cargo-tog-{tag}-{stamp}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn parallel_walk_matches_the_serial_walk() {
        let dir = scratch("walk");
        // Wide enough to pass the frontier threshold on any core count, and
        // deep enough that the split happens below the top level — the shape a
        // real monorepo has, where everything hangs off one `crates/`.
        for i in 0..200 {
            let crate_dir = dir.join("crates").join(format!("c{i:03}"));
            fs::create_dir_all(crate_dir.join("src")).unwrap();
            fs::write(
                crate_dir.join("Cargo.toml"),
                format!("[package]\nname = \"c{i:03}\"\nversion = \"0.1.0\"\n"),
            )
            .unwrap();
            // Noise that must never be collected.
            for skip in ["target", "node_modules", ".git", "dev_docs"] {
                let junk = crate_dir.join(skip);
                fs::create_dir_all(&junk).unwrap();
                fs::write(junk.join("Cargo.toml"), "[package]\nname = \"junk\"\n").unwrap();
            }
        }
        fs::write(dir.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::create_dir_all(dir.join("target/deep")).unwrap();
        fs::write(dir.join("target/deep/Cargo.toml"), "[package]\n").unwrap();

        let found = find_cargo_tomls(&dir).unwrap();
        assert_eq!(found, serial_find(&dir), "parallel walk diverged");
        assert_eq!(found.len(), 201, "expected 200 crates + workspace root");
        assert!(
            !found.iter().any(|p| p.components().any(|c| {
                matches!(
                    c.as_os_str().to_string_lossy().as_ref(),
                    "target" | "node_modules" | ".git" | "dev_docs"
                )
            })),
            "walk descended into a pruned directory"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_root_that_is_itself_pruned_yields_nothing() {
        // `--root ./target` found nothing before, because the prune filter also
        // applied to the walk's own root. Splitting the walk must not quietly
        // start returning results here.
        let dir = scratch("pruned-root").join("target");
        fs::create_dir_all(dir.join("app")).unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        fs::write(dir.join("app/Cargo.toml"), "[package]\nname = \"y\"\n").unwrap();

        assert!(find_cargo_tomls(&dir).unwrap().is_empty());
        assert_eq!(find_cargo_tomls(&dir).unwrap(), serial_find(&dir));
        let _ = fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn a_symlink_loop_does_not_hang_the_walk() {
        let dir = scratch("symlink");
        fs::create_dir_all(dir.join("real")).unwrap();
        fs::write(dir.join("real/Cargo.toml"), "[package]\nname = \"real\"\n").unwrap();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&dir, dir.join("real/loop")).is_ok();
        #[cfg(not(unix))]
        let linked = false;

        let found = find_cargo_tomls(&dir).unwrap();
        assert_eq!(found.len(), 1, "symlink was followed: {found:?}");
        assert_eq!(found, serial_find(&dir));
        let _ = linked;
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parallel_loading_preserves_input_order() {
        // Well past PARALLEL_MIN, so this really does cross threads. Callers zip
        // the result back against the path list, so any reordering would staple
        // each manifest onto a neighbour's path.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cargo-tog-order-{stamp}"));
        let _ = fs::remove_dir_all(&dir);

        let count = PARALLEL_MIN * 5;
        let mut paths = Vec::with_capacity(count);
        for i in 0..count {
            let crate_dir = dir.join(format!("c{i:04}"));
            fs::create_dir_all(&crate_dir).unwrap();
            let path = crate_dir.join("Cargo.toml");
            fs::write(
                &path,
                format!("[package]\nname = \"c{i:04}\"\nversion = \"0.1.0\"\n"),
            )
            .unwrap();
            paths.push(path);
        }
        paths.sort();

        let loaded = load_manifests(&paths).unwrap();
        assert_eq!(loaded.len(), paths.len());
        let names: Vec<&str> = loaded
            .iter()
            .map(|m| m.package_name.as_deref().unwrap_or("?"))
            .collect();
        let expected: Vec<String> = (0..count).map(|i| format!("c{i:04}")).collect();
        assert_eq!(names, expected, "parallel load reordered manifests");

        // And it must agree with reading them one at a time.
        let serial: Vec<_> = paths.iter().map(|p| load_manifest(p).unwrap()).collect();
        assert_eq!(
            names,
            serial
                .iter()
                .map(|m| m.package_name.as_deref().unwrap_or("?"))
                .collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loading_reports_the_file_that_failed() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cargo-tog-missing-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let good = dir.join("Cargo.toml");
        fs::write(&good, "[package]\nname = \"ok\"\nversion = \"0.1.0\"\n").unwrap();
        let missing = dir.join("gone/Cargo.toml");

        let err = load_manifests(&[good, missing.clone()]).unwrap_err();
        assert!(
            err.to_string().contains("gone"),
            "error lost the offending path: {err}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_dependencies_table_alone_marks_a_workspace_root() {
        // No bare `[workspace]` header — TOML still creates the `workspace`
        // table, and Cargo still reads these pins, so this manifest is a root.
        let m = parse_manifest(
            r#"
[workspace.dependencies]
serde = "1"
"#,
        );
        assert!(
            m.is_workspace,
            "headerless workspace root went unrecognized"
        );
        assert!(m.workspace_deps.contains_key("serde"));
    }

    #[test]
    fn a_plain_package_is_not_a_workspace_root() {
        let m = parse_manifest(
            r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
serde = { workspace = true }
"#,
        );
        assert!(!m.is_workspace);
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
            m.workspace_deps
                .get("tokio")
                .and_then(|d| d.version.as_deref()),
            Some("1")
        );
        assert_eq!(
            m.workspace_deps
                .get("serde")
                .and_then(|d| d.version.as_deref()),
            Some("1")
        );
    }
}
