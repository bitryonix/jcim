use super::*;

impl AppState {
    pub(crate) fn registered_project_paths(&self) -> Result<Vec<PathBuf>> {
        self.registry.read().map_err(lock_poisoned).map(|registry| {
            registry
                .projects
                .iter()
                .map(|entry| entry.project_path.clone())
                .collect()
        })
    }

    pub(crate) fn project_path_for_id(&self, project_id: &str) -> Result<PathBuf> {
        self.registry
            .read()
            .map_err(lock_poisoned)?
            .by_id(project_id)
            .map(|entry| entry.project_path.clone())
            .ok_or_else(|| JcimError::Unsupported(format!("unknown project id `{project_id}`")))
    }

    pub(crate) fn register_project_root(&self, project_root: &Path) -> Result<String> {
        let mut registry = self.registry.write().map_err(lock_poisoned)?;
        let record = registry.upsert(project_root)?;
        registry.save_to_path(&self.managed_paths.registry_path)?;
        Ok(record.project_id)
    }
}

pub(crate) struct ResolvedProject {
    pub(crate) project_id: String,
    pub(crate) project_root: PathBuf,
    pub(crate) manifest_toml: String,
    pub(crate) config: ProjectConfig,
}
