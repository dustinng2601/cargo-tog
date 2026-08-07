//! Advanced optional partial mirrors — not required for caching.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use cargo_tog::paths::{expand_user, resolve_relative_to};

#[derive(Debug, Default)]
struct Mirror {
    name: String,
    source_root: String,
    target_root: String,
    files: Vec<(String, String)>,
}

/// Compare (and optionally copy) the files each mirror lists.
///
/// With neither flag this reports drift without writing: `--apply` is the only
/// path that touches the target tree, so the read-only reading is the safe one.
pub fn run(config: &str, check: bool, apply: bool) -> Result<()> {
    let check = check || !apply;

    println!("cargo-tog sync is advanced/optional (not part of core cache).\n");

    let config_path = if Path::new(config).is_absolute() {
        PathBuf::from(config)
    } else {
        std::env::current_dir()?.join(config)
    };

    if !config_path.is_file() {
        eprintln!("config not found: {}", config_path.display());
        eprintln!("Most teams never need sync. Caching works without it.");
        eprintln!("See docs/SYNC.md only if you maintain partial file mirrors.");
        std::process::exit(1);
    }

    let text = fs::read_to_string(&config_path)?;
    let mirrors = parse_sync_config(&text);
    if mirrors.is_empty() {
        println!("no [[sync.mirrors]] entries — nothing to do (cache does not need sync).");
        return Ok(());
    }

    let mut drifted = 0usize;
    let mut copied = 0usize;

    for mirror in &mirrors {
        let src_root = resolve_relative_to(&config_path, &mirror.source_root);
        let dst_root = resolve_relative_to(&config_path, &mirror.target_root);
        let label = if mirror.name.is_empty() {
            "(unnamed)"
        } else {
            mirror.name.as_str()
        };
        println!("mirror {label}");
        println!("  source: {}", src_root.display());
        println!("  target: {}", dst_root.display());

        for (from, to) in &mirror.files {
            let s = src_root.join(from);
            let d = dst_root.join(to);
            if !s.is_file() {
                println!("  MISSING source {from}");
                drifted += 1;
                continue;
            }
            if !d.is_file() {
                println!("  missing target {to}");
                drifted += 1;
                if apply {
                    if let Some(parent) = d.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&s, &d)
                        .with_context(|| format!("copy {} → {}", s.display(), d.display()))?;
                    copied += 1;
                    println!("  copied → {to}");
                }
                continue;
            }
            let sh = file_sha256(&s)?;
            let dh = file_sha256(&d)?;
            if sh == dh {
                println!("  ok  {from} → {to}");
            } else {
                println!("  DIFF {from} → {to}");
                drifted += 1;
                if apply {
                    fs::copy(&s, &d)?;
                    copied += 1;
                    println!("  copied → {to}");
                }
            }
        }
    }

    if apply {
        println!("\napplied {copied} file(s); commit/push in the target repo yourself.");
    } else if drifted > 0 {
        println!("\n{drifted} path(s) drifted. Re-run with --apply to copy.");
    } else {
        println!("\nall listed files in sync.");
    }

    if check && !apply && drifted > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let hash = Sha256::digest(&bytes);
    Ok(hex::encode(hash))
}

/// Every double-quoted span on a line, in order.
fn quoted_strings(line: &str) -> Vec<String> {
    line.split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_string())
        .collect()
}

/// Net `[` minus `]`, ignoring brackets inside quoted filenames.
fn bracket_delta(line: &str) -> i32 {
    let mut delta = 0;
    let mut in_quote = false;
    for c in line.chars() {
        match c {
            '"' => in_quote = !in_quote,
            '[' if !in_quote => delta += 1,
            ']' if !in_quote => delta -= 1,
            _ => {}
        }
    }
    delta
}

fn parse_sync_config(text: &str) -> Vec<Mirror> {
    let mut mirrors = Vec::new();
    let mut cur: Option<Mirror> = None;
    let mut in_files = false;
    let mut depth = 0i32;

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[sync.mirrors]]" {
            if let Some(m) = cur.take() {
                if !m.source_root.is_empty() && !m.target_root.is_empty() {
                    mirrors.push(m);
                }
            }
            cur = Some(Mirror::default());
            in_files = false;
            continue;
        }
        let Some(m) = cur.as_mut() else { continue };
        // Only outside a `files` array does a leading `[` mean a new table —
        // inside one it is an ["from", "to"] element, and treating those as
        // headers ended the mirror on its first entry.
        if !in_files && line.starts_with('[') {
            let done = cur.take().unwrap();
            if !done.source_root.is_empty() && !done.target_root.is_empty() {
                mirrors.push(done);
            }
            in_files = false;
            continue;
        }
        if !in_files {
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim().trim_matches('"');
                match k {
                    "name" => m.name = v.to_string(),
                    "source_root" => m.source_root = expand_user(v).display().to_string(),
                    "target_root" => m.target_root = expand_user(v).display().to_string(),
                    "files" => {
                        in_files = true;
                        depth = 0;
                    }
                    _ => {}
                }
            }
        }
        if in_files {
            // Every ["from", "to"] pair on this line, however many.
            let quoted = quoted_strings(line);
            for pair in quoted.chunks_exact(2) {
                m.files.push((pair[0].clone(), pair[1].clone()));
            }
            // Track nesting instead of matching a bare `]`, so a single-line
            // `files = [["a", "b"]]` closes the array like the multi-line form.
            // Leaving it open would swallow every later key in the mirror.
            depth += bracket_delta(line);
            if depth <= 0 {
                in_files = false;
            }
        }
    }
    if let Some(m) = cur {
        if !m.source_root.is_empty() && !m.target_root.is_empty() {
            mirrors.push(m);
        }
    }
    mirrors
}

#[cfg(test)]
mod tests {
    use super::*;

    const MULTILINE: &str = r#"
[[sync.mirrors]]
name = "split"
source_root = "../main"
target_root = "../split"
files = [
  ["src/lib.rs", "src/lib.rs"],
  ["src/api.rs", "src/api.rs"],
]
"#;

    #[test]
    fn parses_the_documented_multiline_form() {
        let mirrors = parse_sync_config(MULTILINE);
        assert_eq!(mirrors.len(), 1);
        assert_eq!(mirrors[0].name, "split");
        assert_eq!(mirrors[0].files.len(), 2);
        assert_eq!(
            mirrors[0].files[1],
            ("src/api.rs".to_string(), "src/api.rs".to_string())
        );
    }

    #[test]
    fn a_single_line_files_array_closes() {
        // The array used to stay open forever, swallowing every later key.
        let mirrors = parse_sync_config(
            r#"
[[sync.mirrors]]
files = [["a.rs", "b.rs"]]
name = "named-after-files"
source_root = "../main"
target_root = "../split"
"#,
        );
        assert_eq!(mirrors.len(), 1, "mirror was dropped: {mirrors:?}");
        assert_eq!(mirrors[0].name, "named-after-files");
        assert_eq!(mirrors[0].source_root, "../main");
        assert_eq!(
            mirrors[0].files,
            vec![("a.rs".to_string(), "b.rs".to_string())]
        );
    }

    #[test]
    fn several_pairs_on_one_line_are_all_kept() {
        let mirrors = parse_sync_config(
            r#"
[[sync.mirrors]]
source_root = "../main"
target_root = "../split"
files = [["a", "b"], ["c", "d"]]
"#,
        );
        assert_eq!(mirrors[0].files.len(), 2, "{:?}", mirrors[0].files);
        assert_eq!(mirrors[0].files[1], ("c".to_string(), "d".to_string()));
    }

    #[test]
    fn several_mirrors_are_separated() {
        let mut text = MULTILINE.to_string();
        text.push_str(
            r#"
[[sync.mirrors]]
name = "second"
source_root = "../a"
target_root = "../b"
files = [["x", "y"]]
"#,
        );
        let mirrors = parse_sync_config(&text);
        assert_eq!(mirrors.len(), 2);
        assert_eq!(mirrors[1].name, "second");
        assert_eq!(mirrors[1].files.len(), 1);
    }

    #[test]
    fn mirrors_without_both_roots_are_dropped() {
        let mirrors = parse_sync_config("[[sync.mirrors]]\nname = \"incomplete\"\n");
        assert!(mirrors.is_empty(), "{mirrors:?}");
    }

    #[test]
    fn an_unrelated_table_ends_the_mirror() {
        let mirrors = parse_sync_config(
            r#"
[[sync.mirrors]]
source_root = "../main"
target_root = "../split"
files = [["a", "b"]]

[cache]
share_target_dir = false
"#,
        );
        assert_eq!(mirrors.len(), 1);
        assert_eq!(mirrors[0].files.len(), 1, "{:?}", mirrors[0].files);
    }

    #[test]
    fn brackets_inside_filenames_do_not_unbalance_the_array() {
        let mirrors = parse_sync_config(
            r#"
[[sync.mirrors]]
source_root = "../main"
target_root = "../split"
files = [
  ["src/a[1].rs", "src/a[1].rs"],
]
name = "after"
"#,
        );
        assert_eq!(mirrors[0].files.len(), 1);
        assert_eq!(mirrors[0].name, "after", "array closed too early or late");
    }

    #[test]
    fn comments_are_ignored() {
        let mirrors = parse_sync_config(
            r#"
# leading comment
[[sync.mirrors]]
source_root = "../main"   # trailing
target_root = "../split"
files = [["a", "b"]]
"#,
        );
        assert_eq!(mirrors.len(), 1);
        assert_eq!(mirrors[0].source_root, "../main");
    }
}
