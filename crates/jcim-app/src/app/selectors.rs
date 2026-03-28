use std::path::{Path, PathBuf};

use jcim_backends::backend::BackendHandle;
use jcim_config::project::{
    PROJECT_MANIFEST_NAME, ProjectConfig, UserConfig, find_project_manifest,
};
use jcim_core::error::{JcimError, Result};

use super::JcimApp;
use super::default_bundle_root;
use super::state::ResolvedProject;
use crate::model::{ProjectSelectorInput, SimulationSelectorInput};
use crate::registry::normalize_project_root;

impl JcimApp {
    pub(super) fn effective_user_config(&self) -> Result<UserConfig> {
        let mut user_config = self.state.user_config_snapshot()?;
        user_config.bundle_root = Some(user_config.bundle_root.unwrap_or_else(default_bundle_root));
        user_config.java_bin = self.resolved_java_runtime()?.java_bin.display().to_string();
        Ok(user_config)
    }

    pub(super) fn resolve_project(
        &self,
        selector: &ProjectSelectorInput,
    ) -> Result<ResolvedProject> {
        if let Some(project_path) = &selector.project_path {
            return self.load_project_from_input(project_path);
        }
        if let Some(project_id) = &selector.project_id {
            let project_path = self.state.project_path_for_id(project_id)?;
            return self.load_project_by_root(&project_path);
        }
        Err(JcimError::Unsupported(
            "missing project selector; pass a project path or id".to_string(),
        ))
    }

    pub(super) fn load_project_from_input(&self, input: &Path) -> Result<ResolvedProject> {
        let manifest_path = if input.is_file() {
            input.to_path_buf()
        } else {
            find_project_manifest(input)
                .or_else(|| {
                    let candidate = input.join(PROJECT_MANIFEST_NAME);
                    candidate.exists().then_some(candidate)
                })
                .ok_or_else(|| {
                    JcimError::Unsupported(format!("no jcim.toml found under {}", input.display()))
                })?
        };
        let project_root = manifest_path.parent().ok_or_else(|| {
            JcimError::Unsupported(format!(
                "project manifest path has no parent: {}",
                manifest_path.display()
            ))
        })?;
        self.load_project_by_root(project_root)
    }

    pub(super) fn load_project_by_root(&self, project_root: &Path) -> Result<ResolvedProject> {
        let normalized_root = normalize_project_root(project_root)?;
        let manifest_path = normalized_root.join(PROJECT_MANIFEST_NAME);
        let manifest_toml = std::fs::read_to_string(&manifest_path)?;
        let config = ProjectConfig::from_toml_str(&manifest_toml)?;
        let project_id = self.register_project(&normalized_root)?;
        Ok(ResolvedProject {
            project_id,
            project_root: normalized_root,
            manifest_toml,
            config,
        })
    }

    pub(super) fn register_project(&self, project_root: &Path) -> Result<String> {
        self.state.register_project_root(project_root)
    }

    pub(super) fn effective_card_reader(
        &self,
        reader_name: Option<&str>,
        selector: Option<&ProjectSelectorInput>,
    ) -> Result<Option<String>> {
        if let Some(reader_name) = reader_name {
            return Ok(Some(reader_name.to_string()));
        }
        if let Some(selector) = selector
            && (selector.project_id.is_some() || selector.project_path.is_some())
            && let Ok(project) = self.resolve_project(selector)
            && let Some(reader) = project.config.card.default_reader
        {
            return Ok(Some(reader));
        }
        self.state.default_reader()
    }

    pub(super) fn resolve_input_path(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(std::env::current_dir()?.join(path))
        }
    }

    pub(super) fn simulation_handle(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<BackendHandle> {
        self.state.simulation_handle(&selector.simulation_id)
    }
}
