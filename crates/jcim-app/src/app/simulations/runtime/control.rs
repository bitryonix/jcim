use super::*;

impl JcimApp {
    /// Stop one managed simulation.
    pub async fn stop_simulation(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<SimulationSummary> {
        if let Some(handle) = self
            .state
            .optional_simulation_handle(&selector.simulation_id)?
        {
            let _ = handle.shutdown().await;
        }

        self.state.update_simulation_status(
            &selector.simulation_id,
            SimulationStatusKind::Stopped,
            "stopped",
            "info",
            "simulation stopped",
            None,
        )
    }

    /// Reset the selected simulation and return the current ATR.
    pub async fn reset_simulation_summary(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<ResetSummary> {
        let handle = self.simulation_handle(selector)?;
        let reset = handle.reset().await?;
        let parsed_atr = reset
            .atr
            .clone()
            .or_else(|| reset.session_state.atr.clone());
        let session_state = reset.session_state;
        let _ = self.state.update_simulation_session(
            &selector.simulation_id,
            &session_state,
            "info",
            "simulation reset",
        );
        Ok(ResetSummary {
            atr: parsed_atr,
            session_state,
        })
    }
}
