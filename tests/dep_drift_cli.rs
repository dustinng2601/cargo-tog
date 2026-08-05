//! `dep-drift` end to end, over trees that use every dependency spelling.
//!
//! The parser used to see only inline entries under `[dependencies]`, so drift
//! hidden in a `[dependencies.<name>]` table or behind a `[target.…]` predicate
//! was reported as clean — the one answer this command must never get wrong.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const CLI: &str = env!("CARGO_BIN_EXE_cargo-tog");

struct Trees {
    root: PathBuf,
}

impl Trees {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cargo-tog-drift-{label}-{stamp}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, rel: &str, contents: &str) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn drift(&self, extra: &[&str]) -> Output {
        Command::new(CLI)
            .arg("dep-drift")
            .arg("--master")
            .arg(self.root.join("master"))
            .arg("--other")
            .arg(self.root.join("other"))
            .args(extra)
            .output()
            .unwrap()
    }
}

impl Drop for Trees {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const WORKSPACE_ROOT: &str = r#"
[workspace]
members = ["app"]

[workspace.dependencies]
tokio = { version = "1.40", features = ["full"] }
"#;

fn member(libc: &str, serde: &str) -> String {
    format!(
        r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
tokio = {{ workspace = true }}

[dependencies.serde]
version = "{serde}"
features = ["derive"]

[target.'cfg(unix)'.dependencies]
libc = "{libc}"
"#
    )
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn identical_trees_are_clean() {
    let t = Trees::new("clean");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    t.write("master/app/Cargo.toml", &member("0.2", "1.0"));
    t.write("other/Cargo.toml", WORKSPACE_ROOT);
    t.write("other/app/Cargo.toml", &member("0.2", "1.0"));

    let out = t.drift(&[]);
    let text = stdout(&out);
    assert!(out.status.success(), "expected clean, got:\n{text}");
    assert!(text.contains("result: clean"), "{text}");
}

#[test]
fn drift_inside_a_target_specific_table_is_detected() {
    let t = Trees::new("target");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    t.write("master/app/Cargo.toml", &member("0.2", "1.0"));
    t.write("other/Cargo.toml", WORKSPACE_ROOT);
    // Only the platform-gated pin differs.
    t.write("other/app/Cargo.toml", &member("0.3", "1.0"));

    let out = t.drift(&[]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "drift must exit 1:\n{text}");
    assert!(text.contains("libc"), "libc drift not reported:\n{text}");
}

#[test]
fn drift_inside_a_dependency_table_is_detected() {
    let t = Trees::new("table");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    t.write("master/app/Cargo.toml", &member("0.2", "1.0"));
    t.write("other/Cargo.toml", WORKSPACE_ROOT);
    // Only the [dependencies.serde] table differs.
    t.write("other/app/Cargo.toml", &member("0.2", "1.9"));

    let out = t.drift(&[]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "drift must exit 1:\n{text}");
    assert!(text.contains("serde"), "serde drift not reported:\n{text}");
}

#[test]
fn a_workspace_pin_difference_is_detected() {
    let t = Trees::new("pin");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    t.write("master/app/Cargo.toml", &member("0.2", "1.0"));
    t.write(
        "other/Cargo.toml",
        r#"
[workspace]
members = ["app"]

[workspace.dependencies]
tokio = { version = "1.38", features = ["full"] }
"#,
    );
    t.write("other/app/Cargo.toml", &member("0.2", "1.0"));

    let out = t.drift(&[]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("tokio"),
        "tokio pin drift not reported:\n{text}"
    );
}

#[test]
fn an_unpinned_workspace_dependency_is_flagged() {
    let t = Trees::new("unpinned");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    t.write("master/app/Cargo.toml", &member("0.2", "1.0"));
    // Split repo kept `tokio.workspace = true` but lost the pin.
    t.write("other/Cargo.toml", "[workspace]\nmembers = [\"app\"]\n");
    t.write("other/app/Cargo.toml", &member("0.2", "1.0"));

    let out = t.drift(&[]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("unresolved workspace deps on other"),
        "unpinned workspace dep not reported:\n{text}"
    );
}

#[test]
fn warn_only_reports_drift_without_failing() {
    let t = Trees::new("warn");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    t.write("master/app/Cargo.toml", &member("0.2", "1.0"));
    t.write("other/Cargo.toml", WORKSPACE_ROOT);
    t.write("other/app/Cargo.toml", &member("0.3", "1.0"));

    let out = t.drift(&["--warn-only"]);
    let text = stdout(&out);
    assert!(out.status.success(), "--warn-only must exit 0:\n{text}");
    assert!(text.contains("libc"), "{text}");
}

#[test]
fn ignored_crates_do_not_cause_failure() {
    let t = Trees::new("ignore");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    t.write("master/app/Cargo.toml", &member("0.2", "1.0"));
    t.write("other/Cargo.toml", WORKSPACE_ROOT);
    t.write("other/app/Cargo.toml", &member("0.3", "1.0"));

    let out = t.drift(&["--ignore", "libc"]);
    let text = stdout(&out);
    assert!(out.status.success(), "ignored drift must exit 0:\n{text}");
    assert!(text.contains("result: clean"), "{text}");
}

#[test]
fn json_report_is_machine_readable() {
    let t = Trees::new("json");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    t.write("master/app/Cargo.toml", &member("0.2", "1.0"));
    t.write("other/Cargo.toml", WORKSPACE_ROOT);
    t.write("other/app/Cargo.toml", &member("0.3", "1.0"));

    let out = t.drift(&["--json"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.trim_start().starts_with('{'), "not JSON:\n{text}");
    assert!(text.contains("\"crate_name\": \"libc\""), "{text}");
    assert!(text.contains("\"exit_worthy\": true"), "{text}");
}

#[test]
fn missing_directories_are_rejected() {
    let t = Trees::new("missing");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    // `other` is never created.

    let out = t.drift(&[]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("must be directories"),
        "unhelpful error: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn inventory_lists_packages_and_pins() {
    let t = Trees::new("inventory");
    t.write("master/Cargo.toml", WORKSPACE_ROOT);
    t.write("master/app/Cargo.toml", &member("0.2", "1.0"));

    let out = Command::new(CLI)
        .arg("inventory")
        .arg("--root")
        .arg(Path::new(&t.root).join("master"))
        .output()
        .unwrap();
    let text = stdout(&out);

    assert!(out.status.success(), "{text}");
    assert!(text.contains("app@0.1.0"), "{text}");
    assert!(text.contains("tokio"), "workspace pin missing:\n{text}");
}
