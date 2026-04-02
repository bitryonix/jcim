#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::fmt::Display;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

const ASYNC_SOCKET_POLL_ATTEMPTS: usize = 40;
const CROSS_PROCESS_LOCK_ATTEMPTS: usize = 480;
const SYNC_SOCKET_POLL_ATTEMPTS: usize = 120;
const CROSS_PROCESS_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(250);
const SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub struct CrossProcessTestLock {
    path: PathBuf,
}

pub fn acquire_cross_process_lock(scope: &str) -> CrossProcessTestLock {
    let path = cross_process_lock_path(scope);
    let owner = format!("pid={}\n", std::process::id());

    for _ in 0..CROSS_PROCESS_LOCK_ATTEMPTS {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(owner.as_bytes())
                    .expect("write cross-process test lock owner");
                return CrossProcessTestLock { path };
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                std::thread::sleep(CROSS_PROCESS_LOCK_POLL_INTERVAL);
            }
            Err(error) => panic!(
                "failed to acquire cross-process test lock `{scope}` at {}: {error}",
                path.display()
            ),
        }
    }

    let owner = std::fs::read_to_string(&path).unwrap_or_default();
    let owner = owner.trim();
    let owner_suffix = if owner.is_empty() {
        String::new()
    } else {
        format!(" (current owner: {owner})")
    };
    panic!(
        "timed out waiting for cross-process test lock `{scope}` at {}{}",
        path.display(),
        owner_suffix
    );
}

pub fn unix_domain_sockets_supported(test_name: &str) -> bool {
    let probe_path = probe_socket_path(test_name);
    match std::os::unix::net::UnixListener::bind(&probe_path) {
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

pub async fn wait_for_socket_or_server_exit<E>(
    socket_path: &Path,
    server: &mut JoinHandle<Result<(), E>>,
) where
    E: Display,
{
    for _ in 0..ASYNC_SOCKET_POLL_ATTEMPTS {
        if socket_path.exists() {
            return;
        }
        if server.is_finished() {
            panic!("{}", describe_server_exit(socket_path, server).await);
        }
        sleep(SOCKET_POLL_INTERVAL).await;
    }

    if server.is_finished() {
        panic!("{}", describe_server_exit(socket_path, server).await);
    }

    panic!(
        "socket never appeared at {} while the local service task was still running",
        socket_path.display()
    );
}

pub fn wait_for_socket_or_child_exit(
    socket_path: &Path,
    child: &mut Child,
    stderr_log_path: &Path,
) {
    for _ in 0..SYNC_SOCKET_POLL_ATTEMPTS {
        if socket_path.exists() {
            return;
        }
        if let Some(status) = child.try_wait().expect("poll child process") {
            panic!(
                "socket never appeared at {} because the child process exited with status {}{}",
                socket_path.display(),
                status,
                format_log_excerpt(stderr_log_path)
            );
        }
        std::thread::sleep(SOCKET_POLL_INTERVAL);
    }

    if let Some(status) = child.try_wait().expect("poll child process") {
        panic!(
            "socket never appeared at {} because the child process exited with status {}{}",
            socket_path.display(),
            status,
            format_log_excerpt(stderr_log_path)
        );
    }

    panic!(
        "socket never appeared at {} before the timeout elapsed; stderr log: {}{}",
        socket_path.display(),
        stderr_log_path.display(),
        format_log_excerpt(stderr_log_path)
    );
}

impl Drop for CrossProcessTestLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

async fn describe_server_exit<E>(
    socket_path: &Path,
    server: &mut JoinHandle<Result<(), E>>,
) -> String
where
    E: Display,
{
    match server.await {
        Ok(Ok(())) => format!(
            "socket never appeared at {} because the local service task exited cleanly before creating it",
            socket_path.display()
        ),
        Ok(Err(error)) => format!(
            "socket never appeared at {} because the local service task failed before creating it: {error}",
            socket_path.display()
        ),
        Err(error) => format!(
            "socket never appeared at {} because the local service task panicked or was cancelled before creating it: {error}",
            socket_path.display()
        ),
    }
}

fn probe_socket_path(test_name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let base_dir = if Path::new("/tmp").is_dir() {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    };
    let short_name = test_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(12)
        .collect::<String>();
    base_dir.join(format!("j{short_name}{unique:x}.sock"))
}

fn cross_process_lock_path(scope: &str) -> PathBuf {
    let base_dir = if Path::new("/tmp").is_dir() {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    };
    let short_scope = scope
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .take(24)
        .collect::<String>();
    let scope = if short_scope.is_empty() {
        "default".to_string()
    } else {
        short_scope
    };
    base_dir.join(format!("jcim-test-lock-{scope}.lock"))
}

fn format_log_excerpt(stderr_log_path: &Path) -> String {
    match std::fs::read_to_string(stderr_log_path) {
        Ok(contents) => {
            let trimmed = contents.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("\nstderr:\n{trimmed}")
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => format!(
            "\nfailed to read stderr log {}: {error}",
            stderr_log_path.display()
        ),
    }
}
