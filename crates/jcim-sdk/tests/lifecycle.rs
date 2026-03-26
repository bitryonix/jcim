//! End-to-end SDK coverage for build, simulator, and card workflows.

#![forbid(unsafe_code)]

#[path = "../examples/support/satochip.rs"]
mod satochip_support;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_app::{JcimApp, MockPhysicalCardAdapter};
use jcim_config::project::ManagedPaths;
use jcim_sdk::{
    Aid, CardConnectionKind, CardConnectionLocator, CardConnectionTarget, CardInstallSource,
    JcimClient, ProjectRef, SimulationInput, globalplatform, iso7816,
};
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn sdk_builds_and_installs_source_project_with_mock_card() {
    let root = temp_root("sdk-mock-install");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths_and_card_adapter(
        managed_paths.clone(),
        std::sync::Arc::new(MockPhysicalCardAdapter::new()),
    )
    .expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

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
    let root = temp_root("sdk-mock-direct-cap");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths_and_card_adapter(
        managed_paths.clone(),
        std::sync::Arc::new(MockPhysicalCardAdapter::new()),
    )
    .expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

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
    let root = temp_root("sdk-project-sim");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let connection = client
        .open_card_connection(CardConnectionTarget::StartSimulation(
            SimulationInput::Project(ProjectRef::from_path(
                satochip_support::satochip_project_root(),
            )),
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
    let root = temp_root("sdk-attach-sim");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let simulation = client
        .start_simulation(SimulationInput::Project(ProjectRef::from_path(
            satochip_support::satochip_project_root(),
        )))
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
    let root = temp_root("sdk-satochip-wallet");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let connection = client
        .open_card_connection(CardConnectionTarget::StartSimulation(
            SimulationInput::Project(ProjectRef::from_path(
                satochip_support::satochip_project_root(),
            )),
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

async fn wait_for_socket(socket_path: &Path) {
    for _ in 0..40 {
        if socket_path.exists() {
            return;
        }
        sleep(Duration::from_millis(25)).await;
    }
    panic!("socket never appeared at {}", socket_path.display());
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-sdk-{label}-{unique}"))
}
