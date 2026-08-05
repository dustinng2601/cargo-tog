use std::fs;
use std::io::Read;

use anyhow::Result;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::paths::resolve_path;

pub fn run(root: &str) -> Result<()> {
    let root = resolve_path(root);
    let mut locks = Vec::new();
    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != "target" && n != "node_modules" && n != ".git"
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name() == "Cargo.lock" {
            locks.push(entry.path().to_path_buf());
        }
    }
    locks.sort();

    if locks.is_empty() {
        println!("no Cargo.lock under {}", root.display());
        return Ok(());
    }

    for path in locks {
        let mut file = fs::File::open(&path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        let hash = Sha256::digest(&buf);
        let short = hex::encode(&hash[..8]);
        let rel = path.strip_prefix(&root).unwrap_or(&path);
        println!("{short}  {}", rel.display());
    }
    Ok(())
}
