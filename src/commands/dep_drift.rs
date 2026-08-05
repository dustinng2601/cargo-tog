//! Deep dependency drift between two Cargo trees (manifests + optional lockfiles).

use std::collections::BTreeSet;

use anyhow::{bail, Result};
use clap::Args;
use serde::Serialize;

use cargo_tog::dep_graph::DepGraph;
use cargo_tog::lockfile::{find_primary_lock, LockIndex};
use cargo_tog::paths::resolve_path;

#[derive(Debug, Args)]
pub struct DepDriftArgs {
    /// Reference tree (usually monorepo / source of truth)
    #[arg(long)]
    pub master: String,

    /// Comparison tree (split repo, mirror, fork, …)
    #[arg(long)]
    pub other: String,

    /// Machine-readable JSON report
    #[arg(long)]
    pub json: bool,

    /// Also compare Cargo.lock exact versions when both sides have a lockfile (default: on)
    #[arg(long, default_value_t = true)]
    pub lock: bool,

    /// Skip lockfile comparison
    #[arg(long)]
    pub no_lock: bool,

    /// Comma-separated crate names to ignore
    #[arg(long, value_delimiter = ',')]
    pub ignore: Vec<String>,

    /// Include path dependencies in drift (default: skip path-only keys)
    #[arg(long)]
    pub include_path: bool,

    /// Show crates that match as well
    #[arg(long)]
    pub show_ok: bool,

    /// Exit 0 even when drift is found
    #[arg(long)]
    pub warn_only: bool,

    /// Fail if other has crates master lacks (default: no)
    #[arg(long)]
    pub fail_extra: bool,

    /// Fail if master has crates other lacks among third-party (default: no)
    #[arg(long)]
    pub fail_missing: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    master: String,
    other: String,
    master_manifests: usize,
    other_manifests: usize,
    master_workspace_pins: usize,
    other_workspace_pins: usize,
    lock_compared: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    req_drift: Vec<ReqDrift>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lock_drift: Vec<LockDrift>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unresolved_workspace_other: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unresolved_workspace_master: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    only_master: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    only_other: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ok: Vec<String>,
    summary: Summary,
}

#[derive(Debug, Serialize)]
struct ReqDrift {
    crate_name: String,
    master: String,
    other: String,
}

#[derive(Debug, Serialize)]
struct LockDrift {
    crate_name: String,
    master_versions: Vec<String>,
    other_versions: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct Summary {
    req_drifts: usize,
    lock_drifts: usize,
    unresolved_other: usize,
    only_master: usize,
    only_other: usize,
    ok: usize,
    exit_worthy: bool,
}

pub fn run_args(args: DepDriftArgs) -> Result<()> {
    let master = resolve_path(&args.master);
    let other = resolve_path(&args.other);
    if !master.is_dir() || !other.is_dir() {
        bail!("both --master and --other must be directories");
    }

    let ignore: BTreeSet<String> = args.ignore.iter().cloned().collect();
    let g_m = DepGraph::collect(&master)?;
    let g_o = DepGraph::collect(&other)?;

    let compare_lock = args.lock && !args.no_lock;
    let lock_m = if compare_lock {
        find_primary_lock(&master).and_then(|p| LockIndex::load(&p).ok())
    } else {
        None
    };
    let lock_o = if compare_lock {
        find_primary_lock(&other).and_then(|p| LockIndex::load(&p).ok())
    } else {
        None
    };
    let lock_compared = lock_m.is_some() && lock_o.is_some();

    let mut report = Report {
        master: master.display().to_string(),
        other: other.display().to_string(),
        master_manifests: g_m.manifests,
        other_manifests: g_o.manifests,
        master_workspace_pins: g_m.workspace_pins.len(),
        other_workspace_pins: g_o.workspace_pins.len(),
        lock_compared,
        req_drift: Vec::new(),
        lock_drift: Vec::new(),
        unresolved_workspace_other: Vec::new(),
        unresolved_workspace_master: Vec::new(),
        only_master: Vec::new(),
        only_other: Vec::new(),
        ok: Vec::new(),
        summary: Summary::default(),
    };

    // Unresolved workspace refs
    for (name, reqs) in &g_o.deps {
        if ignore.contains(name) {
            continue;
        }
        if reqs
            .iter()
            .any(|r| matches!(r, cargo_tog::dep_graph::ResolvedReq::UnresolvedWorkspace))
        {
            report.unresolved_workspace_other.push(name.clone());
        }
    }
    for (name, reqs) in &g_m.deps {
        if ignore.contains(name) {
            continue;
        }
        if reqs
            .iter()
            .any(|r| matches!(r, cargo_tog::dep_graph::ResolvedReq::UnresolvedWorkspace))
        {
            report.unresolved_workspace_master.push(name.clone());
        }
    }
    report.unresolved_workspace_other.sort();
    report.unresolved_workspace_master.sort();

    // Presence
    let names_m: BTreeSet<_> = g_m.deps.keys().cloned().collect();
    let names_o: BTreeSet<_> = g_o.deps.keys().cloned().collect();
    for n in names_m.difference(&names_o) {
        if ignore.contains(n) {
            continue;
        }
        if !args.include_path && only_path(&g_m, n) {
            continue;
        }
        report.only_master.push(n.clone());
    }
    for n in names_o.difference(&names_m) {
        if ignore.contains(n) {
            continue;
        }
        if !args.include_path && only_path(&g_o, n) {
            continue;
        }
        report.only_other.push(n.clone());
    }

    // Requirement drift on intersection
    for name in names_m.intersection(&names_o) {
        if ignore.contains(name) {
            continue;
        }
        let km = filter_keys(&g_m, name, args.include_path);
        let ko = filter_keys(&g_o, name, args.include_path);
        if km.is_empty() && ko.is_empty() {
            continue;
        }
        if km == ko {
            if args.show_ok {
                report.ok.push(name.clone());
            }
        } else {
            report.req_drift.push(ReqDrift {
                crate_name: name.clone(),
                master: g_m.summary(name).unwrap_or_default(),
                other: g_o.summary(name).unwrap_or_default(),
            });
        }
    }

    // Lockfile exact version drift
    if let (Some(lm), Some(lo)) = (&lock_m, &lock_o) {
        let lock_names: BTreeSet<_> = lm.versions.keys().chain(lo.versions.keys()).cloned().collect();
        for name in lock_names {
            if ignore.contains(&name) {
                continue;
            }
            // Only care about crates that appear as direct deps on either side, or all?
            // Direct-dep focused is less noise; also include if both have it.
            let direct = g_m.deps.contains_key(&name) || g_o.deps.contains_key(&name);
            if !direct {
                continue;
            }
            let vm = lm.get(&name).unwrap_or(&[]);
            let vo = lo.get(&name).unwrap_or(&[]);
            if vm.is_empty() || vo.is_empty() {
                continue;
            }
            if vm != vo {
                report.lock_drift.push(LockDrift {
                    crate_name: name,
                    master_versions: vm.to_vec(),
                    other_versions: vo.to_vec(),
                });
            }
        }
    }

    report.summary = Summary {
        req_drifts: report.req_drift.len(),
        lock_drifts: report.lock_drift.len(),
        unresolved_other: report.unresolved_workspace_other.len(),
        only_master: report.only_master.len(),
        only_other: report.only_other.len(),
        ok: report.ok.len(),
        exit_worthy: false,
    };

    let exit_worthy = report.summary.req_drifts > 0
        || report.summary.lock_drifts > 0
        || report.summary.unresolved_other > 0
        || (args.fail_extra && report.summary.only_other > 0)
        || (args.fail_missing && report.summary.only_master > 0);
    report.summary.exit_worthy = exit_worthy;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human(&report, &args, lock_m.as_ref(), lock_o.as_ref());
    }

    if exit_worthy && !args.warn_only {
        std::process::exit(1);
    }
    Ok(())
}

fn only_path(g: &DepGraph, name: &str) -> bool {
    g.deps.get(name).is_some_and(|s| s.iter().all(|r| r.is_path()))
}

fn filter_keys(g: &DepGraph, name: &str, include_path: bool) -> BTreeSet<String> {
    g.deps
        .get(name)
        .map(|s| {
            s.iter()
                .filter(|r| include_path || !r.is_path())
                .map(|r| r.drift_key())
                .collect()
        })
        .unwrap_or_default()
}

fn print_human(
    r: &Report,
    args: &DepDriftArgs,
    lock_m: Option<&LockIndex>,
    lock_o: Option<&LockIndex>,
) {
    println!("dep-drift");
    println!("  master: {} ({} manifests, {} workspace pins)", r.master, r.master_manifests, r.master_workspace_pins);
    println!("  other:  {} ({} manifests, {} workspace pins)", r.other, r.other_manifests, r.other_workspace_pins);
    if r.lock_compared {
        println!(
            "  lock:   {}  vs  {}",
            lock_m.map(|l| l.path.display().to_string()).unwrap_or_default(),
            lock_o.map(|l| l.path.display().to_string()).unwrap_or_default()
        );
    } else if args.lock && !args.no_lock {
        println!("  lock:   (skipped — need Cargo.lock on both sides)");
    } else {
        println!("  lock:   (disabled)");
    }
    println!();

    if !r.req_drift.is_empty() {
        println!("requirement drift ({}):", r.req_drift.len());
        for d in &r.req_drift {
            println!("  {}:", d.crate_name);
            println!("    master: {}", d.master);
            println!("    other:  {}", d.other);
        }
        println!();
    }

    if !r.lock_drift.is_empty() {
        println!("lockfile exact-version drift ({} direct-dep crates):", r.lock_drift.len());
        for d in &r.lock_drift {
            println!(
                "  {}: master={:?} other={:?}",
                d.crate_name, d.master_versions, d.other_versions
            );
        }
        println!();
    }

    if !r.unresolved_workspace_other.is_empty() {
        println!(
            "unresolved workspace deps on other ({}):",
            r.unresolved_workspace_other.len()
        );
        println!(
            "  (other uses workspace = true but has no [workspace.dependencies] pin in that tree)"
        );
        for n in &r.unresolved_workspace_other {
            println!("  • {n}");
        }
        println!("  tip: copy pins from master [workspace.dependencies], or build only as a monorepo member");
        println!();
    }

    if !r.unresolved_workspace_master.is_empty() {
        println!(
            "unresolved workspace deps on master ({}): {:?}",
            r.unresolved_workspace_master.len(),
            r.unresolved_workspace_master
        );
        println!();
    }

    if !r.only_other.is_empty() {
        println!(
            "only in other ({}): {}",
            r.only_other.len(),
            r.only_other.join(", ")
        );
    }
    if !r.only_master.is_empty() {
        println!(
            "only in master ({}): {}",
            r.only_master.len(),
            truncate_list(&r.only_master, 30)
        );
    }
    if args.show_ok && !r.ok.is_empty() {
        println!("ok ({}): {}", r.ok.len(), truncate_list(&r.ok, 40));
    }

    println!();
    println!(
        "summary: req_drift={} lock_drift={} unresolved_other={} only_master={} only_other={}",
        r.summary.req_drifts,
        r.summary.lock_drifts,
        r.summary.unresolved_other,
        r.summary.only_master,
        r.summary.only_other
    );
    if r.summary.exit_worthy {
        println!("result: DRIFT (exit 1)");
    } else {
        println!("result: clean");
    }
}

fn truncate_list(items: &[String], max: usize) -> String {
    if items.len() <= max {
        items.join(", ")
    } else {
        format!(
            "{} … +{} more",
            items[..max].join(", "),
            items.len() - max
        )
    }
}
