//! Integration coverage for simulator-oriented gRPC flows.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tokio::time::{Duration, sleep};
use tonic::Code;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use jcim_api::v0_2::build_service_client::BuildServiceClient;
use jcim_api::v0_2::simulator_service_client::SimulatorServiceClient;
use jcim_api::v0_2::start_simulation_request::Input as StartSimulationInput;
use jcim_api::v0_2::{BuildProjectRequest, ProjectSelector, StartSimulationRequest};
use jcim_app::JcimApp;
use jcim_config::project::ManagedPaths;

#[tokio::test]
async fn service_builds_the_source_backed_satochip_example() {
    let root = temp_root("service-build");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

    let channel = connect_channel(&managed_paths.service_socket_path)
        .await
        .expect("connect");
    let response = BuildServiceClient::new(channel)
        .build_project(BuildProjectRequest {
            project: Some(ProjectSelector {
                project_path: satochip_project_root().display().to_string(),
                project_id: String::new(),
            }),
        })
        .await
        .expect("build")
        .into_inner();

    assert_eq!(response.artifacts.len(), 1);
    assert_eq!(response.artifacts[0].kind, "cap");
    assert!(Path::new(&response.artifacts[0].path).exists());

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn project_backed_simulation_requires_container_command_on_macos() {
    let root = temp_root("project-sim");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

    let channel = connect_channel(&managed_paths.service_socket_path)
        .await
        .expect("connect");
    let error = SimulatorServiceClient::new(channel)
        .start_simulation(StartSimulationRequest {
            input: Some(StartSimulationInput::Project(ProjectSelector {
                project_path: satochip_project_root().display().to_string(),
                project_id: String::new(),
            })),
        })
        .await
        .expect_err("missing container command should fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert!(error.message().contains("JCIM_SIMULATOR_CONTAINER_CMD"));

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn direct_cap_simulation_requires_container_command_on_macos() {
    let root = temp_root("cap-sim");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let server = tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    wait_for_socket(&managed_paths.service_socket_path).await;

    let channel = connect_channel(&managed_paths.service_socket_path)
        .await
        .expect("connect");
    let build_response = BuildServiceClient::new(channel.clone())
        .build_project(BuildProjectRequest {
            project: Some(ProjectSelector {
                project_path: satochip_project_root().display().to_string(),
                project_id: String::new(),
            }),
        })
        .await
        .expect("build")
        .into_inner();
    let cap_path = build_response
        .artifacts
        .first()
        .expect("cap artifact")
        .path
        .clone();

    let error = SimulatorServiceClient::new(channel)
        .start_simulation(StartSimulationRequest {
            input: Some(StartSimulationInput::CapPath(cap_path)),
        })
        .await
        .expect_err("missing container command should fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert!(error.message().contains("JCIM_SIMULATOR_CONTAINER_CMD"));

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

async fn connect_channel(socket_path: &Path) -> Result<Channel, tonic::transport::Error> {
    let socket_path = socket_path.to_path_buf();
    Endpoint::try_from("http://[::]:50051")
        .expect("endpoint")
        .connect_with_connector(service_fn(move |_| {
            let socket_path = socket_path.clone();
            async move { UnixStream::connect(socket_path).await.map(TokioIo::new) }
        }))
        .await
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jd-{label}-{unique}"))
}
