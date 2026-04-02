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

    /// Convert one resolved project record into the stable summary returned by app services.
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

    /// Materialize the starter Java Card applet source file for a new project skeleton.
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

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use jcim_config::project::PROJECT_MANIFEST_NAME;

    use super::*;
    use crate::app::testsupport::{load_test_app, temp_root};
    use crate::registry::normalize_project_root;

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn create_project_rejects_empty_names_and_duplicate_manifests() {
        let root = temp_root("projects-reject");
        let app = load_test_app(&root);
        let project_root = root.join("demo");

        let empty_name = app
            .create_project("   ", &project_root)
            .expect_err("empty project names should fail");
        assert!(empty_name.to_string().contains("must not be empty"));

        app.create_project("Demo", &project_root)
            .expect("create initial project");
        let duplicate = app
            .create_project("Demo", &project_root)
            .expect_err("existing manifests should fail");
        assert!(
            duplicate
                .to_string()
                .contains("project manifest already exists")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn create_project_normalizes_relative_directories_and_writes_sample_applet() {
        let _cwd_guard = cwd_lock().lock().expect("lock cwd");
        let root = temp_root("projects-relative");
        let app = load_test_app(&root);
        std::fs::create_dir_all(&root).expect("create cwd root");
        let previous_cwd = std::env::current_dir().expect("current dir");
        std::env::set_current_dir(&root).expect("set current dir");

        let created = app
            .create_project("Demo App", Path::new("relative-demo"))
            .expect("create project from relative path");
        let expected_root =
            normalize_project_root(&root.join("relative-demo")).expect("normalize root");

        std::env::set_current_dir(previous_cwd).expect("restore current dir");

        assert_eq!(created.project.project_path, expected_root);
        assert!(
            created
                .project
                .project_path
                .join(PROJECT_MANIFEST_NAME)
                .exists()
        );
        let sample_source = created
            .project
            .project_path
            .join("src/main/javacard/com/jcim/demoapp/DemoAppApplet.java");
        let source = std::fs::read_to_string(&sample_source).expect("read starter applet");
        assert!(source.contains("package com.jcim.demoapp;"));
        assert!(source.contains("public final class DemoAppApplet extends Applet"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_projects_is_sorted_and_skips_stale_registry_entries() {
        let root = temp_root("projects-list");
        let app = load_test_app(&root);
        let later = root.join("zeta");
        let earlier = root.join("alpha");

        app.create_project("Zeta", &later).expect("create zeta");
        app.create_project("Alpha", &earlier).expect("create alpha");
        std::fs::remove_dir_all(&later).expect("remove stale project");

        let listed = app.list_projects().expect("list projects");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Alpha");
        assert_eq!(
            listed[0].project_path,
            normalize_project_root(&earlier).expect("normalize root")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn clean_project_is_idempotent_when_generated_state_is_absent() {
        let root = temp_root("projects-clean");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");

        let cleaned = app
            .clean_project(&crate::app::testsupport::project_selector(&project_root))
            .expect("clean project");
        assert_eq!(
            cleaned,
            normalize_project_root(&project_root)
                .expect("normalize root")
                .join(".jcim")
        );
        assert!(!cleaned.exists());

        let _ = std::fs::remove_dir_all(root);
    }
}
