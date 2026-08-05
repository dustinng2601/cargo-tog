//! End-to-end cover for the `RUSTC_WRAPPER` contract.
//!
//! Cargo invokes a wrapper as `wrapper <rustc> <args…>`, so the leading
//! argument is the compiler to execute — not an argument to hand it. Getting
//! that wrong makes rustc treat its own path as an input filename and breaks
//! every build. The repo's CI always installs the object engine, so this
//! engine-less fallback — what any machine without sccache takes — was never
//! exercised there.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const WRAPPER: &str = env!("CARGO_BIN_EXE_cargo-tog-rustc");

struct Fixture {
    dir: PathBuf,
    rustc: PathBuf,
    argv_log: PathBuf,
}

impl Fixture {
    /// A stand-in compiler that records the argv it was handed.
    fn new(label: &str, exit_code: u8) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cargo-tog-wrapper-{label}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();

        let rustc = dir.join("fake-rustc");
        let argv_log = dir.join("argv.txt");
        fs::write(
            &rustc,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\nexit {exit_code}\n",
                argv_log.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&rustc, fs::Permissions::from_mode(0o755)).unwrap();

        Self {
            dir,
            rustc,
            argv_log,
        }
    }

    /// Run the wrapper in `mode` with an isolated PATH, so no sccache that
    /// happens to be installed on the developer's machine can alter the result.
    fn run(&self, mode: &str, args: &[&str]) -> std::process::ExitStatus {
        Command::new(WRAPPER)
            .args(args)
            .env("CARGO_TOG_MODE", mode)
            .env("CARGO_TOG_CACHE_DIR", self.dir.join("objects"))
            .env("PATH", &self.dir)
            .status()
            .unwrap()
    }

    fn recorded_argv(&self) -> Vec<String> {
        fs::read_to_string(&self.argv_log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn rustc_arg(fx: &Fixture) -> String {
    fx.rustc.display().to_string()
}

#[test]
fn compiler_path_is_executed_not_forwarded_as_an_input_file() {
    let fx = Fixture::new("forward", 0);
    let status = fx.run(
        "off",
        &[&rustc_arg(&fx), "--crate-name", "demo", "src/lib.rs"],
    );

    assert!(status.success(), "wrapper failed: {status:?}");
    assert_eq!(
        fx.recorded_argv(),
        vec!["--crate-name", "demo", "src/lib.rs"],
        "the compiler path must be executed, never passed along as an argument"
    );
}

#[test]
fn engine_free_modes_still_compile() {
    // `registry-only` and `off` deliberately bypass the object engine; they
    // must still run the real compiler rather than skipping the build.
    for mode in ["off", "registry-only"] {
        let fx = Fixture::new(mode, 0);
        let status = fx.run(mode, &[&rustc_arg(&fx), "--version"]);
        assert!(status.success(), "mode={mode} failed: {status:?}");
        assert_eq!(fx.recorded_argv(), vec!["--version"], "mode={mode}");
    }
}

#[test]
fn falls_back_to_the_compiler_when_no_engine_is_on_path() {
    // mode=local wants the engine, but none is installed on the isolated PATH.
    let fx = Fixture::new("fallback", 0);
    let status = fx.run("local", &[&rustc_arg(&fx), "--crate-name", "demo"]);

    assert!(status.success(), "wrapper failed: {status:?}");
    assert_eq!(fx.recorded_argv(), vec!["--crate-name", "demo"]);
}

#[test]
fn compiler_exit_code_is_propagated() {
    // Cargo relies on the wrapper's status to report compile errors.
    let fx = Fixture::new("exit", 3);
    let status = fx.run("off", &[&rustc_arg(&fx), "--crate-name", "demo"]);

    assert_eq!(
        status.code(),
        Some(3),
        "a failing compile must not report success"
    );
}

#[test]
fn a_missing_compiler_reports_failure() {
    let fx = Fixture::new("missing", 0);
    let missing = fx.dir.join("does-not-exist");
    let status = fx.run("off", &[&missing.display().to_string()]);

    assert!(!status.success());
    assert_eq!(status.code(), Some(127));
}

#[test]
fn local_mode_does_not_export_bucket_credentials() {
    // A stale CARGO_TOG_BUCKET in the environment must not turn disk-only
    // caching into authenticated network uploads.
    let fx = Fixture::new("nobucket", 0);
    let dump = fx.dir.join("env.txt");
    fs::write(
        &fx.rustc,
        format!(
            "#!/bin/sh\n/usr/bin/env > \"{}\"\nprintf '%s\\n' \"$@\" > \"{}\"\n",
            dump.display(),
            fx.argv_log.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&fx.rustc, fs::Permissions::from_mode(0o755)).unwrap();

    let status = Command::new(WRAPPER)
        .arg(rustc_arg(&fx))
        .env("CARGO_TOG_MODE", "local")
        .env("CARGO_TOG_BUCKET", "left-over-bucket")
        .env("CARGO_TOG_ACCESS_KEY_ID", "AKIAEXAMPLE")
        .env("CARGO_TOG_SECRET_ACCESS_KEY", "secret")
        .env("CARGO_TOG_CACHE_DIR", fx.dir.join("objects"))
        .env("PATH", &fx.dir)
        .status()
        .unwrap();
    assert!(status.success());

    let env_seen = fs::read_to_string(&dump).unwrap();
    assert!(
        !env_seen.contains("SCCACHE_BUCKET"),
        "local mode leaked a bucket into the engine environment:\n{env_seen}"
    );
    assert!(
        !env_seen.contains("AWS_ACCESS_KEY_ID"),
        "local mode leaked credentials into the engine environment"
    );
    assert!(
        has_line(&env_seen, "SCCACHE_GHA_ENABLED=false"),
        "local mode must explicitly disable the GitHub object backend:\n{env_seen}"
    );
}

#[test]
fn github_mode_enables_the_github_object_backend() {
    let fx = Fixture::new("gha", 0);
    let dump = fx.dir.join("env.txt");
    fs::write(
        &fx.rustc,
        format!("#!/bin/sh\n/usr/bin/env > \"{}\"\n", dump.display()),
    )
    .unwrap();
    fs::set_permissions(&fx.rustc, fs::Permissions::from_mode(0o755)).unwrap();

    let status = Command::new(WRAPPER)
        .arg(rustc_arg(&fx))
        .env("CARGO_TOG_MODE", "github")
        .env("CARGO_TOG_CACHE_DIR", fx.dir.join("objects"))
        .env("PATH", &fx.dir)
        .status()
        .unwrap();
    assert!(status.success());

    let env_seen = fs::read_to_string(&dump).unwrap();
    assert!(
        has_line(&env_seen, "SCCACHE_GHA_ENABLED=true"),
        "github mode must enable the GitHub object backend:\n{env_seen}"
    );
}

fn has_line(env_dump: &str, expected: &str) -> bool {
    env_dump.lines().any(|l| l == expected)
}

#[test]
fn object_cache_directory_is_created() {
    let fx = Fixture::new("mkdir", 0);
    let objects = fx.dir.join("objects");
    assert!(!objects.exists());

    fx.run("local", &[&rustc_arg(&fx)]);

    assert!(
        Path::new(&objects).is_dir(),
        "the engine's object directory should be created up front"
    );
}
