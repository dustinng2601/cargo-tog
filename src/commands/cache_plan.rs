pub fn run() {
    println!(
        r#"# cargo-tog production cache plan
#
# SHARE
#   • CARGO_HOME registry + git downloads
#   • Compiler objects (CARGO_TOG_BUCKET for multi-repo remote)
#
# DO NOT SHARE
#   • target/ across unrelated workspaces
#   • full target/ in GitHub Actions cache
#
# CI DEFAULTS
#   CARGO_INCREMENTAL=0  CARGO_PROFILE_DEV_DEBUG=0  cache-targets=false
#   RUSTC_WRAPPER=cargo-tog-rustc
#   secrets: CARGO_TOG_BUCKET, CARGO_TOG_ENDPOINT, CARGO_TOG_REGION,
#            CARGO_TOG_ACCESS_KEY_ID, CARGO_TOG_SECRET_ACCESS_KEY
#
# LOCAL
#   export RUSTC_WRAPPER=cargo-tog-rustc
#   export CARGO_TOG_CACHE_DIR=$HOME/.cache/cargo-tog
#
# NOT REQUIRED FOR CACHE
#   source sync / partial mirrors (docs/SYNC.md — advanced only)
#
# SEE ALSO
#   docs/PRODUCTION.md  docs/RESEARCH.md  docs/LAYERS.md
"#
    );
}
