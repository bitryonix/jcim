//! End-to-end SDK coverage for build, simulator, and card workflows.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_app::{JcimApp, MockPhysicalCardAdapter};
use jcim_config::project::ManagedPaths;
use jcim_sdk::{Aid, CardInstallSource, JcimClient, ProjectRef, SimulationInput, globalplatform};
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
    let project = ProjectRef::from_path(satochip_project_root());
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
    let response = client
        .iso_select_application_on_card(&applet_aid)
        .await
        .expect("select applet");
    assert_eq!(response.sw, 0x9000);

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

    let reset = client.reset_card_summary().await.expect("reset");
    assert!(reset.atr.is_some());
    assert_eq!(reset.session_state.open_channels.len(), 1);
    assert_eq!(reset.session_state.open_channels[0].channel_number, 0);
    assert!(!reset.session_state.secure_messaging.active);

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
    let project = ProjectRef::from_path(satochip_project_root());
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

#[cfg(target_os = "macos")]
#[tokio::test]
async fn sdk_reports_missing_container_command_for_project_simulation_on_macos() {
    if std::env::var_os("JCIM_SIMULATOR_CONTAINER_CMD").is_some() {
        return;
    }

    let root = temp_root("sdk-project-sim-macos");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let error = client
        .start_simulation(SimulationInput::Project(ProjectRef::from_path(
            satochip_project_root(),
        )))
        .await
        .expect_err("missing container command should fail");
    assert!(error.to_string().contains("JCIM_SIMULATOR_CONTAINER_CMD"));

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(target_os = "macos"))]
#[tokio::test]
async fn sdk_project_simulation_round_trip_when_supported() {
    let root = temp_root("sdk-project-sim");
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
            satochip_project_root(),
        )))
        .await
        .expect("start simulation");
    let select = jcim_sdk::CommandApdu::parse(
        &hex::decode("00A40400095361746F4368697000").expect("decode select"),
    )
    .expect("parse select");
    let response = client
        .transmit_sim_apdu(simulation.simulation_ref(), &select)
        .await
        .expect("select applet");
    assert_eq!(response.sw, 0x9000);
    let _ = client
        .reset_simulation(simulation.simulation_ref())
        .await
        .expect("reset simulation");
    let stopped = client
        .stop_simulation(simulation.simulation_ref())
        .await
        .expect("stop simulation");
    assert_eq!(stopped.status, jcim_sdk::SimulationStatus::Stopped);

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(target_os = "macos"))]
#[tokio::test]
async fn sdk_direct_cap_simulation_round_trip_when_supported() {
    let root = temp_root("sdk-cap-sim");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

    let client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect");
    let project = ProjectRef::from_path(satochip_project_root());
    let build = client.build_project(&project).await.expect("build");
    let simulation = client
        .start_simulation(SimulationInput::Cap(build.artifacts[0].path.clone()))
        .await
        .expect("start simulation");
    let select = jcim_sdk::CommandApdu::parse(
        &hex::decode("00A40400095361746F4368697000").expect("decode select"),
    )
    .expect("parse select");
    let response = client
        .transmit_sim_apdu(simulation.simulation_ref(), &select)
        .await
        .expect("select applet");
    assert_eq!(response.sw, 0x9000);

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

fn satochip_project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/satochip/workdir")
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
