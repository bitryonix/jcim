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
    /// Load the effective user configuration with runtime defaults and resolved Java path applied.
    pub(super) fn effective_user_config(&self) -> Result<UserConfig> {
        let mut user_config = self.state.user_config_snapshot()?;
        user_config.bundle_root = Some(user_config.bundle_root.unwrap_or_else(default_bundle_root));
        user_config.java_bin = self.resolved_java_runtime()?.java_bin.display().to_string();
        Ok(user_config)
    }

    /// Resolve one project selector by explicit path first, then by registered project id.
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

    /// Load a project from either a manifest path or a directory that contains a manifest.
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

    /// Load one project directly from its normalized root directory and register it locally.
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

    /// Ensure a project root has a stable local registry id and return it.
    pub(super) fn register_project(&self, project_root: &Path) -> Result<String> {
        self.state.register_project_root(project_root)
    }

    /// Resolve the effective card reader using explicit, project, then global defaults.
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

    /// Resolve one possibly relative input path against the current working directory.
    pub(super) fn resolve_input_path(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(std::env::current_dir()?.join(path))
        }
    }

    /// Return the live backend handle for one selected simulation.
    pub(super) fn simulation_handle(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<BackendHandle> {
        self.state.simulation_handle(&selector.simulation_id)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use jcim_config::project::{ManagedPaths, ProjectConfig};

    use super::*;

    #[test]
    fn resolve_project_prefers_explicit_path_over_project_id() {
        let root = temp_root("selector-precedence");
        let app = load_test_app(&root);
        let first_project = write_project(&root, "First Project", Some("Reader One"));
        let second_project = write_project(&root, "Second Project", Some("Reader Two"));

        let first_loaded = app
            .load_project_by_root(&first_project)
            .expect("load first project");
        let second_loaded = app
            .load_project_by_root(&second_project)
            .expect("load second project");

        let resolved = app
            .resolve_project(&ProjectSelectorInput {
                project_path: Some(first_project.clone()),
                project_id: Some(second_loaded.project_id),
            })
            .expect("resolve project");

        assert_eq!(resolved.project_root, first_loaded.project_root);
        assert_eq!(resolved.project_id, first_loaded.project_id);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn load_project_from_input_accepts_manifest_and_directory_paths() {
        let root = temp_root("selector-input");
        let app = load_test_app(&root);
        let project_root = write_project(&root, "Manifest Project", Some("Reader"));
        let manifest_path = project_root.join(PROJECT_MANIFEST_NAME);

        let from_directory = app
            .load_project_from_input(&project_root)
            .expect("load from directory");
        let from_manifest = app
            .load_project_from_input(&manifest_path)
            .expect("load from manifest");

        assert_eq!(from_directory.project_root, from_manifest.project_root);
        assert_eq!(from_directory.project_id, from_manifest.project_id);
        assert_eq!(from_directory.config, from_manifest.config);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn effective_card_reader_uses_explicit_then_project_then_global_defaults() {
        let root = temp_root("reader-precedence");
        let app = load_test_app(&root);
        let project_with_default = write_project(&root, "Project Reader", Some("Project Reader"));
        let project_without_default = write_project(&root, "Global Reader", None);
        app.state
            .persist_user_config(|config| {
                config.default_reader = Some("Global Reader".to_string());
            })
            .expect("persist user config");

        let project_selector = ProjectSelectorInput {
            project_path: Some(project_with_default),
            project_id: None,
        };
        let global_selector = ProjectSelectorInput {
            project_path: Some(project_without_default),
            project_id: None,
        };

        assert_eq!(
            app.effective_card_reader(Some("Explicit Reader"), Some(&project_selector))
                .expect("explicit reader"),
            Some("Explicit Reader".to_string())
        );
        assert_eq!(
            app.effective_card_reader(None, Some(&project_selector))
                .expect("project reader"),
            Some("Project Reader".to_string())
        );
        assert_eq!(
            app.effective_card_reader(None, Some(&global_selector))
                .expect("global reader fallback"),
            Some("Global Reader".to_string())
        );
        assert_eq!(
            app.effective_card_reader(None, None)
                .expect("global reader"),
            Some("Global Reader".to_string())
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_input_path_joins_relative_paths_and_missing_selector_fails_closed() {
        let root = temp_root("resolve-input");
        let app = load_test_app(&root);
        let relative = PathBuf::from("examples/satochip/workdir");

        assert_eq!(
            app.resolve_input_path(&relative)
                .expect("resolve relative path"),
            std::env::current_dir()
                .expect("current dir")
                .join(&relative)
        );

        let error = app
            .resolve_project(&ProjectSelectorInput::default())
            .err()
            .expect("missing selector should fail");
        assert!(error.to_string().contains("missing project selector"));

        let _ = std::fs::remove_dir_all(root);
    }

    fn load_test_app(root: &Path) -> JcimApp {
        JcimApp::load_with_paths(ManagedPaths::for_root(root.join("managed"))).expect("load app")
    }

    fn write_project(root: &Path, name: &str, default_reader: Option<&str>) -> PathBuf {
        let project_root = root.join(name.replace(' ', "-").to_ascii_lowercase());
        std::fs::create_dir_all(&project_root).expect("create project root");
        let mut config = ProjectConfig::default_for_project_name(name);
        config.card.default_reader = default_reader.map(ToString::to_string);
        std::fs::write(
            project_root.join(PROJECT_MANIFEST_NAME),
            config.to_pretty_toml().expect("encode project"),
        )
        .expect("write project manifest");
        project_root
    }

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from("/tmp").join(format!("jcim-selectors-{label}-{unique:x}"))
    }
}
