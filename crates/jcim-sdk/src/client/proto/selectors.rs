use jcim_api::v0_3::{ProjectSelector, SimulationSelector};

use crate::types::ProjectRef;

/// Encode one SDK project selector into the maintained protobuf selector shape.
pub(in crate::client) fn project_selector(project: &ProjectRef) -> ProjectSelector {
    ProjectSelector {
        project_path: project
            .project_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        project_id: project.project_id.clone().unwrap_or_default(),
    }
}

/// Encode one simulation id into the maintained protobuf selector shape.
pub(in crate::client) fn simulation_selector(simulation_id: String) -> SimulationSelector {
    SimulationSelector { simulation_id }
}
