use jcim_api::v0_3::{
    AppletInfo, Artifact, GetProjectResponse, ProjectInfo, SimulationInfo, SimulationStatus,
};
use jcim_app::{ArtifactSummary, ProjectDetails, ProjectSummary, SimulationSummary};

use super::iso::{
    atr_info, iso_capabilities_info, iso_session_state_info, protocol_parameters_info,
};

/// Encode project details into the RPC response envelope.
pub(crate) fn project_details_response(details: ProjectDetails) -> GetProjectResponse {
    GetProjectResponse {
        project: Some(project_info(details.project)),
        manifest_toml: details.manifest_toml,
    }
}

/// Convert one project summary into the RPC project-info message.
pub(crate) fn project_info(project: ProjectSummary) -> ProjectInfo {
    ProjectInfo {
        project_id: project.project_id,
        name: project.name,
        project_path: project.project_path.display().to_string(),
        profile: project.profile,
        build_kind: project.build_kind,
        package_name: project.package_name,
        package_aid: project.package_aid,
        applets: project
            .applets
            .into_iter()
            .map(|applet| AppletInfo {
                class_name: applet.class_name,
                aid: applet.aid,
            })
            .collect(),
    }
}

/// Convert one artifact summary into the RPC artifact message.
pub(crate) fn artifact_info(artifact: ArtifactSummary) -> Artifact {
    Artifact {
        kind: artifact.kind,
        path: artifact.path.display().to_string(),
    }
}

/// Convert one simulation summary into the RPC simulation-info message.
pub(crate) fn simulation_info(simulation: SimulationSummary) -> SimulationInfo {
    SimulationInfo {
        simulation_id: simulation.simulation_id,
        project_id: simulation.project_id,
        project_path: simulation.project_path.display().to_string(),
        status: match simulation.status {
            jcim_app::SimulationStatusKind::Starting => SimulationStatus::Starting as i32,
            jcim_app::SimulationStatusKind::Running => SimulationStatus::Running as i32,
            jcim_app::SimulationStatusKind::Stopped => SimulationStatus::Stopped as i32,
            jcim_app::SimulationStatusKind::Failed => SimulationStatus::Failed as i32,
        },
        reader_name: simulation.reader_name,
        health: simulation.health,
        atr: simulation.atr.as_ref().map(atr_info),
        active_protocol: simulation
            .active_protocol
            .as_ref()
            .map(protocol_parameters_info),
        iso_capabilities: Some(iso_capabilities_info(&simulation.iso_capabilities)),
        session_state: Some(iso_session_state_info(&simulation.session_state)),
        package_count: simulation.package_count,
        applet_count: simulation.applet_count,
        package_name: simulation.package_name,
        package_aid: simulation.package_aid,
        recent_events: simulation.recent_events,
    }
}
