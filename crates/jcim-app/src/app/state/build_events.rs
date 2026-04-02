use super::*;

impl AppState {
    /// Return retained build events for one project in insertion order.
    pub(crate) fn build_events_for(&self, project_id: &str) -> Result<Vec<EventLine>> {
        let events = self
            .build_events
            .lock()
            .map_err(lock_poisoned)?
            .get(project_id)
            .cloned()
            .unwrap_or_default();
        Ok(events.into_iter().collect())
    }

    /// Append one retained build event for a project when the store is still available.
    pub(crate) fn remember_build_event(
        &self,
        project_id: &str,
        level: &str,
        message: impl Into<String>,
    ) {
        if let Ok(mut events) = self.build_events.lock() {
            let queue = events.entry(project_id.to_string()).or_default();
            remember_event(queue, level, message);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::AppState;
    use crate::MockPhysicalCardAdapter;
    use crate::registry::ProjectRegistry;
    use jcim_config::project::{ManagedPaths, UserConfig};

    #[test]
    fn build_events_are_retained_through_the_state_store() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let managed_paths = ManagedPaths::for_root(
            PathBuf::from("/tmp").join(format!("jcim-state-build-events-{unique:x}")),
        );
        let state = AppState::new(
            managed_paths,
            PathBuf::from("/tmp/jcimd"),
            "fingerprint".to_string(),
            UserConfig::default(),
            ProjectRegistry::default(),
            Arc::new(MockPhysicalCardAdapter::new()),
            1,
        );

        state.remember_build_event("project-1", "info", "build started");
        state.remember_build_event("project-1", "info", "build finished");

        let events = state
            .build_events_for("project-1")
            .expect("read retained build events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].message, "build started");
        assert_eq!(events[1].message, "build finished");
    }
}
