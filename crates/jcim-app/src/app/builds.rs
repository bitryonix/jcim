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

    fn remember_build_event(&self, project_id: &str, level: &str, message: String) {
        self.state.remember_build_event(project_id, level, message);
    }
}
