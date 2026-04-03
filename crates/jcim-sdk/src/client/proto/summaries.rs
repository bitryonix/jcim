use crate::error::Result;
use crate::types::{
    AppletSummary, ArtifactSummary, ProjectSummary, SimulationStatus, SimulationSummary, owned_path,
};

use super::iso::{
    atr_from_proto, iso_capabilities_from_proto, iso_session_state_from_proto,
    protocol_parameters_from_proto,
};

/// Decode one protobuf project payload into the stable SDK summary type.
pub(in crate::client) fn project_summary(
    project: jcim_api::v0_3::ProjectInfo,
) -> Result<ProjectSummary> {
    Ok(ProjectSummary {
        project_id: project.project_id,
        name: project.name,
        project_path: owned_path(project.project_path),
        profile: project.profile,
        build_kind: project.build_kind,
        package_name: project.package_name,
        package_aid: project.package_aid,
        applets: project
            .applets
            .into_iter()
            .map(|applet| AppletSummary {
                class_name: applet.class_name,
                aid: applet.aid,
            })
            .collect(),
    })
}

/// Decode one protobuf artifact payload into the stable SDK summary type.
pub(in crate::client) fn artifact_summary(
    artifact: jcim_api::v0_3::Artifact,
) -> Result<ArtifactSummary> {
    Ok(ArtifactSummary {
        kind: artifact.kind,
        path: owned_path(artifact.path),
    })
}

/// Decode one protobuf simulation payload into the stable SDK summary type.
pub(in crate::client) fn simulation_summary(
    simulation: jcim_api::v0_3::SimulationInfo,
) -> Result<SimulationSummary> {
    Ok(SimulationSummary {
        simulation_id: simulation.simulation_id,
        project_id: simulation.project_id,
        project_path: owned_path(simulation.project_path),
        status: match jcim_api::v0_3::SimulationStatus::try_from(simulation.status) {
            Ok(jcim_api::v0_3::SimulationStatus::Starting) => SimulationStatus::Starting,
            Ok(jcim_api::v0_3::SimulationStatus::Running) => SimulationStatus::Running,
            Ok(jcim_api::v0_3::SimulationStatus::Stopped) => SimulationStatus::Stopped,
            Ok(jcim_api::v0_3::SimulationStatus::Failed) => SimulationStatus::Failed,
            _ => SimulationStatus::Unknown,
        },
        reader_name: simulation.reader_name,
        health: simulation.health,
        atr: atr_from_proto(simulation.atr)?,
        active_protocol: protocol_parameters_from_proto(simulation.active_protocol),
        iso_capabilities: iso_capabilities_from_proto(simulation.iso_capabilities),
        session_state: iso_session_state_from_proto(simulation.session_state)?,
        package_count: simulation.package_count,
        applet_count: simulation.applet_count,
        package_name: simulation.package_name,
        package_aid: simulation.package_aid,
        recent_events: simulation.recent_events,
    })
}
