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

    pub(crate) fn active_simulation_count(&self) -> u32 {
        self.state.active_simulation_count()
    }
}
