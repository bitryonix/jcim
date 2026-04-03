use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use jcim_app::JcimApp;
use jcim_config::project::{ManagedPaths, ServiceRuntimeRecord, UserConfig};
use jcim_sdk::{JcimClient, ProjectRef};

use super::support::{
    canonical_jcimd_binary, copy_jcimd_binary, lifecycle_lock, repo_root, satochip_support,
    socket_support, stop_managed_daemon, temp_root, wait_for_child_exit, wait_for_path_absent,
};

#[tokio::test]
async fn sdk_connect_or_start_replaces_mismatched_daemon_binary() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
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
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
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
async fn sdk_connect_or_start_recovers_from_stale_runtime_metadata_and_surfaces_backend_start_failures()
 {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    let _guard = lifecycle_lock().lock().await;
    if !socket_support::unix_domain_sockets_supported(
        "sdk_connect_or_start_recovers_from_stale_runtime_metadata_and_surfaces_backend_start_failures",
    ) {
        return;
    }

    let root = temp_root("sdk-stale-runtime-and-backend-failure");
    let managed_paths = ManagedPaths::for_root(root.clone());
    std::fs::create_dir_all(&managed_paths.runtime_dir).expect("create runtime dir");
    ServiceRuntimeRecord {
        format_version: jcim_config::project::current_runtime_record_format_version(),
        pid: 999_999,
        socket_path: managed_paths.service_socket_path.clone(),
        service_binary_path: PathBuf::from("/tmp/stale-jcimd"),
        service_binary_fingerprint: "stale".to_string(),
    }
    .write_to_path(&managed_paths.runtime_metadata_path)
    .expect("write stale runtime metadata");
    UserConfig {
        bundle_root: Some(root.join("missing-bundles")),
        ..UserConfig::default()
    }
    .save_to_path(&managed_paths.config_path)
    .expect("save user config");

    let client = JcimClient::connect_or_start_with_paths(managed_paths.clone())
        .await
        .expect("connect or start");
    let status = client.service_status().await.expect("service status");
    assert!(status.running);
    assert!(managed_paths.runtime_metadata_path.exists());
    let runtime_record =
        ServiceRuntimeRecord::load_if_present(&managed_paths.runtime_metadata_path)
            .expect("load runtime metadata")
            .expect("runtime metadata");
    assert_ne!(runtime_record.pid, 999_999);
    assert_eq!(
        runtime_record.socket_path,
        managed_paths.service_socket_path
    );

    stop_managed_daemon(&managed_paths).await;

    UserConfig {
        bundle_root: Some(root.join("missing-bundles")),
        ..UserConfig::default()
    }
    .save_to_path(&managed_paths.config_path)
    .expect("save failing user config");
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");
    let mut server =
        tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;

    let failing_client = JcimClient::connect_with_paths(managed_paths.clone())
        .await
        .expect("connect to manual daemon");
    let error = failing_client
        .start_simulation(ProjectRef::from_path(
            satochip_support::satochip_project_root(),
        ))
        .await
        .expect_err("invalid bundle root should fail simulation startup");
    let message = error.to_string();
    assert!(
        message.contains("simulation `sim-"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("backend bundle manifest not found"),
        "unexpected error: {message}"
    );

    let simulations = failing_client
        .list_simulations()
        .await
        .expect("list simulations");
    assert_eq!(simulations.len(), 1);
    assert_eq!(simulations[0].status, jcim_sdk::SimulationStatus::Failed);

    let status_after_failure = failing_client
        .service_status()
        .await
        .expect("service status");
    assert!(status_after_failure.running);

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn sdk_connect_or_start_fails_closed_when_a_regular_file_blocks_the_socket() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
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
