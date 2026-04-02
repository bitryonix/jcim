use super::*;

impl JcimApp {
    /// Return the current simulation list.
    pub fn list_simulations(&self) -> Result<Vec<SimulationSummary>> {
        self.state.list_simulation_summaries()
    }

    /// Return one managed simulation by id.
    pub fn get_simulation(&self, selector: &SimulationSelectorInput) -> Result<SimulationSummary> {
        self.state.simulation_summary(&selector.simulation_id)
    }

    /// Return retained simulation events for one running simulation.
    pub fn simulation_events(&self, selector: &SimulationSelectorInput) -> Result<Vec<EventLine>> {
        self.state.simulation_events(&selector.simulation_id)
    }

    /// Return the current tracked ISO/IEC 7816 session state for one simulation.
    pub fn simulation_session_state(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<IsoSessionState> {
        self.state.simulation_session_state(&selector.simulation_id)
    }

    /// Return the number of simulations still considered active by the local state store.
    pub(crate) fn active_simulation_count(&self) -> u32 {
        self.state.active_simulation_count()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::path::PathBuf;

    use jcim_core::iso7816::{Atr, IsoCapabilities, IsoSessionState, ProtocolParameters};

    use super::*;
    use crate::app::testsupport::{load_test_app, simulation_selector, temp_root};
    use crate::model::{EventLine, SimulationStatusKind};

    #[test]
    fn simulation_query_helpers_pass_through_state_queries() {
        let root = temp_root("simulation-query");
        let app = load_test_app(&root);
        let atr = Atr::parse(&[0x3B, 0x80, 0x01, 0x00]).expect("parse atr");
        let session_state = IsoSessionState::reset(Some(atr.clone()), None);

        app.state
            .store_simulation(simulation_record(
                "sim-b",
                SimulationStatusKind::Failed,
                session_state.clone(),
                "failed",
            ))
            .expect("store failed simulation");
        app.state
            .store_simulation(simulation_record(
                "sim-a",
                SimulationStatusKind::Running,
                session_state.clone(),
                "ready",
            ))
            .expect("store running simulation");

        let listed = app.list_simulations().expect("list simulations");
        assert_eq!(
            listed
                .iter()
                .map(|summary| summary.simulation_id.as_str())
                .collect::<Vec<_>>(),
            vec!["sim-a", "sim-b"]
        );

        let selector = simulation_selector("sim-a");
        let summary = app.get_simulation(&selector).expect("get simulation");
        assert_eq!(summary.simulation_id, "sim-a");
        assert_eq!(summary.health, "ready");

        let events = app.simulation_events(&selector).expect("simulation events");
        assert_eq!(
            events,
            vec![EventLine {
                level: "info".to_string(),
                message: "simulation ready".to_string(),
            }]
        );
        assert_eq!(
            app.simulation_session_state(&selector)
                .expect("simulation session state"),
            session_state
        );
        assert_eq!(app.active_simulation_count(), 1);

        let _ = std::fs::remove_dir_all(root);
    }

    fn simulation_record(
        simulation_id: &str,
        status: SimulationStatusKind,
        session_state: IsoSessionState,
        health: &str,
    ) -> SimulationRecord {
        SimulationRecord {
            simulation_id: simulation_id.to_string(),
            project_id: "project-1".to_string(),
            project_path: PathBuf::from("/tmp/project"),
            status,
            reader_name: "Reader".to_string(),
            health: health.to_string(),
            atr: session_state.atr.clone(),
            active_protocol: session_state.atr.as_ref().map(ProtocolParameters::from_atr),
            iso_capabilities: IsoCapabilities::default(),
            session_state,
            package_count: 1,
            applet_count: 1,
            package_name: "com.example.demo".to_string(),
            package_aid: "F000000001".to_string(),
            recent_events: VecDeque::from([EventLine {
                level: "info".to_string(),
                message: format!("simulation {health}"),
            }]),
            handle: None,
        }
    }
}
