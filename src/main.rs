//! cargo-tog — enterprise Cargo build-cache coordination (cross-OS).

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "cargo-tog",
    version,
    about = "Enterprise Cargo build-cache coordination (macOS / Linux / Windows)",
    long_about = "Coordinate registry and compiler-object caches across projects and OSes.\n\n\
Compiler objects are per target triple (not shared across macOS/Linux/Windows).\n\
Registry downloads can be shared. Source sync is advanced/optional only.\n\
See docs/CROSS_OS.md and docs/PRODUCTION.md."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Health check: toolchain, wrapper, cache environment, host triple
    Doctor,
    /// Print production share policy (what to cache / not cache)
    CachePlan,
    /// Map packages and workspace dependency pins under a tree
    Inventory {
        #[arg(long, default_value = ".")]
        root: String,
    },
    /// Deep dependency drift (manifests + lockfiles) between two trees
    DepDrift(commands::dep_drift::DepDriftArgs),
    /// Hash Cargo.lock files (for CI cache keys)
    LockFingerprint {
        #[arg(long, default_value = ".")]
        root: String,
    },
    /// Print recommended CI cache key fragments for this host
    HostKey,
    /// Advanced: partial file mirrors (optional; not required for cache)
    Sync {
        #[arg(long, default_value = "cargo-tog.toml")]
        config: String,
        #[arg(long)]
        check: bool,
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
        Commands::DepDrift(args) => commands::dep_drift::run_args(args),
        Commands::LockFingerprint { root } => commands::lock_fingerprint::run(&root),
        Commands::HostKey => {
            commands::host_key::run();
            Ok(())
        }
        Commands::Sync {
            config,
            check,
            apply,
        } => commands::sync::run(&config, check || !apply, apply),
    }
}
