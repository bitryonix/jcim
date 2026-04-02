use super::*;

impl AppState {
    /// Return all tracked simulation summaries sorted by simulation id.
    pub(crate) fn list_simulation_summaries(&self) -> Result<Vec<SimulationSummary>> {
        let simulations = self.simulations.lock().map_err(lock_poisoned)?;
        let mut values = simulations
            .values()
            .map(SimulationRecord::summary)
            .collect::<Vec<_>>();
        values.sort_by(|left, right| left.simulation_id.cmp(&right.simulation_id));
        Ok(values)
    }

    /// Return the summary for one tracked simulation.
    pub(crate) fn simulation_summary(&self, simulation_id: &str) -> Result<SimulationSummary> {
        self.with_simulation(simulation_id, SimulationRecord::summary)
    }

    /// Return retained event lines for one tracked simulation.
    pub(crate) fn simulation_events(&self, simulation_id: &str) -> Result<Vec<EventLine>> {
        self.with_simulation(simulation_id, |simulation| {
            simulation.recent_events.iter().cloned().collect()
        })
    }

    /// Return the authoritative session state tracked for one simulation.
    pub(crate) fn simulation_session_state(&self, simulation_id: &str) -> Result<IsoSessionState> {
        self.with_simulation(simulation_id, |simulation| simulation.session_state.clone())
    }

    /// Return the live backend handle for one running simulation or fail if it is gone.
    pub(crate) fn simulation_handle(&self, simulation_id: &str) -> Result<BackendHandle> {
        self.optional_simulation_handle(simulation_id)?
            .ok_or_else(|| {
                JcimError::BackendUnavailable(format!(
                    "simulation `{simulation_id}` is no longer running"
                ))
            })
    }

    /// Return the live backend handle for one simulation when it still exists.
    pub(crate) fn optional_simulation_handle(
        &self,
        simulation_id: &str,
    ) -> Result<Option<BackendHandle>> {
        self.with_simulation(simulation_id, |simulation| simulation.handle.clone())
    }

    /// Insert or replace one tracked simulation record and return its derived summary.
    pub(crate) fn store_simulation(&self, record: SimulationRecord) -> Result<SimulationSummary> {
        let summary = record.summary();
        self.simulations
            .lock()
            .map_err(lock_poisoned)?
            .insert(record.simulation_id.clone(), record);
        Ok(summary)
    }

    /// Run one read-only operation against a tracked simulation record.
    pub(crate) fn with_simulation<T>(
        &self,
        simulation_id: &str,
        op: impl FnOnce(&SimulationRecord) -> T,
    ) -> Result<T> {
        let simulations = self.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations
            .get(simulation_id)
            .ok_or_else(|| unknown_simulation_error(simulation_id))?;
        Ok(op(simulation))
    }

    /// Run one mutation against a tracked simulation record.
    pub(crate) fn with_simulation_mut<T>(
        &self,
        simulation_id: &str,
        op: impl FnOnce(&mut SimulationRecord) -> T,
    ) -> Result<T> {
        let mut simulations = self.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations
            .get_mut(simulation_id)
            .ok_or_else(|| unknown_simulation_error(simulation_id))?;
        Ok(op(simulation))
    }

    /// Replace the tracked session state for a simulation and append one retained event.
    pub(crate) fn update_simulation_session(
        &self,
        simulation_id: &str,
        session_state: &IsoSessionState,
        level: &str,
        message: impl Into<String>,
    ) -> Result<()> {
        self.with_simulation_mut(simulation_id, |simulation| {
            apply_authoritative_simulation_session(simulation, session_state);
            remember_event(&mut simulation.recent_events, level, message);
        })
    }

    /// Update one simulation status/health snapshot and append one retained event.
    pub(crate) fn update_simulation_status(
        &self,
        simulation_id: &str,
        status: SimulationStatusKind,
        health: impl Into<String>,
        level: &str,
        message: impl Into<String>,
        handle: Option<BackendHandle>,
    ) -> Result<SimulationSummary> {
        self.with_simulation_mut(simulation_id, |simulation| {
            simulation.status = status;
            simulation.health = health.into();
            simulation.handle = handle;
            remember_event(&mut simulation.recent_events, level, message);
            simulation.summary()
        })
    }

    /// Count the simulations still considered active by the local state machine.
    pub(crate) fn active_simulation_count(&self) -> u32 {
        self.simulations
            .lock()
            .map(|simulations| {
                simulations
                    .values()
                    .filter(|simulation| {
                        matches!(
                            simulation.status,
                            SimulationStatusKind::Starting | SimulationStatusKind::Running
                        )
                    })
                    .count() as u32
            })
            .unwrap_or(0)
    }
}

/// Full retained state for one managed simulation.
pub(crate) struct SimulationRecord {
    /// Stable simulation id allocated by the local service.
    pub(crate) simulation_id: String,
    /// Owning project id from the local registry.
    pub(crate) project_id: String,
    /// Owning project root for artifact and manifest lookups.
    pub(crate) project_path: PathBuf,
    /// Current lifecycle status reported by JCIM.
    pub(crate) status: SimulationStatusKind,
    /// Reader name exposed by the backend.
    pub(crate) reader_name: String,
    /// Human-readable health summary for CLI and RPC output.
    pub(crate) health: String,
    /// Current ATR, when one has been observed.
    pub(crate) atr: Option<Atr>,
    /// Current active transport protocol, when one is known.
    pub(crate) active_protocol: Option<ProtocolParameters>,
    /// Reported ISO capability set from the backend snapshot.
    pub(crate) iso_capabilities: IsoCapabilities,
    /// Authoritative ISO/IEC 7816 session state mirrored from the backend.
    pub(crate) session_state: IsoSessionState,
    /// Number of installable packages currently exposed by the backend.
    pub(crate) package_count: u32,
    /// Number of applets currently exposed by the backend.
    pub(crate) applet_count: u32,
    /// Best-effort package name for the running applet package.
    pub(crate) package_name: String,
    /// Best-effort package AID for the running applet package.
    pub(crate) package_aid: String,
    /// Bounded retained event log for recent simulation activity.
    pub(crate) recent_events: VecDeque<EventLine>,
    /// Live backend handle while the simulation remains runnable.
    pub(crate) handle: Option<BackendHandle>,
}

impl SimulationRecord {
    /// Convert the retained simulation record into the stable summary returned to callers.
    pub(super) fn summary(&self) -> SimulationSummary {
        SimulationSummary {
            simulation_id: self.simulation_id.clone(),
            project_id: self.project_id.clone(),
            project_path: self.project_path.clone(),
            status: self.status,
            reader_name: self.reader_name.clone(),
            health: self.health.clone(),
            atr: self.atr.clone(),
            active_protocol: self.active_protocol.clone(),
            iso_capabilities: self.iso_capabilities.clone(),
            session_state: self.session_state.clone(),
            package_count: self.package_count,
            applet_count: self.applet_count,
            package_name: self.package_name.clone(),
            package_aid: self.package_aid.clone(),
            recent_events: self
                .recent_events
                .iter()
                .map(|event| format!("{}: {}", event.level, event.message))
                .collect(),
        }
    }
}

/// Simulation summary plus backend runtime configuration prepared before startup.
pub(crate) struct PreparedSimulation {
    /// Precomputed summary identity reserved for the simulation.
    pub(crate) summary: SimulationSummary,
    /// Backend runtime config passed to the simulator backend factory.
    pub(crate) runtime_config: RuntimeConfig,
}

/// Replace a simulation's tracked session state using an authoritative backend snapshot.
pub(crate) fn apply_authoritative_simulation_session(
    simulation: &mut SimulationRecord,
    session_state: &IsoSessionState,
) {
    simulation.atr = session_state.atr.clone();
    simulation.active_protocol = session_state
        .active_protocol
        .clone()
        .or_else(|| simulation.atr.as_ref().map(ProtocolParameters::from_atr));
    simulation.session_state = session_state.clone();
}

/// Build the standard unknown-simulation error returned by the state store.
fn unknown_simulation_error(simulation_id: &str) -> JcimError {
    JcimError::Unsupported(format!("unknown simulation id `{simulation_id}`"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use jcim_config::project::ManagedPaths;
    use jcim_config::project::UserConfig;
    use jcim_core::iso7816::{Atr, IsoCapabilities, IsoSessionState, PowerState};

    use super::*;
    use crate::card::JavaPhysicalCardAdapter;
    use crate::registry::ProjectRegistry;

    #[test]
    fn list_simulation_summaries_is_sorted_and_active_count_only_tracks_starting_or_running() {
        let root = temp_root("sorted");
        let state = test_state(&root);
        state
            .store_simulation(simulation_record("sim-b", SimulationStatusKind::Running))
            .expect("store running simulation");
        state
            .store_simulation(simulation_record("sim-a", SimulationStatusKind::Starting))
            .expect("store starting simulation");
        state
            .store_simulation(simulation_record("sim-c", SimulationStatusKind::Stopped))
            .expect("store stopped simulation");
        state
            .store_simulation(simulation_record("sim-d", SimulationStatusKind::Failed))
            .expect("store failed simulation");

        let summaries = state
            .list_simulation_summaries()
            .expect("list simulations")
            .into_iter()
            .map(|summary| summary.simulation_id)
            .collect::<Vec<_>>();

        assert_eq!(summaries, vec!["sim-a", "sim-b", "sim-c", "sim-d"]);
        assert_eq!(state.active_simulation_count(), 2);
    }

    #[test]
    fn unknown_simulation_queries_fail_closed() {
        let root = temp_root("unknown");
        let state = test_state(&root);

        let error = state
            .simulation_summary("missing-sim")
            .expect_err("missing simulation should fail");
        assert!(
            error
                .to_string()
                .contains("unknown simulation id `missing-sim`")
        );
    }

    #[test]
    fn update_simulation_session_derives_protocol_from_atr_and_records_events() {
        let root = temp_root("session-update");
        let state = test_state(&root);
        state
            .store_simulation(simulation_record("sim-1", SimulationStatusKind::Starting))
            .expect("store simulation");

        let atr = Atr::parse(&[0x3B, 0x80, 0x01, 0x00]).expect("parse atr");
        let session_state = IsoSessionState::reset(Some(atr.clone()), None);

        state
            .update_simulation_session("sim-1", &session_state, "info", "session refreshed")
            .expect("update session");

        let summary = state.simulation_summary("sim-1").expect("summary");
        assert_eq!(summary.atr, Some(atr.clone()));
        assert_eq!(
            summary.active_protocol,
            Some(ProtocolParameters::from_atr(&atr))
        );
        assert_eq!(summary.session_state, session_state);

        let events = state.simulation_events("sim-1").expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].level, "info");
        assert_eq!(events[0].message, "session refreshed");
    }

    fn test_state(root: &Path) -> AppState {
        AppState::new(
            ManagedPaths::for_root(root.join("managed")),
            PathBuf::from("/tmp/jcimd-test"),
            "fingerprint".to_string(),
            UserConfig::default(),
            ProjectRegistry::default(),
            Arc::new(JavaPhysicalCardAdapter),
            1,
        )
    }

    fn simulation_record(simulation_id: &str, status: SimulationStatusKind) -> SimulationRecord {
        SimulationRecord {
            simulation_id: simulation_id.to_string(),
            project_id: "project-1".to_string(),
            project_path: PathBuf::from("/tmp/project"),
            status,
            reader_name: "Reader".to_string(),
            health: "healthy".to_string(),
            atr: None,
            active_protocol: None,
            iso_capabilities: IsoCapabilities::default(),
            session_state: IsoSessionState {
                power_state: PowerState::On,
                ..IsoSessionState::default()
            },
            package_count: 1,
            applet_count: 1,
            package_name: "com.example.demo".to_string(),
            package_aid: "F000000001".to_string(),
            recent_events: VecDeque::new(),
            handle: None,
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from("/tmp").join(format!("jcim-simulation-state-{label}-{unique:x}"))
    }
}
