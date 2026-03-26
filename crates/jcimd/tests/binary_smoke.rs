//! Smoke coverage for the documented `jcimd` binary launch path.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jcim_config::project::ManagedPaths;

#[test]
fn jcimd_binary_starts_and_creates_its_socket() {
    let root = temp_root("binary-smoke");
    let managed_paths = ManagedPaths::for_root(managed_root(&root));

    let mut child = Command::new(env!("CARGO_BIN_EXE_jcimd"))
        .current_dir(repo_root())
        .env("HOME", &root)
        .env("XDG_DATA_HOME", root.join("xdg"))
        .env("NO_COLOR", "1")
        .env_remove("JCIM_SIMULATOR_CONTAINER_CMD")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn jcimd");

    wait_for_socket(&managed_paths.service_socket_path);
    assert!(child.try_wait().expect("poll child").is_none());

    let _ = child.kill();
    let _ = child.wait();
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn managed_root(root: &std::path::Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        root.join("Library/Application Support/jcim")
    }
    #[cfg(not(target_os = "macos"))]
    {
        root.join("xdg/jcim")
    }
}

fn wait_for_socket(socket_path: &std::path::Path) {
    for _ in 0..120 {
        if socket_path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("socket never appeared at {}", socket_path.display());
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jd-{label}-{unique:x}"))
}
