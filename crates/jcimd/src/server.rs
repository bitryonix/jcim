use std::path::{Path, PathBuf};

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

use jcim_api::v0_3::build_service_server::BuildServiceServer;
use jcim_api::v0_3::card_service_server::CardServiceServer;
use jcim_api::v0_3::project_service_server::ProjectServiceServer;
use jcim_api::v0_3::simulator_service_server::SimulatorServiceServer;
use jcim_api::v0_3::system_service_server::SystemServiceServer;
use jcim_api::v0_3::workspace_service_server::WorkspaceServiceServer;
use jcim_app::JcimApp;
use jcim_config::project::{
    ServiceRuntimeRecord, current_runtime_record_format_version,
    remove_owned_runtime_file_if_present, remove_owned_socket_if_present,
    runtime_metadata_path_for_socket,
};
use jcim_core::error::JcimError;

use crate::rpc::LocalRpc;

/// Serve the local JCIM gRPC API over one Unix-domain socket.
pub async fn serve_local_service(app: JcimApp, socket_path: &Path) -> Result<(), JcimError> {
    serve_local_service_until_shutdown(app, socket_path, std::future::pending()).await
}

/// Serve the local JCIM gRPC API until one shutdown signal resolves.
pub async fn serve_local_service_until_shutdown<F>(
    app: JcimApp,
    socket_path: &Path,
    shutdown: F,
) -> Result<(), JcimError>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let runtime_metadata_path = runtime_metadata_path(&app, socket_path);
    if let Some(parent) = runtime_metadata_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    prepare_socket_for_bind(socket_path)?;
    let runtime_owner_dir = runtime_owner_dir(socket_path, &runtime_metadata_path);
    remove_owned_runtime_file_if_present(&runtime_metadata_path, &runtime_owner_dir)?;

    let listener = UnixListener::bind(socket_path)?;
    let _runtime_guard =
        ServiceRuntimeGuard::install(&app, socket_path, &runtime_metadata_path, runtime_owner_dir)?;
    let rpc = LocalRpc { app };
    Server::builder()
        .add_service(WorkspaceServiceServer::new(rpc.clone()))
        .add_service(ProjectServiceServer::new(rpc.clone()))
        .add_service(BuildServiceServer::new(rpc.clone()))
        .add_service(SimulatorServiceServer::new(rpc.clone()))
        .add_service(CardServiceServer::new(rpc.clone()))
        .add_service(SystemServiceServer::new(rpc))
        .serve_with_incoming_shutdown(UnixListenerStream::new(listener), shutdown)
        .await
        .map_err(|error| JcimError::Unsupported(format!("gRPC server failed: {error}")))
}

struct ServiceRuntimeGuard {
    socket_path: PathBuf,
    runtime_metadata_path: PathBuf,
    runtime_owner_dir: PathBuf,
}

impl ServiceRuntimeGuard {
    fn install(
        app: &JcimApp,
        socket_path: &Path,
        runtime_metadata_path: &Path,
        runtime_owner_dir: PathBuf,
    ) -> Result<Self, JcimError> {
        let status = app.service_status()?;
        let guard = Self {
            socket_path: socket_path.to_path_buf(),
            runtime_metadata_path: runtime_metadata_path.to_path_buf(),
            runtime_owner_dir,
        };
        ServiceRuntimeRecord {
            format_version: current_runtime_record_format_version(),
            pid: std::process::id(),
            socket_path: socket_path.to_path_buf(),
            service_binary_path: status.service_binary_path,
            service_binary_fingerprint: status.service_binary_fingerprint,
        }
        .write_to_path(runtime_metadata_path)?;
        Ok(guard)
    }
}

impl Drop for ServiceRuntimeGuard {
    fn drop(&mut self) {
        let _ = remove_owned_socket_if_present(&self.socket_path, &self.runtime_owner_dir);
        let _ = remove_owned_runtime_file_if_present(
            &self.runtime_metadata_path,
            &self.runtime_owner_dir,
        );
    }
}

fn runtime_metadata_path(app: &JcimApp, socket_path: &Path) -> PathBuf {
    if socket_path == app.managed_paths().service_socket_path {
        app.managed_paths().runtime_metadata_path.clone()
    } else {
        runtime_metadata_path_for_socket(socket_path)
    }
}

fn runtime_owner_dir(socket_path: &Path, runtime_metadata_path: &Path) -> PathBuf {
    socket_path
        .parent()
        .or_else(|| runtime_metadata_path.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn prepare_socket_for_bind(socket_path: &Path) -> Result<(), JcimError> {
    let Some(owner_dir) = socket_path.parent() else {
        return Err(JcimError::Unsupported(format!(
            "managed service socket path has no parent directory: {}",
            socket_path.display()
        )));
    };

    if !socket_path.exists() {
        return Ok(());
    }

    match std::os::unix::net::UnixStream::connect(socket_path) {
        Ok(_) => Err(JcimError::Unsupported(format!(
            "refusing to replace a live local service socket at {}",
            socket_path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => remove_owned_socket_if_present(socket_path, owner_dir),
    }
}
