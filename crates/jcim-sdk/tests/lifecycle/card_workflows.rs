use std::sync::Arc;

use jcim_app::{JcimApp, MockPhysicalCardAdapter};
use jcim_config::project::ManagedPaths;
use jcim_sdk::{
    Aid, CardConnectionKind, CardConnectionLocator, CardConnectionTarget, CardInstallSource,
    JcimClient, ProjectRef, globalplatform, iso7816,
};

use super::support::{lifecycle_lock, satochip_support, socket_support, temp_root};

#[tokio::test]
async fn sdk_builds_and_installs_source_project_with_mock_card() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
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
        Arc::new(MockPhysicalCardAdapter::new()),
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
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported("sdk_installs_direct_cap_with_mock_card") {
        return;
    }

    let root = temp_root("sdk-mock-direct-cap");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths_and_card_adapter(
        managed_paths.clone(),
        Arc::new(MockPhysicalCardAdapter::new()),
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
