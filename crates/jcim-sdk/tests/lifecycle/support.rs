#[path = "../../../../tests/support/integration_runtime.rs"]
mod integration_runtime;
#[path = "../../examples/support/satochip.rs"]
pub(crate) mod satochip_support;

use std::path::PathBuf;
use std::sync::OnceLock;

pub(crate) use integration_runtime::repo_root;
pub(crate) use integration_runtime::socket_support;
pub(crate) use integration_runtime::{stop_managed_daemon, wait_for_path_absent};

pub(crate) fn canonical_jcimd_binary() -> PathBuf {
    integration_runtime::canonical_jcimd_binary()
}

pub(crate) fn copy_jcimd_binary(destination: PathBuf) -> PathBuf {
    integration_runtime::copy_binary(&canonical_jcimd_binary(), destination)
}

pub(crate) fn temp_root(label: &str) -> PathBuf {
    integration_runtime::temp_root("jcim-sdk", label)
}

pub(crate) fn lifecycle_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

pub(crate) async fn wait_for_child_exit(child: &mut std::process::Child) {
    integration_runtime::wait_for_child_exit(
        child,
        "copied jcimd child did not exit after the SDK replaced it",
    )
    .await;
}
