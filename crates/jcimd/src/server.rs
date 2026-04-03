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

/// Drop guard that owns the runtime-record and socket cleanup for one server instance.
struct ServiceRuntimeGuard {
    /// Bound Unix-domain socket path owned by this server instance.
    socket_path: PathBuf,
    /// Runtime metadata file written for this server instance.
    runtime_metadata_path: PathBuf,
    /// Directory used to validate ownership before cleanup removes files.
    runtime_owner_dir: PathBuf,
}

impl ServiceRuntimeGuard {
    /// Write the runtime record for one running server and return the cleanup guard.
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

/// Resolve the runtime metadata path for the selected socket, reusing the managed path when possible.
fn runtime_metadata_path(app: &JcimApp, socket_path: &Path) -> PathBuf {
    if socket_path == app.managed_paths().service_socket_path {
        app.managed_paths().runtime_metadata_path.clone()
    } else {
        runtime_metadata_path_for_socket(socket_path)
    }
}

/// Pick the directory used to prove socket and runtime-record ownership during cleanup.
fn runtime_owner_dir(socket_path: &Path, runtime_metadata_path: &Path) -> PathBuf {
    owner_parent(socket_path)
        .or_else(|| owner_parent(runtime_metadata_path))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Return a non-empty parent directory for a path when one exists.
fn owner_parent(path: &Path) -> Option<PathBuf> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
}

/// Refuse to replace a live socket and otherwise clear stale owned sockets before bind.
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

#[cfg(test)]
mod tests {
    use std::io;
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use jcim_app::JcimApp;
    use jcim_config::project::{ManagedPaths, runtime_metadata_path_for_socket};

    use super::{prepare_socket_for_bind, runtime_metadata_path, runtime_owner_dir};

    #[test]
    fn runtime_metadata_path_uses_managed_metadata_for_managed_socket_and_sidecar_for_custom_one() {
        let root = temp_root("metadata-path");
        let managed_paths = ManagedPaths::for_root(root.join("managed"));
        let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

        assert_eq!(
            runtime_metadata_path(&app, &managed_paths.service_socket_path),
            managed_paths.runtime_metadata_path
        );

        let custom_socket = root.join("custom").join("jcimd.sock");
        assert_eq!(
            runtime_metadata_path(&app, &custom_socket),
            runtime_metadata_path_for_socket(&custom_socket)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_owner_dir_prefers_socket_parent_then_runtime_parent() {
        let socket_parent = runtime_owner_dir(
            Path::new("/tmp/jcim/socket/jcimd.sock"),
            Path::new("/tmp/jcim/runtime/daemon.toml"),
        );
        assert_eq!(socket_parent, PathBuf::from("/tmp/jcim/socket"));

        let runtime_parent = runtime_owner_dir(
            Path::new("jcimd.sock"),
            Path::new("/tmp/runtime/daemon.toml"),
        );
        assert_eq!(runtime_parent, PathBuf::from("/tmp/runtime"));
    }

    #[test]
    fn prepare_socket_for_bind_refuses_replacing_live_sockets() {
        if !unix_domain_sockets_supported("prepare_socket_for_bind_refuses_replacing_live_sockets")
        {
            return;
        }

        let root = temp_root("live-socket");
        std::fs::create_dir_all(&root).expect("create root");
        let socket_path = root.join("jcimd.sock");
        let _listener = UnixListener::bind(&socket_path).expect("bind listener");

        let error = prepare_socket_for_bind(&socket_path).expect_err("live socket should fail");
        assert!(
            error
                .to_string()
                .contains("refusing to replace a live local service socket")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from("/tmp").join(format!("jcimd-server-{label}-{unique:x}"))
    }

    fn unix_domain_sockets_supported(test_name: &str) -> bool {
        let probe_path = temp_root(test_name).join("probe.sock");
        std::fs::create_dir_all(probe_path.parent().expect("probe parent"))
            .expect("create probe root");
        match UnixListener::bind(&probe_path) {
            Ok(listener) => {
                drop(listener);
                let _ = std::fs::remove_file(&probe_path);
                true
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!(
                    "skipping {test_name}: this environment denies Unix-domain socket listeners ({error})"
                );
                false
            }
            Err(error) => panic!(
                "failed to probe Unix-domain socket support for {test_name} at {}: {error}",
                probe_path.display()
            ),
        }
    }
}
