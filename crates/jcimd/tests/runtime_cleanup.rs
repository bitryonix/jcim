//! Restart and runtime-cleanup coverage for the local JCIM daemon.

#![forbid(unsafe_code)]

#[path = "../../../tests/support/socket.rs"]
mod socket_support;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use jcim_api::v0_3::simulator_service_client::SimulatorServiceClient;
use jcim_api::v0_3::{ProjectSelector, StartSimulationRequest};
use jcim_app::JcimApp;
use jcim_config::project::{ManagedPaths, UserConfig};

#[tokio::test]
async fn stale_socket_is_replaced_and_runtime_files_are_cleaned_on_shutdown() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "stale_socket_is_replaced_and_runtime_files_are_cleaned_on_shutdown",
    ) {
        return;
    }

    let root = temp_root("stale-socket");
    let managed_paths = ManagedPaths::for_root(root.clone());
    std::fs::create_dir_all(&managed_paths.runtime_dir).expect("create runtime dir");
    let stale_listener = std::os::unix::net::UnixListener::bind(&managed_paths.service_socket_path)
        .expect("bind stale socket");
    drop(stale_listener);
    assert!(managed_paths.service_socket_path.exists());

    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");
    let socket_path = managed_paths.service_socket_path.clone();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let mut server = tokio::spawn(async move {
        jcimd::serve_local_service_until_shutdown(app, &socket_path, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });

    wait_for_runtime_metadata_or_server_exit(&managed_paths.runtime_metadata_path, &mut server)
        .await;
    assert!(managed_paths.service_socket_path.exists());

    let _ = shutdown_tx.send(());
    server.await.expect("server task").expect("server result");

    assert!(!managed_paths.service_socket_path.exists());
    assert!(!managed_paths.runtime_metadata_path.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn regular_files_at_the_socket_path_fail_closed() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    let root = temp_root("regular-file");
    let managed_paths = ManagedPaths::for_root(root.clone());
    std::fs::create_dir_all(&managed_paths.runtime_dir).expect("create runtime dir");
    std::fs::write(&managed_paths.service_socket_path, "not a socket").expect("write regular file");
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let error = jcimd::serve_local_service(app, &managed_paths.service_socket_path)
        .await
        .expect_err("regular file should block daemon startup");
    assert!(error.to_string().contains("refusing to remove non-socket"));

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn symlinked_socket_paths_fail_closed() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    let root = temp_root("symlink-socket");
    let managed_paths = ManagedPaths::for_root(root.clone());
    std::fs::create_dir_all(&managed_paths.runtime_dir).expect("create runtime dir");
    let target = root.join("other-file");
    std::fs::write(&target, "target").expect("write target");
    std::os::unix::fs::symlink(&target, &managed_paths.service_socket_path)
        .expect("create symlink");
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let error = jcimd::serve_local_service(app, &managed_paths.service_socket_path)
        .await
        .expect_err("symlink should block daemon startup");
    assert!(error.to_string().contains("refusing to remove symlinked"));

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn graceful_shutdown_removes_socket_and_runtime_metadata() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "graceful_shutdown_removes_socket_and_runtime_metadata",
    ) {
        return;
    }

    let root = temp_root("graceful-shutdown");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");
    let socket_path = managed_paths.service_socket_path.clone();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let mut server = tokio::spawn(async move {
        jcimd::serve_local_service_until_shutdown(app, &socket_path, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });

    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;
    assert!(managed_paths.runtime_metadata_path.exists());

    let _ = shutdown_tx.send(());
    server.await.expect("server task").expect("server result");

    assert!(!managed_paths.service_socket_path.exists());
    assert!(!managed_paths.runtime_metadata_path.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn stale_runtime_metadata_is_replaced_before_restart() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "stale_runtime_metadata_is_replaced_before_restart",
    ) {
        return;
    }

    let root = temp_root("stale-runtime-metadata");
    let managed_paths = ManagedPaths::for_root(root.clone());
    std::fs::create_dir_all(&managed_paths.runtime_dir).expect("create runtime dir");
    jcim_config::project::ServiceRuntimeRecord {
        format_version: jcim_config::project::current_runtime_record_format_version(),
        pid: 999_999,
        socket_path: managed_paths.service_socket_path.clone(),
        service_binary_path: PathBuf::from("/tmp/stale-jcimd"),
        service_binary_fingerprint: "stale".to_string(),
    }
    .write_to_path(&managed_paths.runtime_metadata_path)
    .expect("write stale runtime metadata");

    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");
    let socket_path = managed_paths.service_socket_path.clone();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let mut server = tokio::spawn(async move {
        jcimd::serve_local_service_until_shutdown(app, &socket_path, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });

    let runtime_record = wait_for_replaced_runtime_metadata(
        &managed_paths.runtime_metadata_path,
        999_999,
        &mut server,
    )
    .await;
    assert_eq!(
        runtime_record.socket_path,
        managed_paths.service_socket_path
    );
    assert_eq!(
        runtime_record.format_version,
        jcim_config::project::current_runtime_record_format_version()
    );
    assert_ne!(runtime_record.service_binary_fingerprint, "stale");

    let _ = shutdown_tx.send(());
    server.await.expect("server task").expect("server result");

    assert!(!managed_paths.service_socket_path.exists());
    assert!(!managed_paths.runtime_metadata_path.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn repeated_graceful_restarts_reuse_the_same_runtime_directory() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "repeated_graceful_restarts_reuse_the_same_runtime_directory",
    ) {
        return;
    }

    let root = temp_root("repeat-restart");
    let managed_paths = ManagedPaths::for_root(root.clone());

    for cycle in 0..3 {
        let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");
        let socket_path = managed_paths.service_socket_path.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let mut server = tokio::spawn(async move {
            jcimd::serve_local_service_until_shutdown(app, &socket_path, async move {
                let _ = shutdown_rx.await;
            })
            .await
        });

        wait_for_runtime_metadata_or_server_exit(&managed_paths.runtime_metadata_path, &mut server)
            .await;
        assert!(
            managed_paths.service_socket_path.exists(),
            "cycle {cycle} should expose the runtime socket"
        );
        assert!(
            managed_paths.runtime_metadata_path.exists(),
            "cycle {cycle} should expose runtime metadata"
        );

        let _ = shutdown_tx.send(());
        server.await.expect("server task").expect("server result");

        assert!(
            !managed_paths.service_socket_path.exists(),
            "cycle {cycle} should clean the runtime socket"
        );
        assert!(
            !managed_paths.runtime_metadata_path.exists(),
            "cycle {cycle} should clean runtime metadata"
        );
    }

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn backend_start_failure_leaves_daemon_recoverable_until_shutdown() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "backend_start_failure_leaves_daemon_recoverable_until_shutdown",
    ) {
        return;
    }

    let root = temp_root("backend-start-failure");
    let managed_paths = ManagedPaths::for_root(root.clone());
    managed_paths
        .prepare_layout()
        .expect("prepare managed layout");
    UserConfig {
        bundle_root: Some(root.join("missing-bundles")),
        ..UserConfig::default()
    }
    .save_to_path(&managed_paths.config_path)
    .expect("save user config");
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");
    let socket_path = managed_paths.service_socket_path.clone();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let mut server = tokio::spawn(async move {
        jcimd::serve_local_service_until_shutdown(app, &socket_path, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });

    wait_for_runtime_metadata_or_server_exit(&managed_paths.runtime_metadata_path, &mut server)
        .await;
    let channel = connect_channel(&managed_paths.service_socket_path)
        .await
        .expect("connect");
    let error = SimulatorServiceClient::new(channel)
        .start_simulation(StartSimulationRequest {
            project: Some(ProjectSelector {
                project_path: satochip_project_root().display().to_string(),
                project_id: String::new(),
            }),
        })
        .await
        .expect_err("invalid bundle root should fail simulation startup");

    assert_eq!(error.code(), tonic::Code::Unavailable);
    assert!(
        error
            .message()
            .contains("backend bundle manifest not found"),
        "unexpected gRPC error: {error}"
    );
    assert!(managed_paths.service_socket_path.exists());
    assert!(managed_paths.runtime_metadata_path.exists());
    assert!(
        !server.is_finished(),
        "daemon should remain alive after managed backend startup failure"
    );

    let _ = shutdown_tx.send(());
    server.await.expect("server task").expect("server result");

    assert!(!managed_paths.service_socket_path.exists());
    assert!(!managed_paths.runtime_metadata_path.exists());
    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcimd-runtime-{label}-{unique:x}"))
}

async fn wait_for_runtime_metadata_or_server_exit(
    runtime_metadata_path: &std::path::Path,
    server: &mut tokio::task::JoinHandle<Result<(), jcim_core::error::JcimError>>,
) {
    for _ in 0..40 {
        if runtime_metadata_path.exists() {
            return;
        }
        if server.is_finished() {
            let result = server.await.expect("server join");
            panic!(
                "runtime metadata never appeared at {} because the server exited early: {}",
                runtime_metadata_path.display(),
                result
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "server exited cleanly".to_string())
            );
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
    }
    panic!(
        "runtime metadata never appeared at {} while the server was still running",
        runtime_metadata_path.display()
    );
}

async fn wait_for_replaced_runtime_metadata(
    runtime_metadata_path: &std::path::Path,
    stale_pid: u32,
    server: &mut tokio::task::JoinHandle<Result<(), jcim_core::error::JcimError>>,
) -> jcim_config::project::ServiceRuntimeRecord {
    for _ in 0..40 {
        if let Some(record) =
            jcim_config::project::ServiceRuntimeRecord::load_if_present(runtime_metadata_path)
                .expect("load runtime metadata")
            && record.pid != stale_pid
        {
            return record;
        }
        if server.is_finished() {
            let result = server.await.expect("server join");
            panic!(
                "runtime metadata at {} never replaced stale pid {} because the server exited early: {}",
                runtime_metadata_path.display(),
                stale_pid,
                result
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "server exited cleanly".to_string())
            );
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
    }
    panic!(
        "runtime metadata at {} never replaced stale pid {} while the server was still running",
        runtime_metadata_path.display(),
        stale_pid
    );
}

async fn connect_channel(
    socket_path: &std::path::Path,
) -> Result<Channel, tonic::transport::Error> {
    let socket_path = socket_path.to_path_buf();
    Endpoint::try_from("http://[::]:50051")
        .expect("endpoint")
        .connect_with_connector(service_fn(move |_| {
            let socket_path = socket_path.clone();
            async move { UnixStream::connect(socket_path).await.map(TokioIo::new) }
        }))
        .await
}

fn satochip_project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/satochip/workdir")
}
