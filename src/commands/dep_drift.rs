use anyhow::{bail, Result};

use crate::cargo_toml::collect_explicit_deps;
use crate::paths::resolve_path;

pub fn run(master: &str, other: &str) -> Result<()> {
    let master = resolve_path(master);
    let other = resolve_path(other);
    if !master.is_dir() || !other.is_dir() {
        bail!("usage: cargo-tog dep-drift --master <path> --other <path>");
    }

    let a = collect_explicit_deps(&master)?;
    let b = collect_explicit_deps(&other)?;

    println!("dep-drift");
    println!("  master: {}", master.display());
    println!("  other:  {}\n", other.display());

    let mut drifts = 0usize;
    let mut names: Vec<_> = a.keys().chain(b.keys()).collect();
    names.sort();
    names.dedup();

    for name in names {
        let Some(av) = a.get(name.as_str()) else { continue };
        let Some(bv) = b.get(name.as_str()) else { continue };
        if av != bv {
            drifts += 1;
            println!(
                "  {name}: master=[{}] other=[{}]",
                av.join("|"),
                bv.join("|")
            );
        }
    }

    if drifts == 0 {
        println!("  no overlapping explicit version drifts found");
        Ok(())
    } else {
        println!("\n{drifts} drifted crate(s)");
        std::process::exit(1);
    }
}
