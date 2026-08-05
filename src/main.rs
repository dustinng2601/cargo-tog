//! cargo-tog — enterprise Cargo build-cache coordination.
//!
//! Core: doctor, cache-plan, inventory, dep-drift, lock-fingerprint.
//! Advanced (optional): sync — not required for caching.

mod cargo_toml;
mod commands;
mod paths;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "cargo-tog",
    version,
    about = "Enterprise Cargo build-cache coordination for monorepos and polyrepos",
    long_about = "Coordinate registry and compiler-object caches across projects.\n\n\
Source file sync is an advanced optional utility — caching does not require it.\n\
See docs/PRODUCTION.md and docs/RESEARCH.md."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Health check: toolchain, wrapper, cache environment
    Doctor,
    /// Print production share policy (what to cache / not cache)
    CachePlan,
    /// Map packages and workspace dependency pins under a tree
    Inventory {
        /// Workspace or monorepo root
        #[arg(long, default_value = ".")]
        root: String,
    },
    /// Compare explicit dependency versions between two trees
    DepDrift {
        #[arg(long)]
        master: String,
        #[arg(long)]
        other: String,
    },
    /// Hash Cargo.lock files (for CI cache keys)
    LockFingerprint {
        #[arg(long, default_value = ".")]
        root: String,
    },
    /// Advanced: partial file mirrors (optional; not required for cache)
    Sync {
        #[arg(long, default_value = "cargo-tog.toml")]
        config: String,
        /// Report drift only (default if --apply is absent)
        #[arg(long)]
        check: bool,
        /// Copy source → target for listed files
        #[arg(long)]
        apply: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Doctor => commands::doctor::run(),
        Commands::CachePlan => {
            commands::cache_plan::run();
            Ok(())
        }
        Commands::Inventory { root } => commands::inventory::run(&root),
        Commands::DepDrift { master, other } => commands::dep_drift::run(&master, &other),
        Commands::LockFingerprint { root } => commands::lock_fingerprint::run(&root),
        Commands::Sync {
            config,
            check,
            apply,
        } => commands::sync::run(&config, check || !apply, apply),
    }
}
