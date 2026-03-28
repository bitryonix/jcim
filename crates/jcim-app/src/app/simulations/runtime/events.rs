use std::collections::VecDeque;

use super::*;

pub(super) fn starting_simulation_record(prepared: &PreparedSimulation) -> SimulationRecord {
    let mut recent_events = VecDeque::new();
    remember_event(
        &mut recent_events,
        "info",
        "simulation prepared from project",
    );
    SimulationRecord {
        simulation_id: prepared.summary.simulation_id.clone(),
        project_id: prepared.summary.project_id.clone(),
        project_path: prepared.summary.project_path.clone(),
        status: SimulationStatusKind::Starting,
        reader_name: prepared.summary.reader_name.clone(),
        health: prepared.summary.health.clone(),
        atr: None,
        active_protocol: None,
        iso_capabilities: IsoCapabilities::default(),
        session_state: IsoSessionState::default(),
        package_count: prepared.summary.package_count,
        applet_count: prepared.summary.applet_count,
        package_name: prepared.summary.package_name.clone(),
        package_aid: prepared.summary.package_aid.clone(),
        recent_events,
        handle: None,
    }
}

pub(super) fn annotate_simulation_start_error(
    prepared: &PreparedSimulation,
    error: JcimError,
) -> JcimError {
    let context = format!(
        "simulation `{}` for project `{}`",
        prepared.summary.simulation_id, prepared.summary.project_id
    );
    match error {
        JcimError::BackendStartup(message) => JcimError::BackendStartup(format!(
            "{context} failed during backend startup: {message}"
        )),
        JcimError::BackendUnavailable(message) => JcimError::BackendUnavailable(format!(
            "{context} became unavailable during startup: {message}"
        )),
        JcimError::BackendExited(message) => {
            JcimError::BackendExited(format!("{context} exited during startup: {message}"))
        }
        JcimError::Unsupported(message) => {
            JcimError::Unsupported(format!("{context} could not be prepared: {message}"))
        }
        other => JcimError::Unsupported(format!("{context} could not start: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{annotate_simulation_start_error, starting_simulation_record};
    use jcim_config::config::RuntimeConfig;
    use jcim_core::error::JcimError;

    use crate::model::{SimulationStatusKind, SimulationSummary};

    #[test]
    fn starting_record_retains_prepared_identity_and_starts_without_handle() {
        let prepared = crate::app::state::PreparedSimulation {
            summary: SimulationSummary {
                simulation_id: "sim-1".to_string(),
                project_id: "project-1".to_string(),
                project_path: std::path::PathBuf::from("/tmp/project"),
                status: SimulationStatusKind::Starting,
                reader_name: "JCIM Simulation".to_string(),
                health: "starting".to_string(),
                atr: None,
                active_protocol: None,
                iso_capabilities: Default::default(),
                session_state: Default::default(),
                package_count: 0,
                applet_count: 0,
                package_name: "pkg".to_string(),
                package_aid: "A000".to_string(),
                recent_events: Vec::new(),
            },
            runtime_config: RuntimeConfig::default(),
        };

        let record = starting_simulation_record(&prepared);
        assert_eq!(record.simulation_id, "sim-1");
        assert!(matches!(record.status, SimulationStatusKind::Starting));
        assert!(record.handle.is_none());
        assert_eq!(record.recent_events.len(), 1);
    }

    #[test]
    fn start_error_annotation_preserves_failure_class_and_context() {
        let prepared = crate::app::state::PreparedSimulation {
            summary: SimulationSummary {
                simulation_id: "sim-2".to_string(),
                project_id: "project-2".to_string(),
                project_path: std::path::PathBuf::from("/tmp/project"),
                status: SimulationStatusKind::Starting,
                reader_name: "JCIM Simulation".to_string(),
                health: "starting".to_string(),
                atr: None,
                active_protocol: None,
                iso_capabilities: Default::default(),
                session_state: Default::default(),
                package_count: 0,
                applet_count: 0,
                package_name: "pkg".to_string(),
                package_aid: "A000".to_string(),
                recent_events: Vec::new(),
            },
            runtime_config: RuntimeConfig::default(),
        };

        let error = annotate_simulation_start_error(
            &prepared,
            JcimError::BackendStartup("timeout".to_string()),
        );
        assert!(
            error.to_string().contains(
                "simulation `sim-2` for project `project-2` failed during backend startup"
            )
        );
    }
}
