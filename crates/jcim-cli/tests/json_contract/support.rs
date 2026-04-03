#[path = "../../../../tests/support/integration_runtime.rs"]
mod integration_runtime;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};

use jcim_app::{JcimApp, MockPhysicalCardAdapter};
use jcim_config::project::ManagedPaths;
use serde_json::Value;

pub(crate) use integration_runtime::socket_support;

fn cli_bin() -> &'static str {
    env!("CARGO_BIN_EXE_jcim-cli")
}

pub(crate) fn run_cli(home_root: &Path, args: &[&str]) -> std::process::Output {
    let output = command(home_root, args).output().expect("run jcim-cli");
    if !output.status.success() {
        panic!(
            "jcim-cli {:?} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    output
}

pub(crate) fn run_cli_failure(home_root: &Path, args: &[&str]) -> std::process::Output {
    let output = command(home_root, args).output().expect("run jcim-cli");
    assert!(
        !output.status.success(),
        "jcim-cli {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

pub(crate) fn run_cli_on_mock_service(home_root: &Path, args: &[&str]) -> std::process::Output {
    let output = mock_service_command(home_root, args)
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
    output
}

fn command(home_root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(cli_bin());
    command
        .args(args)
        .current_dir(repo_root())
        .env("HOME", home_root)
        .env("XDG_CONFIG_HOME", home_root.join("xdg-config"))
        .env("XDG_DATA_HOME", home_root.join("xdg-data"))
        .env("XDG_STATE_HOME", home_root.join("xdg-state"))
        .env("XDG_CACHE_HOME", home_root.join("xdg-cache"))
        .env("XDG_RUNTIME_DIR", home_root.join("xdg-runtime"))
        .env("JCIM_GP_DEFAULT_KEYSET", "mock")
        .env("JCIM_GP_MOCK_MODE", "scp03")
        .env("JCIM_GP_MOCK_ENC", "404142434445464748494A4B4C4D4E4F")
        .env("JCIM_GP_MOCK_MAC", "505152535455565758595A5B5C5D5E5F")
        .env("JCIM_GP_MOCK_DEK", "606162636465666768696A6B6C6D6E6F")
        .env("JCIM_SERVICE_BIN", fresh_jcimd_binary())
        .env("NO_COLOR", "1")
        .env_remove("JCIM_SIMULATOR_CONTAINER_CMD");
    command
}

fn mock_service_command(home_root: &Path, args: &[&str]) -> Command {
    let mut command = command(home_root, args);
    command.env("JCIM_SERVICE_BIN", current_test_binary());
    command
}

pub(crate) fn parse_json(label: &str, bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or_else(|error| {
        panic!(
            "failed to parse {label} JSON: {error}\nraw:\n{}",
            String::from_utf8_lossy(bytes)
        )
    })
}

pub(crate) fn assert_kind(json: &Value, kind: &str) {
    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], kind);
}

pub(crate) fn repo_root() -> PathBuf {
    integration_runtime::repo_root()
}

fn current_test_binary() -> PathBuf {
    std::env::current_exe().expect("current json_contract test binary")
}

fn fresh_jcimd_binary() -> PathBuf {
    static JCIMD_BIN: OnceLock<PathBuf> = OnceLock::new();
    JCIMD_BIN
        .get_or_init(|| {
            let status = Command::new("cargo")
                .args(["build", "-p", "jcimd", "--bin", "jcimd"])
                .current_dir(repo_root())
                .status()
                .expect("build jcimd binary for CLI integration tests");
            assert!(status.success(), "building jcimd binary failed");
            integration_runtime::canonical_jcimd_binary()
        })
        .clone()
}

pub(crate) fn path_arg(path: &Path) -> String {
    path.display().to_string()
}

pub(crate) fn temp_root(label: &str) -> PathBuf {
    integration_runtime::temp_root("jcim-cli-json", label)
}

pub(crate) async fn spawn_mock_service(
    home_root: &Path,
) -> (
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<(), jcim_core::error::JcimError>>,
) {
    let managed_paths = managed_paths_for_test_home(home_root);
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths_and_card_adapter(
        managed_paths.clone(),
        Arc::new(MockPhysicalCardAdapter::new()),
    )
    .expect("load app");
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let mut server = tokio::spawn(async move {
        jcimd::serve_local_service_until_shutdown(app, &socket_path, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });
    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;
    (shutdown_tx, server)
}

fn managed_paths_for_test_home(home_root: &Path) -> ManagedPaths {
    ManagedPaths::for_env_roots(
        home_root.to_path_buf(),
        Some(home_root.join("xdg-config")),
        Some(home_root.join("xdg-data")),
        Some(home_root.join("xdg-state")),
        Some(home_root.join("xdg-cache")),
        Some(home_root.join("xdg-runtime")),
    )
}
