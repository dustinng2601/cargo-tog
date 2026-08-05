//! Advanced optional partial mirrors — not required for caching.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use cargo_tog::paths::{expand_user, resolve_relative_to};

#[derive(Debug, Default)]
struct Mirror {
    name: String,
    source_root: String,
    target_root: String,
    files: Vec<(String, String)>,
}

pub fn run(config: &str, check: bool, apply: bool) -> Result<()> {
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
    if !check && !apply {
        bail!("pass --check or --apply");
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let hash = Sha256::digest(&bytes);
    Ok(hex::encode(hash))
}

fn parse_sync_config(text: &str) -> Vec<Mirror> {
    let mut mirrors = Vec::new();
    let mut cur: Option<Mirror> = None;
    let mut in_files = false;

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
        if line.starts_with('[') {
            let done = cur.take().unwrap();
            if !done.source_root.is_empty() && !done.target_root.is_empty() {
                mirrors.push(done);
            }
            in_files = false;
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"');
            if !in_files {
                match k {
                    "name" => m.name = v.to_string(),
                    "source_root" => m.source_root = expand_user(v).display().to_string(),
                    "target_root" => m.target_root = expand_user(v).display().to_string(),
                    "files" => in_files = true,
                    _ => {}
                }
            }
        }
        if in_files {
            // ["a", "b"]
            let parts: Vec<&str> = line
                .split('"')
                .enumerate()
                .filter_map(|(i, s)| if i % 2 == 1 { Some(s) } else { None })
                .collect();
            if parts.len() >= 2 {
                m.files.push((parts[0].to_string(), parts[1].to_string()));
            }
            if line == "]" {
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
