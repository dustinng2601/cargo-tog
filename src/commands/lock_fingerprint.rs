use std::fs;
use std::io::Read;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use cargo_tog::paths::resolve_path;

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

    // Hashing is per-file and independent; a polyrepo checkout can hold
    // hundreds of lockfiles. Chunk across cores, keeping `locks` order so the
    // printed fingerprints stay stable between runs on one tree.
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(locks.len());
    let chunk = locks.len().div_ceil(workers.max(1));

    let mut digests: Vec<String> = Vec::with_capacity(locks.len());
    std::thread::scope(|scope| -> Result<()> {
        let handles: Vec<_> = locks
            .chunks(chunk)
            .map(|slice| {
                scope.spawn(move || slice.iter().map(short_digest).collect::<Result<Vec<_>>>())
            })
            .collect();
        for handle in handles {
            let part = handle
                .join()
                .map_err(|_| anyhow!("lockfile hasher thread panicked"))??;
            digests.extend(part);
        }
        Ok(())
    })?;

    for (path, short) in locks.iter().zip(digests) {
        let rel = path.strip_prefix(&root).unwrap_or(path);
        println!("{short}  {}", rel.display());
    }
    Ok(())
}

fn short_digest(path: &std::path::PathBuf) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(hex::encode(&Sha256::digest(&buf)[..8]))
}
