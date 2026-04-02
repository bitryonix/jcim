use super::*;

impl JcimApp {
    /// Build one project and return emitted artifacts.
    pub fn build_project(
        &self,
        selector: &ProjectSelectorInput,
    ) -> Result<(ProjectSummary, Vec<ArtifactSummary>, bool)> {
        let resolved = self.resolve_project(selector)?;
        let request = artifact_metadata_from_project(&resolved.project_root, &resolved.config)?;
        let toolchain = build_toolchain_layout()?;
        let java_runtime = self.resolved_java_runtime()?;
        self.remember_build_event(
            &resolved.project_id,
            "info",
            format!("building project {}", resolved.project_root.display()),
        );
        let outcome = build_project_artifacts_if_stale_with_java_bin(
            &request,
            &toolchain,
            &java_runtime.java_bin,
        )?;
        self.remember_build_event(
            &resolved.project_id,
            "info",
            if outcome.rebuilt {
                "build completed".to_string()
            } else {
                "build reused current artifacts".to_string()
            },
        );
        Ok((
            self.project_summary(&resolved),
            artifacts_from_metadata(&resolved.project_root, &outcome.metadata),
            outcome.rebuilt,
        ))
    }

    /// Return the current artifact metadata for one project without rebuilding it.
    pub fn get_artifacts(
        &self,
        selector: &ProjectSelectorInput,
    ) -> Result<(ProjectSummary, Vec<ArtifactSummary>)> {
        let resolved = self.resolve_project(selector)?;
        let metadata = load_artifact_metadata(&resolved.project_root)?.ok_or_else(|| {
            JcimError::Unsupported(
                "no artifact metadata found for this project; run `jcim build` first".to_string(),
            )
        })?;
        Ok((
            self.project_summary(&resolved),
            artifacts_from_metadata(&resolved.project_root, &metadata),
        ))
    }

    /// Return retained build events for one project.
    pub fn build_events(&self, selector: &ProjectSelectorInput) -> Result<Vec<EventLine>> {
        let resolved = self.resolve_project(selector)?;
        self.state.build_events_for(&resolved.project_id)
    }

    /// Append one retained build event to the in-memory event store for a project.
    fn remember_build_event(&self, project_id: &str, level: &str, message: String) {
        self.state.remember_build_event(project_id, level, message);
    }
}

#[cfg(test)]
mod tests {
    use crate::app::testsupport::{load_test_app, project_selector, temp_root};

    #[test]
    fn get_artifacts_fails_closed_without_recorded_metadata() {
        let root = temp_root("builds-missing");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");

        let error = app
            .get_artifacts(&project_selector(&project_root))
            .expect_err("artifacts should require metadata");
        assert!(error.to_string().contains("run `jcim build` first"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn build_project_records_rebuild_and_reuse_events() {
        let root = temp_root("builds-events");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");
        let selector = project_selector(&project_root);

        let (project, first_artifacts, rebuilt_first) =
            app.build_project(&selector).expect("initial build");
        let (_, second_artifacts, rebuilt_second) =
            app.build_project(&selector).expect("reused build");
        let events = app.build_events(&selector).expect("build events");

        assert_eq!(project.name, "Demo");
        assert!(rebuilt_first);
        assert!(!rebuilt_second);
        assert_eq!(first_artifacts, second_artifacts);
        assert!(
            events
                .iter()
                .any(|event| event.message.contains("build completed"))
        );
        assert!(
            events
                .iter()
                .any(|event| event.message.contains("build reused current artifacts"))
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
