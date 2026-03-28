use super::*;

impl AppState {
    pub(crate) fn list_simulation_summaries(&self) -> Result<Vec<SimulationSummary>> {
        let simulations = self.simulations.lock().map_err(lock_poisoned)?;
        let mut values = simulations
            .values()
            .map(SimulationRecord::summary)
            .collect::<Vec<_>>();
        values.sort_by(|left, right| left.simulation_id.cmp(&right.simulation_id));
        Ok(values)
    }

    pub(crate) fn simulation_summary(&self, simulation_id: &str) -> Result<SimulationSummary> {
        self.with_simulation(simulation_id, SimulationRecord::summary)
    }

    pub(crate) fn simulation_events(&self, simulation_id: &str) -> Result<Vec<EventLine>> {
        self.with_simulation(simulation_id, |simulation| {
            simulation.recent_events.iter().cloned().collect()
        })
    }

    pub(crate) fn simulation_session_state(&self, simulation_id: &str) -> Result<IsoSessionState> {
        self.with_simulation(simulation_id, |simulation| simulation.session_state.clone())
    }

    pub(crate) fn simulation_handle(&self, simulation_id: &str) -> Result<BackendHandle> {
        self.optional_simulation_handle(simulation_id)?
            .ok_or_else(|| {
                JcimError::BackendUnavailable(format!(
                    "simulation `{simulation_id}` is no longer running"
                ))
            })
    }

    pub(crate) fn optional_simulation_handle(
        &self,
        simulation_id: &str,
    ) -> Result<Option<BackendHandle>> {
        self.with_simulation(simulation_id, |simulation| simulation.handle.clone())
    }

    pub(crate) fn store_simulation(&self, record: SimulationRecord) -> Result<SimulationSummary> {
        let summary = record.summary();
        self.simulations
            .lock()
            .map_err(lock_poisoned)?
            .insert(record.simulation_id.clone(), record);
        Ok(summary)
    }

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

pub(crate) struct SimulationRecord {
    pub(crate) simulation_id: String,
    pub(crate) project_id: String,
    pub(crate) project_path: PathBuf,
    pub(crate) status: SimulationStatusKind,
    pub(crate) reader_name: String,
    pub(crate) health: String,
    pub(crate) atr: Option<Atr>,
    pub(crate) active_protocol: Option<ProtocolParameters>,
    pub(crate) iso_capabilities: IsoCapabilities,
    pub(crate) session_state: IsoSessionState,
    pub(crate) package_count: u32,
    pub(crate) applet_count: u32,
    pub(crate) package_name: String,
    pub(crate) package_aid: String,
    pub(crate) recent_events: VecDeque<EventLine>,
    pub(crate) handle: Option<BackendHandle>,
}

impl SimulationRecord {
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

pub(crate) struct PreparedSimulation {
    pub(crate) summary: SimulationSummary,
    pub(crate) runtime_config: RuntimeConfig,
}

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

fn unknown_simulation_error(simulation_id: &str) -> JcimError {
    JcimError::Unsupported(format!("unknown simulation id `{simulation_id}`"))
}
