//! Smoke coverage for the documented `jcimd` binary launch path.

#![forbid(unsafe_code)]

#[path = "../../../tests/support/socket.rs"]
mod socket_support;

use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_config::project::ManagedPaths;

#[test]
fn jcimd_binary_starts_and_creates_its_socket() {
    if !socket_support::unix_domain_sockets_supported("jcimd_binary_starts_and_creates_its_socket")
    {
        return;
    }

    let root = temp_root("binary-smoke");
    std::fs::create_dir_all(&root).expect("create temp root");
    let managed_paths = ManagedPaths::for_root(root.join("managed"));
    let stderr_log_path = root.join("jcimd.stderr.log");
    let stderr_log = File::create(&stderr_log_path).expect("create stderr log");

    let mut child = Command::new(env!("CARGO_BIN_EXE_jcimd"))
        .current_dir(repo_root())
        .arg("--socket-path")
        .arg(&managed_paths.service_socket_path)
        .env("HOME", &root)
        .env("XDG_DATA_HOME", root.join("xdg"))
        .env("NO_COLOR", "1")
        .env_remove("JCIM_SIMULATOR_CONTAINER_CMD")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .expect("spawn jcimd");

    socket_support::wait_for_socket_or_child_exit(
        &managed_paths.service_socket_path,
        &mut child,
        &stderr_log_path,
    );
    assert!(child.try_wait().expect("poll child").is_none());

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(root);
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jd-{label}-{unique:x}"))
}
