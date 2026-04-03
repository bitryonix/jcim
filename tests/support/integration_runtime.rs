#![forbid(unsafe_code)]
#![allow(dead_code)]

#[path = "socket.rs"]
pub mod socket_support;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_config::project::{ManagedPaths, ServiceRuntimeRecord};

/// Resolve the workspace root for integration-test command execution.
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// Build one unique temporary root path using a stable prefix and human-readable label.
pub fn temp_root(prefix: &str, label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("{prefix}-{label}-{unique:x}"))
}

/// Resolve the maintained `jcimd` binary, preferring explicit test env vars before `target/debug`.
pub fn canonical_jcimd_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("JCIM_SERVICE_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path.canonicalize().unwrap_or(path);
        }
    }
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_jcimd") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path.canonicalize().unwrap_or(path);
        }
    }

    let current = std::env::current_exe().expect("current test executable");
    let parent = current.parent().expect("test executable parent");
    let candidates = [
        parent.join("jcimd"),
        parent
            .parent()
            .expect("test executable grandparent")
            .join("jcimd"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return candidate.canonicalize().unwrap_or(candidate);
        }
    }

    panic!("unable to locate jcimd near {}", current.display());
}

/// Copy or hard-link one binary into a temporary location while preserving executability.
pub fn copy_binary(source: &Path, destination: PathBuf) -> PathBuf {
    match std::fs::hard_link(source, &destination) {
        Ok(()) => destination,
        Err(_) => {
            std::fs::copy(source, &destination).expect("copy binary");
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                let mut permissions = std::fs::metadata(&destination)
                    .expect("copied binary metadata")
                    .permissions();
                permissions.set_mode(0o755);
                std::fs::set_permissions(&destination, permissions)
                    .expect("copied binary permissions");
            }
            destination
        }
    }
}

/// Terminate one managed daemon process and wait for both its socket and runtime metadata to clear.
pub async fn stop_managed_daemon(managed_paths: &ManagedPaths) {
    let Some(record) = ServiceRuntimeRecord::load_if_present(&managed_paths.runtime_metadata_path)
        .expect("load runtime metadata")
    else {
        return;
    };

    let status = Command::new("kill")
        .arg("-TERM")
        .arg(record.pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("send SIGTERM");
    assert!(
        status.success(),
        "SIGTERM failed for recorded jcimd pid {} with status {}",
        record.pid,
        status
    );

    if !wait_for_pid_absent(record.pid, 80).await {
        let kill_status = Command::new("kill")
            .arg("-KILL")
            .arg(record.pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("send SIGKILL");
        assert!(
            kill_status.success(),
            "SIGKILL failed for recorded jcimd pid {} with status {}",
            record.pid,
            kill_status
        );
        assert!(
            wait_for_pid_absent(record.pid, 40).await,
            "jcimd pid {} did not exit after SIGTERM and SIGKILL",
            record.pid
        );
    }
    wait_for_path_absent(&managed_paths.service_socket_path).await;
    wait_for_path_absent(&managed_paths.runtime_metadata_path).await;
}

/// Wait for one child process to exit, failing with the provided description if it stays alive.
pub async fn wait_for_child_exit(child: &mut Child, description: &str) {
    for _ in 0..80 {
        if child.try_wait().expect("poll child").is_some() {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    panic!("{description}");
}

/// Wait until a runtime-managed path disappears.
pub async fn wait_for_path_absent(path: &Path) {
    for _ in 0..80 {
        if !path.exists() {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    panic!("path {} still exists after shutdown", path.display());
}

async fn wait_for_pid_absent(pid: u32, attempts: usize) -> bool {
    for _ in 0..attempts {
        let output = Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("stat=")
            .stdin(Stdio::null())
            .output()
            .expect("poll pid");
        if !output.status.success() {
            return true;
        }
        let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if state.is_empty() || state.starts_with('Z') {
            return true;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    false
}
