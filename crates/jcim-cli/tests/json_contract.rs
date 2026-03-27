//! Stable `--json` contract coverage for representative CLI flows.

#![forbid(unsafe_code)]

#[path = "../../../tests/support/socket.rs"]
mod socket_support;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_app::{JcimApp, MockPhysicalCardAdapter};
use jcim_config::project::ManagedPaths;
use serde_json::Value;

fn cli_bin() -> &'static str {
    env!("CARGO_BIN_EXE_jcim-cli")
}

#[test]
fn system_service_status_json_is_versioned_even_when_service_is_not_running() {
    let root = temp_root("json-service-status");
    let output = run_cli(&root, &["--json", "system", "service", "status"]);
    let json = parse_json("service status", &output.stdout);

    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], "system.service_status");
    assert_eq!(json["running"], false);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_new_json_includes_version_and_kind_markers() {
    if !socket_support::unix_domain_sockets_supported(
        "project_new_json_includes_version_and_kind_markers",
    ) {
        return;
    }

    let root = temp_root("json-project-new");
    let project_dir = root.join("demo");
    let output = run_cli(
        &root,
        &[
            "--json",
            "project",
            "new",
            "demo",
            "--directory",
            &path_arg(&project_dir),
        ],
    );
    let json = parse_json("project new", &output.stdout);
    let expected_project_path = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.clone());

    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], "project.summary");
    assert_eq!(json["name"], "demo");
    assert_eq!(
        json["project_path"],
        expected_project_path.display().to_string()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn simulation_status_json_uses_the_list_kind() {
    if !socket_support::unix_domain_sockets_supported("simulation_status_json_uses_the_list_kind") {
        return;
    }

    let root = temp_root("json-sim-status");
    let output = run_cli(&root, &["--json", "sim", "status"]);
    let json = parse_json("sim status", &output.stdout);

    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], "simulation.list");
    assert!(json["simulations"].is_array());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn json_failures_go_to_stderr_with_the_error_envelope() {
    if !socket_support::unix_domain_sockets_supported(
        "json_failures_go_to_stderr_with_the_error_envelope",
    ) {
        return;
    }

    let root = temp_root("json-error");
    let missing_project = root.join("missing-project");
    let output = run_cli_failure(
        &root,
        &["--json", "build", "--project", &path_arg(&missing_project)],
    );
    let json = parse_json("json error", &output.stderr);

    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], "error");
    assert!(!json["message"].as_str().expect("error message").is_empty());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_build_system_and_simulation_json_commands_cover_the_managed_surface() {
    if !socket_support::unix_domain_sockets_supported(
        "project_build_system_and_simulation_json_commands_cover_the_managed_surface",
    ) {
        return;
    }

    let root = temp_root("json-sim-suite");
    let project_dir = root.join("demo");
    let project_arg = path_arg(&project_dir);
    let applet_aid = "F00000000101";

    let project_new = parse_json(
        "project new",
        &run_cli(
            &root,
            &[
                "--json",
                "project",
                "new",
                "demo",
                "--directory",
                &project_arg,
            ],
        )
        .stdout,
    );
    assert_kind(&project_new, "project.summary");

    let project_show = parse_json(
        "project show",
        &run_cli(
            &root,
            &["--json", "project", "show", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&project_show, "project.details");

    let build = parse_json(
        "build",
        &run_cli(&root, &["--json", "build", "--project", &project_arg]).stdout,
    );
    assert_kind(&build, "build.summary");
    assert!(
        build["artifacts"]
            .as_array()
            .expect("build artifacts")
            .iter()
            .any(|artifact| artifact["kind"] == "cap")
    );

    let artifacts = parse_json(
        "build artifacts",
        &run_cli(
            &root,
            &["--json", "build", "artifacts", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&artifacts, "build.summary");

    let project_clean = parse_json(
        "project clean",
        &run_cli(
            &root,
            &["--json", "project", "clean", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&project_clean, "project.clean");

    let system_setup = parse_json(
        "system setup",
        &run_cli(&root, &["--json", "system", "setup"]).stdout,
    );
    assert_kind(&system_setup, "system.setup");

    let system_doctor = parse_json(
        "system doctor",
        &run_cli(&root, &["--json", "system", "doctor"]).stdout,
    );
    assert_kind(&system_doctor, "system.doctor");

    let service_status = parse_json(
        "system service status",
        &run_cli(&root, &["--json", "system", "service", "status"]).stdout,
    );
    assert_kind(&service_status, "system.service_status");
    assert_eq!(service_status["running"], true);

    let sim_start = parse_json(
        "sim start",
        &run_cli(
            &root,
            &["--json", "sim", "start", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&sim_start, "simulation.summary");

    let sim_status = parse_json(
        "sim status",
        &run_cli(&root, &["--json", "sim", "status"]).stdout,
    );
    assert_kind(&sim_status, "simulation.list");

    let sim_logs = parse_json(
        "sim logs",
        &run_cli(&root, &["--json", "sim", "logs"]).stdout,
    );
    assert_kind(&sim_logs, "simulation.events");

    let sim_apdu = parse_json(
        "sim apdu",
        &run_cli(
            &root,
            &["--json", "sim", "apdu", "00A4040006F0000000010100"],
        )
        .stdout,
    );
    assert_kind(&sim_apdu, "apdu.response");
    assert_eq!(sim_apdu["status_word"], "9000");

    let sim_reset = parse_json(
        "sim reset",
        &run_cli(&root, &["--json", "sim", "reset"]).stdout,
    );
    assert_kind(&sim_reset, "simulation.reset");

    let sim_iso_status = parse_json(
        "sim iso status",
        &run_cli(&root, &["--json", "sim", "iso", "status"]).stdout,
    );
    assert_kind(&sim_iso_status, "session.iso");

    let sim_iso_select = parse_json(
        "sim iso select",
        &run_cli(
            &root,
            &["--json", "sim", "iso", "select", "--aid", applet_aid],
        )
        .stdout,
    );
    assert_kind(&sim_iso_select, "apdu.response");
    assert_eq!(sim_iso_select["status_word"], "9000");

    let sim_channel_open = parse_json(
        "sim iso channel-open",
        &run_cli(&root, &["--json", "sim", "iso", "channel-open"]).stdout,
    );
    assert_kind(&sim_channel_open, "channel.summary");
    let opened_channel = sim_channel_open["channel_number"]
        .as_u64()
        .expect("opened channel number")
        .to_string();

    let sim_channel_close = parse_json(
        "sim iso channel-close",
        &run_cli(
            &root,
            &[
                "--json",
                "sim",
                "iso",
                "channel-close",
                "--channel",
                &opened_channel,
            ],
        )
        .stdout,
    );
    assert_kind(&sim_channel_close, "channel.summary");

    let sim_secure_open = parse_json(
        "sim iso secure-open",
        &run_cli(
            &root,
            &[
                "--json",
                "sim",
                "iso",
                "secure-open",
                "--protocol",
                "scp03",
                "--security-level",
                "3",
                "--session-id",
                "sim-session",
            ],
        )
        .stdout,
    );
    assert_kind(&sim_secure_open, "secure_messaging.summary");

    let sim_secure_advance = parse_json(
        "sim iso secure-advance",
        &run_cli(
            &root,
            &["--json", "sim", "iso", "secure-advance", "--increment", "2"],
        )
        .stdout,
    );
    assert_kind(&sim_secure_advance, "secure_messaging.summary");

    let sim_secure_close = parse_json(
        "sim iso secure-close",
        &run_cli(&root, &["--json", "sim", "iso", "secure-close"]).stdout,
    );
    assert_kind(&sim_secure_close, "secure_messaging.summary");

    let sim_stop = parse_json(
        "sim stop",
        &run_cli(&root, &["--json", "sim", "stop"]).stdout,
    );
    assert_kind(&sim_stop, "simulation.summary");

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn card_json_commands_cover_the_mock_reader_surface() {
    if !socket_support::unix_domain_sockets_supported(
        "card_json_commands_cover_the_mock_reader_surface",
    ) {
        return;
    }

    let root = temp_root("json-card-suite");
    let (shutdown_tx, server) = spawn_mock_service(&root).await;
    let project_dir = repo_root().join("examples/satochip/workdir");
    let project_arg = path_arg(&project_dir);
    let applet_aid = "5361746F4368697000";

    let readers = parse_json(
        "card readers",
        &run_cli_on_mock_service(&root, &["--json", "card", "readers"]).stdout,
    );
    assert_kind(&readers, "card.readers");
    assert_eq!(readers["readers"][0]["name"], "Mock Reader 0");

    let card_status = parse_json(
        "card status",
        &run_cli_on_mock_service(&root, &["--json", "card", "status"]).stdout,
    );
    assert_kind(&card_status, "card.status");
    assert_eq!(card_status["reader_name"], "Mock Reader 0");

    let install = parse_json(
        "card install",
        &run_cli_on_mock_service(
            &root,
            &["--json", "card", "install", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&install, "card.install");
    let package_aid = install["package_aid"]
        .as_str()
        .expect("package aid")
        .to_string();

    let packages = parse_json(
        "card packages",
        &run_cli_on_mock_service(&root, &["--json", "card", "packages"]).stdout,
    );
    assert_kind(&packages, "card.packages");

    let applets = parse_json(
        "card applets",
        &run_cli_on_mock_service(&root, &["--json", "card", "applets"]).stdout,
    );
    assert_kind(&applets, "card.applets");

    let card_apdu = parse_json(
        "card apdu",
        &run_cli_on_mock_service(
            &root,
            &["--json", "card", "apdu", "00A40400095361746F4368697000"],
        )
        .stdout,
    );
    assert_kind(&card_apdu, "apdu.response");
    assert_eq!(card_apdu["status_word"], "9000");

    let card_reset = parse_json(
        "card reset",
        &run_cli_on_mock_service(&root, &["--json", "card", "reset"]).stdout,
    );
    assert_kind(&card_reset, "card.reset");

    let card_iso_status = parse_json(
        "card iso status",
        &run_cli_on_mock_service(&root, &["--json", "card", "iso", "status"]).stdout,
    );
    assert_kind(&card_iso_status, "session.iso");

    let card_iso_select = parse_json(
        "card iso select",
        &run_cli_on_mock_service(
            &root,
            &["--json", "card", "iso", "select", "--aid", applet_aid],
        )
        .stdout,
    );
    assert_kind(&card_iso_select, "apdu.response");
    assert_eq!(card_iso_select["status_word"], "9000");

    let card_channel_open = parse_json(
        "card iso channel-open",
        &run_cli_on_mock_service(&root, &["--json", "card", "iso", "channel-open"]).stdout,
    );
    assert_kind(&card_channel_open, "channel.summary");
    let opened_channel = card_channel_open["channel_number"]
        .as_u64()
        .expect("opened card channel")
        .to_string();

    let card_channel_close = parse_json(
        "card iso channel-close",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "iso",
                "channel-close",
                "--channel",
                &opened_channel,
            ],
        )
        .stdout,
    );
    assert_kind(&card_channel_close, "channel.summary");

    let card_secure_open = parse_json(
        "card iso secure-open",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "iso",
                "secure-open",
                "--protocol",
                "scp03",
                "--security-level",
                "3",
                "--session-id",
                "card-session",
            ],
        )
        .stdout,
    );
    assert_kind(&card_secure_open, "secure_messaging.summary");

    let card_secure_advance = parse_json(
        "card iso secure-advance",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "iso",
                "secure-advance",
                "--increment",
                "2",
            ],
        )
        .stdout,
    );
    assert_kind(&card_secure_advance, "secure_messaging.summary");

    let card_secure_close = parse_json(
        "card iso secure-close",
        &run_cli_on_mock_service(&root, &["--json", "card", "iso", "secure-close"]).stdout,
    );
    assert_kind(&card_secure_close, "secure_messaging.summary");

    let card_gp_select = parse_json(
        "card gp select-isd",
        &run_cli_on_mock_service(&root, &["--json", "card", "gp", "select-isd"]).stdout,
    );
    assert_kind(&card_gp_select, "apdu.response");
    assert_eq!(card_gp_select["status_word"], "9000");

    let card_gp_status_apps = parse_json(
        "card gp get-status applications",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "get-status",
                "--kind",
                "applications",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_status_apps, "gp.status");

    let card_gp_status_isd = parse_json(
        "card gp get-status isd",
        &run_cli_on_mock_service(
            &root,
            &["--json", "card", "gp", "get-status", "--kind", "isd"],
        )
        .stdout,
    );
    assert_kind(&card_gp_status_isd, "gp.status");
    let security_domain_aid = card_gp_status_isd["entries"]
        .as_array()
        .and_then(|entries| entries.first())
        .and_then(|entry| entry["aid"].as_str())
        .expect("card security domain aid")
        .to_string();

    let card_gp_set_locked = parse_json(
        "card gp set-card-status locked",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-card-status",
                "--state",
                "card-locked",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_set_locked, "apdu.response");

    let card_gp_set_secured = parse_json(
        "card gp set-card-status secured",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-card-status",
                "--state",
                "secured",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_set_secured, "apdu.response");

    let card_gp_lock_app = parse_json(
        "card gp set-application-status lock",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-application-status",
                "--aid",
                applet_aid,
                "--transition",
                "lock",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_lock_app, "apdu.response");

    let card_gp_unlock_app = parse_json(
        "card gp set-application-status unlock",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-application-status",
                "--aid",
                applet_aid,
                "--transition",
                "unlock",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_unlock_app, "apdu.response");

    let card_gp_lock_sd = parse_json(
        "card gp set-security-domain-status",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-security-domain-status",
                "--aid",
                &security_domain_aid,
                "--transition",
                "lock",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_lock_sd, "apdu.response");

    let delete = parse_json(
        "card delete",
        &run_cli_on_mock_service(&root, &["--json", "card", "delete", &package_aid]).stdout,
    );
    assert_kind(&delete, "card.delete");
    assert_eq!(delete["deleted"], true);

    let _ = shutdown_tx.send(());
    server.await.expect("server task").expect("server result");
    let _ = std::fs::remove_dir_all(root);
}

fn run_cli(home_root: &Path, args: &[&str]) -> std::process::Output {
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

fn run_cli_failure(home_root: &Path, args: &[&str]) -> std::process::Output {
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

fn run_cli_on_mock_service(home_root: &Path, args: &[&str]) -> std::process::Output {
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

fn parse_json(label: &str, bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or_else(|error| {
        panic!(
            "failed to parse {label} JSON: {error}\nraw:\n{}",
            String::from_utf8_lossy(bytes)
        )
    })
}

fn assert_kind(json: &Value, kind: &str) {
    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], kind);
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
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
            repo_root().join("target/debug/jcimd")
        })
        .clone()
}

fn path_arg(path: &Path) -> String {
    path.display().to_string()
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-cli-json-{label}-{unique:x}"))
}

async fn spawn_mock_service(
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
