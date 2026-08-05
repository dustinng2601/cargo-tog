use cargo_tog::mode::CacheMode;

pub fn run() {
    let mode = CacheMode::resolve();
    println!(
        r#"# cargo-tog cache plan
#
# RESOLVED MODE: {mode} — {desc}
# Outside dependencies: {deps}
#
# ── Modes (bucket is optional) ─────────────────────────────────────────
#   github         Quick CI. No bucket. Uses GitHub's cache service only.
#   local          Disk only (CARGO_TOG_CACHE_DIR). No cloud.
#   registry-only  crates.io/git downloads only. No object engine.
#   remote         Multi-repo objects via CARGO_TOG_BUCKET (upgrade path).
#   off            Plain Cargo.
#   auto           bucket? → remote; else GITHUB_ACTIONS? → github; else local
#
#   export CARGO_TOG_MODE=github     # or pass Action input mode: github
#   docs/MODES.md
#
# ── What to share ──────────────────────────────────────────────────────
#   YES  registry downloads (all OS)
#   YES  compiler objects within one target triple (mode=github|local|remote)
#   NO   objects across linux/darwin/windows
#   NO   target/ across workspaces
#
# ── Quick start (no bucket) ────────────────────────────────────────────
#   CI:     uses: org/cargo-tog/action@main
#           with: {{ mode: github, key: test-${{{{ runner.os }}}}-${{{{ runner.arch }}}} }}
#   Laptop: export RUSTC_WRAPPER=cargo-tog-rustc
#           # mode defaults to local
#
# ── When you later want multi-repo ─────────────────────────────────────
#   Set CARGO_TOG_BUCKET + keys (or mode: remote). Not required on day one.
#
# SEE docs/MODES.md  docs/CROSS_OS.md  docs/PRODUCTION.md
"#,
        mode = mode.as_str(),
        desc = mode.description(),
        deps = mode.outside_dependencies(),
    );
}
