use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "../../../../tests/support/socket.rs"]
mod socket_support;

use jcim_api::v0_3::{ProjectSelector, SimulationSelector};
use jcim_app::{JcimApp, MockPhysicalCardAdapter};
use jcim_config::project::ManagedPaths;

use super::LocalRpc;

pub(super) fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcimd-rpc-{label}-{unique:x}"))
}

pub(super) fn acquire_local_service_lock() -> socket_support::CrossProcessTestLock {
    socket_support::acquire_cross_process_lock("local-service")
}

pub(super) fn load_rpc(root: &Path) -> LocalRpc {
    LocalRpc {
        app: JcimApp::load_with_paths_and_card_adapter(
            ManagedPaths::for_root(root.to_path_buf()),
            Arc::new(MockPhysicalCardAdapter::new()),
        )
        .expect("load rpc app"),
    }
}

pub(super) fn create_demo_project(rpc: &LocalRpc, root: &Path, name: &str) -> PathBuf {
    let project_root = root.join(name.replace(' ', "-").to_ascii_lowercase());
    rpc.app
        .create_project(name, &project_root)
        .expect("create demo project");
    project_root
}

pub(super) fn project_selector(project_root: &Path) -> ProjectSelector {
    ProjectSelector {
        project_path: project_root.display().to_string(),
        project_id: String::new(),
    }
}

pub(super) fn simulation_selector(simulation_id: &str) -> SimulationSelector {
    SimulationSelector {
        simulation_id: simulation_id.to_string(),
    }
}
