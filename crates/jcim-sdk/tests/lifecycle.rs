//! End-to-end SDK coverage for build, simulator, and card workflows.

#![forbid(unsafe_code)]

#[path = "../examples/support/satochip.rs"]
mod satochip_support;
#[path = "../../../tests/support/socket.rs"]
mod socket_support;

use std::fs::File;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_app::{JcimApp, MockPhysicalCardAdapter};
use jcim_config::project::{ManagedPaths, ServiceRuntimeRecord};
use jcim_sdk::{
    Aid, CardConnectionKind, CardConnectionLocator, CardConnectionTarget, CardInstallSource,
    JcimClient, ProjectRef, globalplatform, iso7816,
};

#[tokio::test]
async fn sdk_builds_and_installs_source_project_with_mock_card() {
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported(
        "sdk_builds_and_installs_source_project_with_mock_card",
    ) {
        return;
    }

    let root = temp_root("sdk-mock-install");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths_and_card_adapter(
        managed_paths.clone(),
        std::sync::Arc::new(MockPhysicalCardAdapter::new()),
    )
    .expect("load app");

    let mut server =
        tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let project = ProjectRef::from_path(satochip_support::satochip_project_root());
    let build = client.build_project(&project).await.expect("build");
    assert_eq!(build.artifacts.len(), 1);
    assert_eq!(build.artifacts[0].kind, "cap");
    assert!(build.artifacts[0].path.exists());

    let install = client
        .install_cap(CardInstallSource::Project(project.clone()))
        .await
        .expect("install");
    assert_eq!(install.package_name, "org.satochip.applet");
    assert!(!install.applets.is_empty());

    let packages = client.list_packages().await.expect("packages");
    assert!(
        packages
            .packages
            .iter()
            .any(|package| package.aid == install.package_aid)
    );

    let applets = client.list_applets().await.expect("applets");
    assert!(
        applets
            .applets
            .iter()
            .any(|applet| applet.aid == install.applets[0].aid)
    );

    let applet_aid = Aid::from_hex(&install.applets[0].aid).expect("aid");
    let connection = client
        .open_card_connection(CardConnectionTarget::Reader(jcim_sdk::ReaderRef::Default))
        .await
        .expect("open card connection");
    assert_eq!(connection.kind(), CardConnectionKind::Reader);
    assert_eq!(
        connection.locator(),
        &CardConnectionLocator::Reader {
            reader_name: "Mock Reader 0".to_string(),
        }
    );

    let response = connection
        .transmit(&iso7816::select_by_name(&applet_aid))
        .await
        .expect("select applet");
    assert_eq!(response.sw, 0x9000);

    let raw = connection
        .transmit_raw(&iso7816::select_by_name(&applet_aid).to_bytes())
        .await
        .expect("raw select applet");
    assert_eq!(raw.response.sw, 0x9000);
    assert_eq!(raw.session_state.selected_aid, Some(applet_aid.clone()));

    let session_state = connection.session_state().await.expect("session state");
    assert_eq!(session_state.selected_aid, Some(applet_aid.clone()));

    let isd = client
        .gp_select_issuer_security_domain_on_card()
        .await
        .expect("select isd");
    assert_eq!(isd.sw, 0x9000);

    let status = client
        .gp_get_status_on_card(
            globalplatform::RegistryKind::Applications,
            globalplatform::GetStatusOccurrence::FirstOrAll,
        )
        .await
        .expect("get status");
    assert!(
        status
            .entries
            .iter()
            .any(|entry| entry.aid == applet_aid && entry.life_cycle_state == 0x07)
    );

    let lock = client
        .gp_set_application_status_on_card(&applet_aid, globalplatform::LockTransition::Lock)
        .await
        .expect("lock applet");
    assert!(lock.is_success());

    let locked_status = client
        .gp_get_status_on_card(
            globalplatform::RegistryKind::Applications,
            globalplatform::GetStatusOccurrence::FirstOrAll,
        )
        .await
        .expect("get status after lock");
    assert!(
        locked_status
            .entries
            .iter()
            .any(|entry| entry.aid == applet_aid && entry.life_cycle_state == 0x83)
    );

    let locked_select = client
        .iso_select_application_on_card(&applet_aid)
        .await
        .expect("select locked applet");
    assert_eq!(
        locked_select.sw,
        jcim_sdk::iso7816::StatusWord::WARNING_SELECTED_FILE_INVALIDATED.as_u16()
    );

    let unlock = client
        .gp_set_application_status_on_card(&applet_aid, globalplatform::LockTransition::Unlock)
        .await
        .expect("unlock applet");
    assert!(unlock.is_success());

    let unlocked_select = client
        .iso_select_application_on_card(&applet_aid)
        .await
        .expect("select unlocked applet");
    assert_eq!(unlocked_select.sw, 0x9000);

    let card_lock = client
        .gp_set_card_status_on_card(globalplatform::CardLifeCycle::CardLocked)
        .await
        .expect("lock card");
    assert!(card_lock.is_success());

    let card_locked_select = client
        .iso_select_application_on_card(&applet_aid)
        .await
        .expect("select applet on locked card");
    assert_eq!(
        card_locked_select.sw,
        jcim_sdk::iso7816::StatusWord::COMMAND_NOT_ALLOWED.as_u16()
    );

    let card_unlock = client
        .gp_set_card_status_on_card(globalplatform::CardLifeCycle::Secured)
        .await
        .expect("unlock card");
    assert!(card_unlock.is_success());

    let channel = client
        .manage_card_channel(true, None)
        .await
        .expect("open logical channel");
    assert_eq!(channel.channel_number, Some(1));
    assert!(
        channel
            .session_state
            .open_channels
            .iter()
            .any(|entry| entry.channel_number == 1)
    );

    let secure = client
        .open_card_secure_messaging(
            Some(jcim_sdk::iso7816::SecureMessagingProtocol::Scp03),
            Some(0x03),
            Some("mock-session".to_string()),
        )
        .await
        .expect("open secure messaging");
    assert!(secure.session_state.secure_messaging.active);

    let reset = connection.reset_summary().await.expect("reset");
    assert!(reset.atr.is_some());
    assert_eq!(reset.session_state.open_channels.len(), 1);
    assert_eq!(reset.session_state.open_channels[0].channel_number, 0);
    assert!(!reset.session_state.secure_messaging.active);
    connection.close().await.expect("close card connection");

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn sdk_installs_direct_cap_with_mock_card() {
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported("sdk_installs_direct_cap_with_mock_card") {
        return;
    }

    let root = temp_root("sdk-mock-direct-cap");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths_and_card_adapter(
        managed_paths.clone(),
        std::sync::Arc::new(MockPhysicalCardAdapter::new()),
    )
    .expect("load app");

    let mut server =
        tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let project = ProjectRef::from_path(satochip_support::satochip_project_root());
    let build = client.build_project(&project).await.expect("build");
    let cap_path = build.artifacts[0].path.clone();

    let install = client
        .install_cap(CardInstallSource::Cap(cap_path.clone()))
        .await
        .expect("install");
    assert_eq!(install.cap_path, cap_path);

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn sdk_owned_project_simulation_connection_round_trip() {
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported(
        "sdk_owned_project_simulation_connection_round_trip",
    ) {
        return;
    }

    let root = temp_root("sdk-project-sim");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let mut server =
        tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let connection = client
        .open_card_connection(CardConnectionTarget::StartSimulation(
            ProjectRef::from_path(satochip_support::satochip_project_root()),
        ))
        .await
        .expect("open simulation connection");
    assert_eq!(connection.kind(), CardConnectionKind::Simulation);
    let locator = connection.locator().clone();
    let select = jcim_sdk::CommandApdu::parse(
        &hex::decode("00A40400095361746F4368697000").expect("decode select"),
    )
    .expect("parse select");
    let response = connection.transmit(&select).await.expect("select applet");
    assert_eq!(response.sw, 0x9000);
    let session_state = connection.session_state().await.expect("session state");
    assert!(session_state.selected_aid.is_some());
    let reset = connection.reset_summary().await.expect("reset simulation");
    assert!(reset.atr.is_some());
    connection.close().await.expect("close owned simulation");

    let CardConnectionLocator::Simulation { simulation, owned } = locator else {
        panic!("expected simulation locator");
    };
    assert!(owned);
    let stopped = client
        .get_simulation(simulation)
        .await
        .expect("stopped simulation");
    assert_eq!(stopped.status, jcim_sdk::SimulationStatus::Stopped);

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn sdk_attach_existing_simulation_connection_leaves_simulation_running() {
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported(
        "sdk_attach_existing_simulation_connection_leaves_simulation_running",
    ) {
        return;
    }

    let root = temp_root("sdk-attach-sim");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let mut server =
        tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let simulation = client
        .start_simulation(ProjectRef::from_path(
            satochip_support::satochip_project_root(),
        ))
        .await
        .expect("start simulation");
    let connection = client
        .open_card_connection(CardConnectionTarget::ExistingSimulation(
            simulation.simulation_ref(),
        ))
        .await
        .expect("attach simulation connection");
    let locator = connection.locator().clone();
    let response = connection
        .transmit(
            &jcim_sdk::CommandApdu::parse(
                &hex::decode("00A40400095361746F4368697000").expect("decode select"),
            )
            .expect("parse select"),
        )
        .await
        .expect("select applet");
    assert_eq!(response.sw, 0x9000);
    connection
        .close()
        .await
        .expect("close attached simulation connection");

    let CardConnectionLocator::Simulation { simulation, owned } = locator else {
        panic!("expected simulation locator");
    };
    assert!(!owned);
    let still_running = client
        .get_simulation(simulation.clone())
        .await
        .expect("simulation still present");
    assert_eq!(still_running.status, jcim_sdk::SimulationStatus::Running);
    let stopped = client
        .stop_simulation(simulation)
        .await
        .expect("stop attached simulation");
    assert_eq!(stopped.status, jcim_sdk::SimulationStatus::Stopped);

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn sdk_runs_the_satochip_wallet_demo_on_a_project_backed_simulation() {
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported(
        "sdk_runs_the_satochip_wallet_demo_on_a_project_backed_simulation",
    ) {
        return;
    }

    let root = temp_root("sdk-satochip-wallet");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let mut server =
        tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let connection = client
        .open_card_connection(CardConnectionTarget::StartSimulation(
            ProjectRef::from_path(satochip_support::satochip_project_root()),
        ))
        .await
        .expect("open simulation connection");
    let locator = connection.locator().clone();

    let flow = satochip_support::run_wallet_demo(&connection)
        .await
        .expect("run wallet demo");
    assert!(flow.initial_status.needs_secure_channel);
    assert!(!flow.initial_status.setup_done);
    assert!(!flow.initial_status.seeded);
    assert!(flow.post_setup_status.setup_done);
    assert!(!flow.post_setup_status.seeded);
    assert!(flow.post_seed_status.seeded);
    assert_eq!(
        flow.initial_status.protocol_version,
        flow.post_seed_status.protocol_version
    );
    assert!(!flow.authentikey_hex.is_empty());
    assert!(!flow.derived_pubkey_hex.is_empty());
    assert!(!flow.chain_code_hex.is_empty());
    assert_eq!(flow.transaction_hash_hex.len(), 64);
    assert!(!flow.signature_hex.is_empty());

    connection.close().await.expect("close owned simulation");

    let CardConnectionLocator::Simulation { simulation, owned } = locator else {
        panic!("expected simulation locator");
    };
    assert!(owned);
    let stopped = client
        .get_simulation(simulation)
        .await
        .expect("stopped simulation");
    assert_eq!(stopped.status, jcim_sdk::SimulationStatus::Stopped);

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-sdk-{label}-{unique}"))
}

#[tokio::test]
async fn sdk_connect_or_start_replaces_mismatched_daemon_binary() {
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported(
        "sdk_connect_or_start_replaces_mismatched_daemon_binary",
    ) {
        return;
    }

    let root = temp_root("sdk-binary-mismatch");
    let managed_paths = ManagedPaths::for_root(root.join("managed"));
    managed_paths.prepare_layout().expect("prepare layout");

    let copied_binary = copy_jcimd_binary(root.join("jcimd-copied"));
    let stderr_log_path = root.join("jcimd-copied.stderr.log");
    let stderr_log = File::create(&stderr_log_path).expect("create copied stderr log");
    let mut child = Command::new(&copied_binary)
        .current_dir(repo_root())
        .arg("--socket-path")
        .arg(&managed_paths.service_socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .expect("spawn copied jcimd");
    socket_support::wait_for_socket_or_child_exit(
        &managed_paths.service_socket_path,
        &mut child,
        &stderr_log_path,
    );

    let client = JcimClient::connect_or_start_with_paths(managed_paths.clone())
        .await
        .expect("restart through canonical jcimd");
    let status = client.service_status().await.expect("service status");
    assert!(status.running);
    assert_eq!(status.service_binary_path, canonical_jcimd_binary());
    wait_for_child_exit(&mut child).await;

    stop_managed_daemon(&managed_paths).await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn sdk_connect_or_start_survives_repeated_service_restarts() {
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported(
        "sdk_connect_or_start_survives_repeated_service_restarts",
    ) {
        return;
    }

    let root = temp_root("sdk-restart-loops");
    let managed_paths = ManagedPaths::for_root(root.clone());

    for cycle in 0..3 {
        let client = JcimClient::connect_or_start_with_paths(managed_paths.clone())
            .await
            .unwrap_or_else(|error| panic!("connect_or_start cycle {cycle} failed: {error}"));
        let status = client.service_status().await.expect("service status");
        assert!(
            status.running,
            "cycle {cycle} should report a running daemon"
        );
        assert!(managed_paths.runtime_metadata_path.exists());

        stop_managed_daemon(&managed_paths).await;
        wait_for_path_absent(&managed_paths.service_socket_path).await;
        wait_for_path_absent(&managed_paths.runtime_metadata_path).await;
    }

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn sdk_connect_or_start_fails_closed_when_a_regular_file_blocks_the_socket() {
    let _guard = lifecycle_lock().lock().await;
    let root = temp_root("sdk-regular-file");
    let managed_paths = ManagedPaths::for_root(root.clone());
    std::fs::create_dir_all(&managed_paths.runtime_dir).expect("create runtime dir");
    std::fs::write(&managed_paths.service_socket_path, "not a socket").expect("write blocker");

    let error = match JcimClient::connect_or_start_with_paths(managed_paths.clone()).await {
        Ok(_) => panic!("non-socket path should block bootstrap"),
        Err(error) => error,
    };
    let message = error.to_string();
    assert!(
        message.contains("refusing to remove non-socket"),
        "unexpected bootstrap error: {message}"
    );
    assert!(!managed_paths.runtime_metadata_path.exists());

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn sdk_two_clients_can_drive_one_simulation_concurrently() {
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported(
        "sdk_two_clients_can_drive_one_simulation_concurrently",
    ) {
        return;
    }

    let root = temp_root("sdk-two-clients");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let client_a = JcimClient::connect_or_start_with_paths(managed_paths.clone())
        .await
        .expect("connect client A");
    let client_b = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect client B");

    let simulation = client_a
        .start_simulation(ProjectRef::from_path(
            satochip_support::satochip_project_root(),
        ))
        .await
        .expect("start simulation");
    let simulation_ref = simulation.simulation_ref();
    let select_apdu = hex::decode("00A40400095361746F4368697000").expect("decode select APDU");
    let applet_aid = Aid::from_hex("5361746F4368697000").expect("parse applet aid");
    let client_a_select = client_a.clone();
    let client_b_raw = client_b.clone();
    let client_a_status = client_a.clone();
    let client_b_session = client_b.clone();
    let client_a_events = client_a.clone();
    let client_a_service = client_a.clone();
    let client_b_service = client_b.clone();

    let (
        select_result,
        raw_result,
        status_result,
        session_result,
        events_result,
        service_status_a,
        service_status_b,
    ) = tokio::join!(
        client_a_select.iso_select_application_on_simulation(simulation_ref.clone(), &applet_aid),
        client_b_raw.transmit_raw_sim_apdu(simulation_ref.clone(), &select_apdu),
        client_a_status.get_simulation(simulation_ref.clone()),
        client_b_session.get_simulation_session_state(simulation_ref.clone()),
        client_a_events.simulation_events(simulation_ref.clone()),
        client_a_service.service_status(),
        client_b_service.service_status(),
    );

    assert_eq!(select_result.expect("typed select").sw, 0x9000);
    assert_eq!(raw_result.expect("raw select").response.sw, 0x9000);
    assert_eq!(
        status_result.expect("simulation status").status,
        jcim_sdk::SimulationStatus::Running
    );
    let _session_state = session_result.expect("session state");
    assert!(service_status_a.expect("service status A").running);
    assert!(service_status_b.expect("service status B").running);
    let _events = events_result.expect("simulation events");

    let stopped = client_a
        .stop_simulation(simulation_ref)
        .await
        .expect("stop simulation");
    assert_eq!(stopped.status, jcim_sdk::SimulationStatus::Stopped);

    stop_managed_daemon(&managed_paths).await;
    let _ = std::fs::remove_dir_all(root);
}

fn copy_jcimd_binary(destination: PathBuf) -> PathBuf {
    let source = canonical_jcimd_binary();
    match std::fs::hard_link(&source, &destination) {
        Ok(()) => destination,
        Err(_) => {
            std::fs::copy(&source, &destination).expect("copy jcimd binary");
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                let mut permissions = std::fs::metadata(&destination)
                    .expect("copied binary metadata")
                    .permissions();
                permissions.set_mode(0o755);
                std::fs::set_permissions(&destination, permissions)
                    .expect("copied binary permissions");
            }
            destination
        }
    }
}

fn canonical_jcimd_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("JCIM_SERVICE_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path.canonicalize().unwrap_or(path);
        }
    }
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_jcimd") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path.canonicalize().unwrap_or(path);
        }
    }

    let current = std::env::current_exe().expect("current test executable");
    let parent = current.parent().expect("test executable parent");
    let candidates = [
        parent.join("jcimd"),
        parent
            .parent()
            .expect("test executable grandparent")
            .join("jcimd"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return candidate.canonicalize().unwrap_or(candidate);
        }
    }

    panic!("unable to locate jcimd near {}", current.display());
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn lifecycle_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

async fn stop_managed_daemon(managed_paths: &ManagedPaths) {
    let Some(record) = ServiceRuntimeRecord::load_if_present(&managed_paths.runtime_metadata_path)
        .expect("load runtime metadata")
    else {
        return;
    };

    let status = Command::new("kill")
        .arg("-TERM")
        .arg(record.pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("send SIGTERM");
    assert!(
        status.success(),
        "SIGTERM failed for recorded jcimd pid {} with status {}",
        record.pid,
        status
    );

    if !wait_for_pid_absent(record.pid, 80).await {
        let kill_status = Command::new("kill")
            .arg("-KILL")
            .arg(record.pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("send SIGKILL");
        assert!(
            kill_status.success(),
            "SIGKILL failed for recorded jcimd pid {} with status {}",
            record.pid,
            kill_status
        );
        assert!(
            wait_for_pid_absent(record.pid, 40).await,
            "jcimd pid {} did not exit after SIGTERM and SIGKILL",
            record.pid
        );
    }
    wait_for_path_absent(&managed_paths.service_socket_path).await;
    wait_for_path_absent(&managed_paths.runtime_metadata_path).await;
}

async fn wait_for_child_exit(child: &mut std::process::Child) {
    for _ in 0..80 {
        if child.try_wait().expect("poll child").is_some() {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    panic!("copied jcimd child did not exit after the SDK replaced it");
}

async fn wait_for_pid_absent(pid: u32, attempts: usize) -> bool {
    for _ in 0..attempts {
        let output = Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("stat=")
            .stdin(Stdio::null())
            .output()
            .expect("poll pid");
        if !output.status.success() {
            return true;
        }
        let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if state.is_empty() || state.starts_with('Z') {
            return true;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    false
}

async fn wait_for_path_absent(path: &std::path::Path) {
    for _ in 0..80 {
        if !path.exists() {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    panic!("path {} still exists after shutdown", path.display());
}
