use std::path::PathBuf;

use jcim_api::v0_3::{ProjectSelector, SimulationSelector};
use jcim_app::{ProjectSelectorInput, SimulationSelectorInput};

/// Convert the RPC project selector into the application-layer selector model.
pub(crate) fn into_project_selector(selector: ProjectSelector) -> ProjectSelectorInput {
    ProjectSelectorInput {
        project_path: (!selector.project_path.is_empty())
            .then_some(PathBuf::from(selector.project_path)),
        project_id: (!selector.project_id.is_empty()).then_some(selector.project_id),
    }
}

/// Convert the RPC simulation selector into the application-layer selector model.
pub(crate) fn into_simulation_selector(selector: SimulationSelector) -> SimulationSelectorInput {
    SimulationSelectorInput {
        simulation_id: selector.simulation_id,
    }
}
