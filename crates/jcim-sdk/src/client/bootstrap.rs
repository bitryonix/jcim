use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tokio::time::sleep;
use tonic::Code;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use jcim_config::project::{
    ManagedPaths, ServiceRuntimeRecord, remove_owned_runtime_file_if_present,
    remove_owned_socket_if_present,
};
use jcim_core::error::JcimError;

use crate::error::{JcimSdkError, Result};
use crate::types::ServiceStatusSummary;

use super::JcimClient;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::MetadataExt;

impl JcimClient {
    /// Connect to an already-running local JCIM service using the default managed paths.
    pub async fn connect() -> Result<Self> {
        let managed_paths = ManagedPaths::discover()?;
        Self::connect_with_paths(managed_paths).await
    }

    /// Connect to an already-running local JCIM service using explicit managed paths.
    pub async fn connect_with_paths(managed_paths: ManagedPaths) -> Result<Self> {
        let channel = connect_channel(&managed_paths.service_socket_path).await?;
        Ok(Self {
            managed_paths,
            channel,
        })
    }

    /// Connect to the local JCIM service, starting it if needed, using the default managed paths.
    pub async fn connect_or_start() -> Result<Self> {
        let managed_paths = ManagedPaths::discover()?;
        Self::connect_or_start_with_paths(managed_paths).await
    }

    /// Connect to the local JCIM service, starting it if needed, using explicit managed paths.
    pub async fn connect_or_start_with_paths(managed_paths: ManagedPaths) -> Result<Self> {
        if let Ok(channel) = connect_channel(&managed_paths.service_socket_path).await {
            let client = Self {
                managed_paths: managed_paths.clone(),
                channel,
            };
            if client.connected_service_matches_current_binary().await? {
                return Ok(client);
            }
            drop(client);
            prepare_service_restart(&managed_paths).await?;
        } else {
            prepare_service_restart(&managed_paths).await?;
        }

        let mut service = spawn_service(&managed_paths)?;
        for _ in 0..40 {
            if let Ok(channel) = connect_channel(&managed_paths.service_socket_path).await {
                return Ok(Self {
                    managed_paths,
                    channel,
                });
            }
            if let Some(status) = service.child.try_wait().map_err(|error| {
                JcimSdkError::Bootstrap(format!("unable to observe jcimd startup status: {error}"))
            })? {
                let log_tail = read_bootstrap_log_tail(&service.stderr_log_path);
                return Err(JcimSdkError::Bootstrap(match log_tail {
                    Some(log_tail) => format!(
                        "jcimd exited during startup with status {status}. stderr from {}:\n{log_tail}",
                        service.stderr_log_path.display()
                    ),
                    None => format!(
                        "jcimd exited during startup with status {status}. no stderr was captured at {}",
                        service.stderr_log_path.display()
                    ),
                }));
            }
            sleep(Duration::from_millis(100)).await;
        }

        Err(JcimSdkError::Bootstrap(format!(
            "unable to connect to the JCIM local service at {} after startup; stderr log: {}",
            managed_paths.service_socket_path.display(),
            service.stderr_log_path.display()
        )))
    }

    pub(super) async fn connected_service_matches_current_binary(&self) -> Result<bool> {
        let status = match self.service_status().await {
            Ok(status) => status,
            Err(JcimSdkError::Status(status)) if status.code() == Code::Unimplemented => {
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        let expected_identity = local_service_binary_identity(&service_binary_path()?)?;
        Ok(service_status_matches_binary(&status, &expected_identity))
    }
}

async fn connect_channel(
    socket_path: &Path,
) -> std::result::Result<Channel, tonic::transport::Error> {
    let socket_path = socket_path.to_path_buf();
    Endpoint::try_from("http://[::]:50051")?
        .connect_with_connector(service_fn(move |_| {
            let socket_path = socket_path.clone();
            async move { UnixStream::connect(socket_path).await.map(TokioIo::new) }
        }))
        .await
}

pub(super) fn invalid_connection_target(message: String) -> JcimSdkError {
    JcimError::Unsupported(message).into()
}

struct SpawnedService {
    child: std::process::Child,
    stderr_log_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ServiceBinaryIdentity {
    path: PathBuf,
    fingerprint: String,
}

fn spawn_service(managed_paths: &ManagedPaths) -> Result<SpawnedService> {
    managed_paths.prepare_layout()?;
    let binary = service_binary_path()?;
    std::fs::create_dir_all(&managed_paths.log_dir)?;
    let stderr_log_path = managed_paths.log_dir.join("jcimd-bootstrap.stderr.log");
    let stderr_file = std::fs::File::create(&stderr_log_path).map_err(|error| {
        JcimSdkError::Bootstrap(format!(
            "unable to create jcimd bootstrap log at {}: {error}",
            stderr_log_path.display()
        ))
    })?;
    Command::new(binary)
        .arg("--socket-path")
        .arg(&managed_paths.service_socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map(|child| SpawnedService {
            child,
            stderr_log_path,
        })
        .map_err(|error| JcimSdkError::Bootstrap(format!("unable to launch jcimd: {error}")))
}

fn read_bootstrap_log_tail(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

async fn prepare_service_restart(managed_paths: &ManagedPaths) -> Result<()> {
    let runtime_owner_dir = runtime_owner_dir(managed_paths);
    if let Some(record) =
        validated_runtime_record(&managed_paths.runtime_metadata_path, &runtime_owner_dir)?
    {
        if record.socket_path == managed_paths.service_socket_path
            && record.pid != std::process::id()
            && recorded_process_matches_binary(&record)
        {
            terminate_recorded_service(&record).await?;
        }
        remove_owned_runtime_file_if_present(
            &managed_paths.runtime_metadata_path,
            &runtime_owner_dir,
        )?;
    }

    if managed_paths.service_socket_path.exists() {
        if connect_channel(&managed_paths.service_socket_path)
            .await
            .is_ok()
        {
            return Err(JcimSdkError::Bootstrap(format!(
                "a live jcimd instance is still serving {}; stop it manually before retrying",
                managed_paths.service_socket_path.display()
            )));
        }
        remove_owned_socket_if_present(
            &managed_paths.service_socket_path,
            &managed_paths.runtime_dir,
        )?;
    }
    Ok(())
}

fn validated_runtime_record(
    runtime_metadata_path: &Path,
    owner_dir: &Path,
) -> Result<Option<ServiceRuntimeRecord>> {
    match std::fs::symlink_metadata(runtime_metadata_path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                return Err(JcimSdkError::Bootstrap(format!(
                    "refusing to read symlinked daemon runtime metadata at {}",
                    runtime_metadata_path.display()
                )));
            }
            if !file_type.is_file() {
                return Err(JcimSdkError::Bootstrap(format!(
                    "refusing to read non-file daemon runtime metadata at {}",
                    runtime_metadata_path.display()
                )));
            }
            let owner_metadata = std::fs::metadata(owner_dir)?;
            if metadata.uid() != owner_metadata.uid() {
                return Err(JcimSdkError::Bootstrap(format!(
                    "refusing to trust daemon runtime metadata at {} because its owner does not match {}",
                    runtime_metadata_path.display(),
                    owner_dir.display()
                )));
            }
            ServiceRuntimeRecord::load_if_present(runtime_metadata_path).map_err(Into::into)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn runtime_owner_dir(managed_paths: &ManagedPaths) -> PathBuf {
    managed_paths
        .runtime_metadata_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| managed_paths.runtime_dir.clone())
}

fn recorded_process_matches_binary(record: &ServiceRuntimeRecord) -> bool {
    let output = match Command::new("ps")
        .arg("-p")
        .arg(record.pid.to_string())
        .arg("-o")
        .arg("command=")
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };
    if !output.status.success() {
        return false;
    }

    let command = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if command.is_empty() {
        return false;
    }
    let expected_path = record.service_binary_path.display().to_string();
    let expected_name = record
        .service_binary_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    command == expected_path
        || command.starts_with(&format!("{expected_path} "))
        || (!expected_name.is_empty()
            && (command == expected_name || command.starts_with(&format!("{expected_name} "))))
}

async fn terminate_recorded_service(record: &ServiceRuntimeRecord) -> Result<()> {
    signal_recorded_service(record.pid, "-TERM")?;
    for _ in 0..40 {
        if !process_exists(record.pid) {
            return Ok(());
        }
        sleep(Duration::from_millis(50)).await;
    }

    signal_recorded_service(record.pid, "-KILL")?;
    for _ in 0..20 {
        if !process_exists(record.pid) {
            return Ok(());
        }
        sleep(Duration::from_millis(50)).await;
    }

    Err(JcimSdkError::Bootstrap(format!(
        "recorded jcimd pid {} did not exit after SIGTERM and SIGKILL",
        record.pid
    )))
}

fn process_exists(pid: u32) -> bool {
    let output = match Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("stat=")
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };
    if !output.status.success() {
        return false;
    }
    let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
    !state.is_empty() && !state.starts_with('Z')
}

fn signal_recorded_service(pid: u32, signal: &str) -> Result<()> {
    let status = Command::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| {
            JcimSdkError::Bootstrap(format!(
                "unable to send {signal} to recorded jcimd pid {pid}: {error}"
            ))
        })?;
    if !status.success() {
        return Err(JcimSdkError::Bootstrap(format!(
            "recorded jcimd pid {pid} rejected {signal} with status {status}"
        )));
    }
    Ok(())
}

fn local_service_binary_identity(path: &Path) -> Result<ServiceBinaryIdentity> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(ServiceBinaryIdentity {
        path: path.to_path_buf(),
        fingerprint: format!(
            "{}:{}:{}",
            metadata.len(),
            modified.as_secs(),
            modified.subsec_nanos()
        ),
    })
}

fn service_status_matches_binary(
    status: &ServiceStatusSummary,
    identity: &ServiceBinaryIdentity,
) -> bool {
    status.service_binary_path == identity.path
        && !status.service_binary_fingerprint.trim().is_empty()
        && status.service_binary_fingerprint == identity.fingerprint
}

fn service_binary_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("JCIM_SERVICE_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_jcimd") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }

    let current = std::env::current_exe()?;
    for candidate in binary_candidates(&current, "jcimd") {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(JcimSdkError::Bootstrap(format!(
        "unable to find jcimd near {} or from JCIM_SERVICE_BIN",
        current.display()
    )))
}

fn binary_candidates(current_exe: &Path, name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(parent) = current_exe.parent() {
        candidates.push(parent.join(name));
        if let Some(grandparent) = parent.parent() {
            candidates.push(grandparent.join(name));
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join(format!("{name}.exe")));
            if let Some(grandparent) = parent.parent() {
                candidates.push(grandparent.join(format!("{name}.exe")));
            }
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::types::ServiceStatusSummary;

    use super::{ServiceBinaryIdentity, binary_candidates, service_status_matches_binary};

    #[test]
    fn binary_candidates_check_parent_and_grandparent() {
        let candidates = binary_candidates(Path::new("/tmp/target/debug/examples/demo"), "jcimd");
        assert!(candidates.contains(&PathBuf::from("/tmp/target/debug/examples/jcimd")));
        assert!(candidates.contains(&PathBuf::from("/tmp/target/debug/jcimd")));
    }

    #[test]
    fn service_status_requires_matching_binary_identity() {
        let identity = ServiceBinaryIdentity {
            path: PathBuf::from("/tmp/jcimd"),
            fingerprint: "123:456:789".to_string(),
        };
        let matching = ServiceStatusSummary {
            socket_path: PathBuf::from("/tmp/jcimd.sock"),
            running: true,
            known_project_count: 0,
            active_simulation_count: 0,
            service_binary_path: identity.path.clone(),
            service_binary_fingerprint: identity.fingerprint.clone(),
        };
        let missing_fingerprint = ServiceStatusSummary {
            service_binary_fingerprint: String::new(),
            ..matching.clone()
        };
        let wrong_path = ServiceStatusSummary {
            service_binary_path: PathBuf::from("/tmp/other-jcimd"),
            ..matching.clone()
        };

        assert!(service_status_matches_binary(&matching, &identity));
        assert!(!service_status_matches_binary(
            &missing_fingerprint,
            &identity
        ));
        assert!(!service_status_matches_binary(&wrong_path, &identity));
    }
}
