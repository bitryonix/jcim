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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::testsupport::{
        acquire_local_service_lock, load_test_app, project_selector, simulation_selector, temp_root,
    };

    #[tokio::test]
    async fn reset_simulation_summary_records_events_and_updates_session_state() {
        let _service_lock = acquire_local_service_lock();
        let root = temp_root("sim-reset");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");

        let simulation = app
            .start_project_simulation(&project_selector(&project_root))
            .await
            .expect("start simulation");
        let selector = simulation_selector(simulation.simulation_id.clone());

        let reset = app
            .reset_simulation_summary(&selector)
            .await
            .expect("reset simulation");
        let events = app.simulation_events(&selector).expect("simulation events");

        assert!(reset.atr.is_some());
        assert_eq!(
            app.simulation_session_state(&selector)
                .expect("session state after reset"),
            reset.session_state
        );
        assert!(
            events
                .iter()
                .any(|event| event.message.contains("simulation reset"))
        );

        let _ = app.stop_simulation(&selector).await;
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn stop_simulation_succeeds_when_handle_is_already_missing() {
        let _service_lock = acquire_local_service_lock();
        let root = temp_root("sim-stop-missing-handle");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");

        let simulation = app
            .start_project_simulation(&project_selector(&project_root))
            .await
            .expect("start simulation");
        let selector = simulation_selector(simulation.simulation_id.clone());
        app.state
            .with_simulation_mut(&selector.simulation_id, |simulation| {
                simulation.handle = None;
            })
            .expect("clear handle");

        let stopped = app
            .stop_simulation(&selector)
            .await
            .expect("stop simulation");

        assert_eq!(stopped.status, SimulationStatusKind::Stopped);
        assert_eq!(
            app.get_simulation(&selector)
                .expect("stored simulation")
                .status,
            SimulationStatusKind::Stopped
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
