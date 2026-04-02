use super::*;

impl AppState {
    /// Return the currently registered project roots from the local registry snapshot.
    pub(crate) fn registered_project_paths(&self) -> Result<Vec<PathBuf>> {
        self.registry.read().map_err(lock_poisoned).map(|registry| {
            registry
                .projects
                .iter()
                .map(|entry| entry.project_path.clone())
                .collect()
        })
    }

    /// Resolve one registered project id into its normalized project root.
    pub(crate) fn project_path_for_id(&self, project_id: &str) -> Result<PathBuf> {
        self.registry
            .read()
            .map_err(lock_poisoned)?
            .by_id(project_id)
            .map(|entry| entry.project_path.clone())
            .ok_or_else(|| JcimError::Unsupported(format!("unknown project id `{project_id}`")))
    }

    /// Upsert one project root into the registry and persist the updated registry file.
    pub(crate) fn register_project_root(&self, project_root: &Path) -> Result<String> {
        let mut registry = self.registry.write().map_err(lock_poisoned)?;
        let record = registry.upsert(project_root)?;
        registry.save_to_path(&self.managed_paths.registry_path)?;
        Ok(record.project_id)
    }
}

/// Fully resolved JCIM project metadata loaded from disk plus its local registry id.
pub(crate) struct ResolvedProject {
    /// Stable local registry id for the project.
    pub(crate) project_id: String,
    /// Normalized absolute project root.
    pub(crate) project_root: PathBuf,
    /// Raw manifest contents as read from disk.
    pub(crate) manifest_toml: String,
    /// Parsed project configuration model.
    pub(crate) config: ProjectConfig,
}

#[cfg(test)]
mod tests {
    use crate::app::testsupport::{load_test_app, temp_root};
    use crate::registry::ProjectRegistry;

    #[test]
    fn register_project_root_persists_and_resolves_paths() {
        let root = temp_root("state-registry");
        let app = load_test_app(&root);
        let workspace = root.join("workspace");
        let project_a = workspace.join("a");
        let project_b = workspace.join("b");
        std::fs::create_dir_all(&project_a).expect("create project a");
        std::fs::create_dir_all(&project_b).expect("create project b");

        let project_b_id = app
            .state
            .register_project_root(&project_b)
            .expect("register project b");
        let project_a_id = app
            .state
            .register_project_root(&project_a.join("."))
            .expect("register project a");

        let canonical_a = project_a.canonicalize().expect("canonical project a");
        let canonical_b = project_b.canonicalize().expect("canonical project b");
        assert_eq!(
            app.state
                .registered_project_paths()
                .expect("registered project paths"),
            vec![canonical_a.clone(), canonical_b.clone()]
        );
        assert_eq!(
            app.state
                .project_path_for_id(&project_a_id)
                .expect("project a path"),
            canonical_a
        );
        assert_eq!(
            app.state
                .project_path_for_id(&project_b_id)
                .expect("project b path"),
            canonical_b
        );

        let saved = ProjectRegistry::load_or_default(&app.state.managed_paths.registry_path)
            .expect("load saved registry");
        assert_eq!(saved.projects.len(), 2);
        assert_eq!(saved.projects[0].project_id, project_a_id);
        assert_eq!(saved.projects[1].project_id, project_b_id);

        let error = app
            .state
            .project_path_for_id("missing-project")
            .expect_err("missing project id should fail");
        assert!(
            error
                .to_string()
                .contains("unknown project id `missing-project`")
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
