//! Piping into a short reader must not abort the CLI.
//!
//! Rust ignores `SIGPIPE`, so a closed pipe surfaced as an `EPIPE` write error
//! and `println!` panicked: `cargo-tog inventory | head` died with a Rust
//! backtrace and exit 101. Under `bash -eo pipefail` — how GitHub Actions runs
//! `shell: bash`, and how this repo's own CI pipes `cache-plan` into `head` —
//! that failed the step.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const CLI: &str = env!("CARGO_BIN_EXE_cargo-tog");

/// A tree whose `inventory` output comfortably exceeds a 64 KiB pipe buffer,
/// so the write really does outlive the reader. Long names get there with far
/// fewer files than short ones would.
fn wide_tree() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("cargo-tog-pipe-{stamp}"));
    let _ = fs::remove_dir_all(&root);
    for i in 0..400 {
        let name = format!(
            "crate-with-a-deliberately-long-name-{i:04}-{}",
            "x".repeat(160)
        );
        let dir = root.join(&name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
        )
        .unwrap();
    }
    root
}

#[test]
fn a_reader_that_stops_early_does_not_abort_the_cli() {
    let root = wide_tree();

    let mut child = Command::new(CLI)
        .arg("inventory")
        .arg("--root")
        .arg(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cargo-tog");

    // Read a little, then drop the pipe — this is what `| head -5` does.
    {
        let mut out = child.stdout.take().expect("stdout");
        let mut head = [0u8; 256];
        let _ = out.read(&mut head);
    }

    let mut stderr = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut stderr);
    }
    let status = child.wait().expect("wait");

    assert!(
        !stderr.contains("panicked"),
        "CLI panicked when its reader went away:\n{stderr}"
    );
    assert!(
        !stderr.contains("Broken pipe"),
        "CLI reported a broken pipe instead of exiting quietly:\n{stderr}"
    );
    // 101 is the Rust panic exit code; on unix the process now dies from
    // SIGPIPE (no code) or finishes normally if it outran the reader.
    assert_ne!(
        status.code(),
        Some(101),
        "CLI aborted with a panic exit code: {status:?}\n{stderr}"
    );

    let _ = fs::remove_dir_all(&root);
}
