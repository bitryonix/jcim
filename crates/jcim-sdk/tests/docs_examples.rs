//! Smoke coverage for documented SDK example commands.

#![forbid(unsafe_code)]

#[path = "../../../tests/support/socket.rs"]
mod socket_support;

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn satochip_lifecycle_example_runs_from_docs_flow() {
    if !socket_support::unix_domain_sockets_supported(
        "satochip_lifecycle_example_runs_from_docs_flow",
    ) {
        return;
    }

    let root = temp_root("docs-example-lifecycle");
    let output = run_example(&root, "satochip_lifecycle", &[]);
    assert!(output.contains("Started simulation"));
    assert!(output.contains("Simulator SELECT status: 9000"));
}

#[test]
fn satochip_wallet_example_runs_from_docs_flow() {
    if !socket_support::unix_domain_sockets_supported("satochip_wallet_example_runs_from_docs_flow")
    {
        return;
    }

    let root = temp_root("docs-example-wallet");
    let output = run_example(&root, "satochip_wallet", &[]);
    assert!(output.contains("Started virtual Satochip target:"));
    assert!(output.contains("Wallet created with primary PIN"));
}

#[test]
fn hardware_gated_wallet_example_runs_when_enabled() {
    if std::env::var("JCIM_HARDWARE_TESTS").ok().as_deref() != Some("1") {
        return;
    }
    if !socket_support::unix_domain_sockets_supported(
        "hardware_gated_wallet_example_runs_when_enabled",
    ) {
        return;
    }

    let reader = std::env::var("JCIM_TEST_CARD_READER")
        .expect("SDK doc hardware smoke requires JCIM_TEST_CARD_READER when JCIM_HARDWARE_TESTS=1");
    let root = temp_root("docs-example-wallet-hardware");
    let output = run_example(&root, "satochip_wallet", &["--reader", &reader]);
    assert!(output.contains("Installed"));
    assert!(output.contains("Wallet created with primary PIN"));
}

fn run_example(home_root: &std::path::Path, name: &str, args: &[&str]) -> String {
    let output = Command::new(example_binary_path(name))
        .args(args)
        .current_dir(repo_root())
        .env("HOME", home_root)
        .env("XDG_DATA_HOME", home_root.join("xdg"))
        .env("NO_COLOR", "1")
        .env_remove("JCIM_SIMULATOR_CONTAINER_CMD")
        .output()
        .expect("run example");
    if !output.status.success() {
        panic!(
            "example {name} {:?} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn example_binary_path(name: &str) -> PathBuf {
    let current = std::env::current_exe().expect("current exe");
    let debug_dir = current
        .parent()
        .and_then(|path| path.parent())
        .expect("target debug dir");
    let candidate = debug_dir
        .join("examples")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(
        candidate.exists(),
        "example binary not found at {}",
        candidate.display()
    );
    candidate
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jsdk-{label}-{unique:x}"))
}
