use jcim_app::JcimApp;
use jcim_config::project::ManagedPaths;
use jcim_sdk::{
    Aid, CardConnectionKind, CardConnectionLocator, CardConnectionTarget, JcimClient, ProjectRef,
};

use super::support::{
    lifecycle_lock, satochip_support, socket_support, stop_managed_daemon, temp_root,
};

#[tokio::test]
async fn sdk_owned_project_simulation_connection_round_trip() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
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
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
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
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
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

#[tokio::test]
async fn sdk_two_clients_can_drive_one_simulation_concurrently() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
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
