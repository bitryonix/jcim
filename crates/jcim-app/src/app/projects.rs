use super::*;

impl JcimApp {
    /// Return the registered project list with current manifest metadata.
    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let mut projects = Vec::new();
        for project_path in self.state.registered_project_paths()? {
            if let Ok(resolved) = self.load_project_by_root(&project_path) {
                projects.push(self.project_summary(&resolved));
            }
        }
        projects.sort_by(|left, right| left.project_path.cmp(&right.project_path));
        Ok(projects)
    }

    /// Create a new project skeleton and register it locally.
    pub fn create_project(&self, name: &str, directory: &Path) -> Result<ProjectDetails> {
        if name.trim().is_empty() {
            return Err(JcimError::Unsupported(
                "project name must not be empty".to_string(),
            ));
        }

        let project_root = if directory.is_absolute() {
            directory.to_path_buf()
        } else {
            std::env::current_dir()?.join(directory)
        };
        std::fs::create_dir_all(&project_root)?;
        let manifest_path = project_root.join(PROJECT_MANIFEST_NAME);
        if manifest_path.exists() {
            return Err(JcimError::Unsupported(format!(
                "project manifest already exists at {}",
                manifest_path.display()
            )));
        }

        let config = ProjectConfig::default_for_project_name(name);
        std::fs::write(&manifest_path, config.to_pretty_toml()?)?;
        self.write_sample_applet(&project_root, &config)?;

        let resolved = self.load_project_by_root(&project_root)?;
        Ok(ProjectDetails {
            project: self.project_summary(&resolved),
            manifest_toml: resolved.manifest_toml,
        })
    }

    /// Load one project and return its current manifest contents.
    pub fn get_project(&self, selector: &ProjectSelectorInput) -> Result<ProjectDetails> {
        let resolved = self.resolve_project(selector)?;
        Ok(ProjectDetails {
            project: self.project_summary(&resolved),
            manifest_toml: resolved.manifest_toml,
        })
    }

    /// Clean the project-local generated build directory.
    pub fn clean_project(&self, selector: &ProjectSelectorInput) -> Result<PathBuf> {
        let resolved = self.resolve_project(selector)?;
        let build_root = resolved.project_root.join(".jcim");
        if build_root.exists() {
            std::fs::remove_dir_all(&build_root)?;
        }
        Ok(build_root)
    }

    pub(super) fn project_summary(&self, resolved: &ResolvedProject) -> ProjectSummary {
        ProjectSummary {
            project_id: resolved.project_id.clone(),
            name: resolved.config.metadata.name.clone(),
            project_path: resolved.project_root.clone(),
            profile: resolved.config.metadata.profile.to_string(),
            build_kind: match resolved.config.build.kind {
                BuildKind::Native => "native".to_string(),
                BuildKind::Command => "command".to_string(),
            },
            package_name: resolved.config.metadata.package_name.clone(),
            package_aid: resolved.config.metadata.package_aid.to_hex(),
            applets: resolved
                .config
                .metadata
                .applets
                .iter()
                .map(|applet| AppletSummary {
                    class_name: applet.class_name.clone(),
                    aid: applet.aid.to_hex(),
                })
                .collect(),
        }
    }

    fn write_sample_applet(&self, project_root: &Path, config: &ProjectConfig) -> Result<()> {
        let applet = config.metadata.applets.first().ok_or_else(|| {
            JcimError::Unsupported("starter project is missing a default applet".to_string())
        })?;
        let (package_name, class_name) = split_class_name(&applet.class_name)?;
        let source_root = resolve_project_path(project_root, &config.source_root());
        let package_dir = if package_name.is_empty() {
            source_root.clone()
        } else {
            source_root.join(package_name.replace('.', "/"))
        };
        std::fs::create_dir_all(&package_dir)?;
        let source_path = package_dir.join(format!("{class_name}.java"));
        std::fs::write(
            source_path,
            sample_applet_source(&package_name, &class_name),
        )?;
        Ok(())
    }
}
