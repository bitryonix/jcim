//! Smoke coverage for README and docs command flows.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn cli_bin() -> &'static str {
    env!("CARGO_BIN_EXE_jcim-cli")
}

#[test]
fn quickstart_commands_from_docs_run_from_repo_root() {
    let root = temp_root("docs-quickstart");
    let demo = root.join("demo");
    let demo_arg = demo_string(&demo);

    let created = run_cli(&root, &["project", "new", "demo", "--directory", &demo_arg]);
    assert!(created.contains("Project: demo"));

    let setup = run_cli(&root, &["system", "setup"]);
    assert!(setup.contains("saved machine-local JCIM settings"));

    let doctor = run_cli(&root, &["system", "doctor"]);
    assert!(doctor.contains("Effective Java runtime:"));

    let build = run_cli(&root, &["build", "--project", &demo_arg]);
    assert!(build.contains("Artifacts:"));

    let artifacts = run_cli(&root, &["build", "artifacts", "--project", &demo_arg]);
    assert!(artifacts.contains("cap:"));

    let started = run_cli(&root, &["sim", "start", "--project", &demo_arg]);
    assert!(started.contains("Status: running"));

    let status = run_cli(&root, &["sim", "status"]);
    assert!(status.contains("Source: project"));

    let select = run_cli(&root, &["sim", "iso", "select", "--aid", "F00000000101"]);
    assert_eq!(select.trim(), "9000");

    let apdu = run_cli(&root, &["sim", "apdu", "00A4040006F0000000010100"]);
    assert_eq!(apdu.trim(), "9000");

    let reset = run_cli(&root, &["sim", "reset"]);
    assert!(!reset.trim().is_empty());

    let readers = run_cli(&root, &["card", "readers"]);
    assert!(
        readers.contains("Reader:")
            || readers.contains("No PC/SC readers found.")
            || readers.trim() == "[]"
    );

    let stopped = run_cli(&root, &["sim", "stop"]);
    assert!(stopped.contains("Status: stopped"));

    let service = run_cli(&root, &["system", "service", "status"]);
    assert!(service.contains("Running: yes"));
}

#[test]
fn satochip_cli_commands_from_docs_run_from_repo_root() {
    let root = temp_root("docs-satochip");
    let project = repo_root().join("examples/satochip/workdir");
    let project_arg = demo_string(&project);

    let build = run_cli(&root, &["build", "--project", &project_arg]);
    assert!(build.contains("Project: Satochip Example"));

    let started = run_cli(&root, &["sim", "start", "--project", &project_arg]);
    assert!(started.contains("Status: running"));

    let status = run_cli(&root, &["sim", "status"]);
    assert!(status.contains("Source: project"));
    assert!(status.contains("Project Path:"));

    let select = run_cli(
        &root,
        &["sim", "iso", "select", "--aid", "5361746F4368697000"],
    );
    assert_eq!(select.trim(), "9000");

    let apdu = run_cli(&root, &["sim", "apdu", "B03C000000"]);
    assert!(apdu.trim().ends_with("9000"));

    let reset = run_cli(&root, &["sim", "reset"]);
    assert!(!reset.trim().is_empty());

    let stopped = run_cli(&root, &["sim", "stop"]);
    assert!(stopped.contains("Status: stopped"));
}

#[test]
fn hardware_gated_doc_commands_run_when_enabled() {
    if std::env::var("JCIM_HARDWARE_TESTS").ok().as_deref() != Some("1") {
        return;
    }

    let reader = std::env::var("JCIM_TEST_CARD_READER")
        .expect("CLI doc hardware smoke requires JCIM_TEST_CARD_READER when JCIM_HARDWARE_TESTS=1");
    let root = temp_root("docs-hardware");
    let demo = root.join("demo");
    let demo_arg = demo_string(&demo);

    let _created = run_cli(&root, &["project", "new", "demo", "--directory", &demo_arg]);
    let _build = run_cli(&root, &["build", "--project", &demo_arg]);
    let readers = run_cli(&root, &["card", "readers"]);
    assert!(readers.contains(&reader));

    let status = run_cli(&root, &["card", "status", "--reader", &reader]);
    assert!(status.contains("Reader:") || status.contains("reader_name"));

    let install = run_cli(
        &root,
        &[
            "card",
            "install",
            "--project",
            &demo_arg,
            "--reader",
            &reader,
        ],
    );
    assert!(install.contains("package") || install.contains("Package:"));
}

fn run_cli(home_root: &Path, args: &[&str]) -> String {
    let output = Command::new(cli_bin())
        .args(args)
        .current_dir(repo_root())
        .env("HOME", home_root)
        .env("XDG_DATA_HOME", home_root.join("xdg"))
        .env("NO_COLOR", "1")
        .env_remove("JCIM_SIMULATOR_CONTAINER_CMD")
        .output()
        .expect("run jcim-cli");
    if !output.status.success() {
        panic!(
            "jcim-cli {:?} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn demo_string(path: &Path) -> String {
    path.display().to_string()
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcli-{label}-{unique:x}"))
}
