pub fn run() {
    println!(
        r#"# cargo-tog production cache plan (cross-OS)
#
# SHARE ACROSS OS
#   • CARGO_HOME registry + git downloads (same crate bytes everywhere)
#
# SHARE ONLY WITHIN A TARGET TRIPLE (not across OS)
#   • Compiler objects via CARGO_TOG_BUCKET
#     e.g. x86_64-unknown-linux-gnu  ≠  aarch64-apple-darwin  ≠  x86_64-pc-windows-msvc
#   • One bucket is OK — the engine partitions by compile identity / triple
#
# DO NOT SHARE
#   • target/ across workspaces or across OS
#   • full target/ in GitHub Actions cache
#
# CI MATRIX KEYS (required for registry cache correctness)
#   key: test-${{ runner.os }}-${{ runner.arch }}-${{ matrix.target }}
#   run: cargo-tog host-key   # prints this host's fragments
#
# CI DEFAULTS
#   CARGO_INCREMENTAL=0  CARGO_PROFILE_DEV_DEBUG=0  cache-targets=false
#   RUSTC_WRAPPER=cargo-tog-rustc   # real binary on all OSes
#   secrets: CARGO_TOG_BUCKET, CARGO_TOG_ENDPOINT, CARGO_TOG_REGION,
#            CARGO_TOG_ACCESS_KEY_ID, CARGO_TOG_SECRET_ACCESS_KEY
#
# LOCAL (all OSes)
#   cargo install --path .     # installs cargo-tog + cargo-tog-rustc
#   set RUSTC_WRAPPER=cargo-tog-rustc
#   # cache dir defaults:
#   #   macOS:   ~/Library/Caches/cargo-tog
#   #   Linux:   $XDG_CACHE_HOME/cargo-tog or ~/.cache/cargo-tog
#   #   Windows: %LOCALAPPDATA%\\cargo-tog
#
# NOT REQUIRED FOR CACHE
#   source sync (docs/SYNC.md — advanced only)
#
# SEE ALSO
#   docs/CROSS_OS.md  docs/PRODUCTION.md  docs/RESEARCH.md
"#
    );
}
