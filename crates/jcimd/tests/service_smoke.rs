//! Integration smoke test for the JCIM 0.3 local gRPC service.

#![forbid(unsafe_code)]

#[path = "../../../tests/support/socket.rs"]
mod socket_support;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use jcim_api::v0_3::Empty;
use jcim_api::v0_3::workspace_service_client::WorkspaceServiceClient;
use jcim_app::JcimApp;
use jcim_config::project::ManagedPaths;

#[tokio::test]
async fn service_serves_overview_over_uds() {
    if !socket_support::unix_domain_sockets_supported("service_serves_overview_over_uds") {
        return;
    }

    let root = temp_root("service-overview");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let mut server =
        tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;

    let channel = connect_channel(&managed_paths.service_socket_path)
        .await
        .expect("connect");
    let response = WorkspaceServiceClient::new(channel)
        .get_overview(Empty {})
        .await
        .expect("overview")
        .into_inner();
    let overview = response.overview.expect("overview payload");
    assert_eq!(overview.known_project_count, 0);
    assert_eq!(overview.active_simulation_count, 0);

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
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
