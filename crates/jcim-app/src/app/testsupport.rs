use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "../../../../tests/support/socket.rs"]
mod socket_support;

use jcim_config::project::{ManagedPaths, PROJECT_MANIFEST_NAME, ProjectConfig};

use super::JcimApp;
use crate::card::{MockPhysicalCardAdapter, PhysicalCardAdapter};
use crate::model::{ProjectSelectorInput, SimulationSelectorInput};

pub(crate) fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-app-{label}-{unique:x}"))
}

pub(crate) fn acquire_local_service_lock() -> socket_support::CrossProcessTestLock {
    socket_support::acquire_cross_process_lock("local-service")
}

pub(crate) fn load_test_app(root: &Path) -> JcimApp {
    load_test_app_with_adapter(root, Arc::new(MockPhysicalCardAdapter::new()))
}

pub(crate) fn load_test_app_with_adapter(
    root: &Path,
    card_adapter: Arc<dyn PhysicalCardAdapter>,
) -> JcimApp {
    JcimApp::load_with_paths_and_card_adapter(
        ManagedPaths::for_root(root.to_path_buf()),
        card_adapter,
    )
    .expect("load test app")
}

pub(crate) fn project_selector(project_root: &Path) -> ProjectSelectorInput {
    ProjectSelectorInput {
        project_path: Some(project_root.to_path_buf()),
        project_id: None,
    }
}

pub(crate) fn simulation_selector(simulation_id: impl Into<String>) -> SimulationSelectorInput {
    SimulationSelectorInput {
        simulation_id: simulation_id.into(),
    }
}

pub(crate) fn read_project_config(project_root: &Path) -> ProjectConfig {
    ProjectConfig::from_toml_path(&project_root.join(PROJECT_MANIFEST_NAME))
        .expect("read project config")
}

pub(crate) fn write_project_config(project_root: &Path, config: &ProjectConfig) {
    std::fs::write(
        project_root.join(PROJECT_MANIFEST_NAME),
        config.to_pretty_toml().expect("encode project config"),
    )
    .expect("write project config");
}
