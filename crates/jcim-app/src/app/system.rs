use super::*;

impl JcimApp {
    /// Persist machine-local toolchain settings.
    pub fn setup_toolchains(&self, java_bin: Option<&str>) -> Result<SetupSummary> {
        self.state.persist_user_config(|user_config| {
            if let Some(java_bin) = java_bin {
                user_config.java_bin = java_bin.to_string();
            }
            if user_config.bundle_root.is_none() {
                user_config.bundle_root = Some(default_bundle_root());
            }
        })?;
        Ok(SetupSummary {
            config_path: self.state.managed_paths.config_path.clone(),
            message: format!(
                "saved machine-local JCIM settings to {}",
                self.state.managed_paths.config_path.display()
            ),
        })
    }

    /// Return a human-facing doctor report for the local service environment.
    pub fn doctor(&self) -> Result<Vec<String>> {
        let user_config = self.state.user_config_snapshot()?;
        let java_runtime = self.resolved_java_runtime()?;
        let java_source = match java_runtime.source {
            JavaRuntimeSource::Bundled => "bundled",
            JavaRuntimeSource::Configured => "configured",
        };
        Ok(vec![
            format!(
                "Managed data root: {}",
                self.state.managed_paths.root.display()
            ),
            format!(
                "Managed config dir: {}",
                self.state.managed_paths.config_dir.display()
            ),
            format!(
                "Managed state dir: {}",
                self.state.managed_paths.state_dir.display()
            ),
            format!(
                "Managed runtime dir: {}",
                self.state.managed_paths.runtime_dir.display()
            ),
            format!(
                "Managed cache dir: {}",
                self.state.managed_paths.cache_dir.display()
            ),
            format!(
                "Managed log dir: {}",
                self.state.managed_paths.log_dir.display()
            ),
            format!(
                "Config path: {}",
                self.state.managed_paths.config_path.display()
            ),
            format!(
                "Registry path: {}",
                self.state.managed_paths.registry_path.display()
            ),
            format!(
                "Service socket: {}",
                self.state.managed_paths.service_socket_path.display()
            ),
            format!(
                "Service runtime metadata: {}",
                self.state.managed_paths.runtime_metadata_path.display()
            ),
            format!(
                "Managed runtime asset root: {}",
                self.state.managed_paths.bundle_root.display()
            ),
            format!("Configured Java bin: {}", user_config.java_bin),
            format!(
                "Effective Java runtime: {} ({java_source})",
                java_runtime.java_bin.display()
            ),
            format!(
                "Simulator bundle root: {}",
                user_config
                    .bundle_root
                    .unwrap_or_else(default_bundle_root)
                    .display()
            ),
            format!("Card helper jar: {}", helper_jar_path().display()),
            format!("GPPro jar: {}", gppro_jar_path().display()),
        ])
    }

    /// Return service status for the current in-process instance.
    pub fn service_status(&self) -> Result<ServiceStatusSummary> {
        Ok(ServiceStatusSummary {
            socket_path: self.state.managed_paths.service_socket_path.clone(),
            running: true,
            known_project_count: self.list_projects()?.len() as u32,
            active_simulation_count: self.active_simulation_count(),
            service_binary_path: self.state.service_binary_path.clone(),
            service_binary_fingerprint: self.state.service_binary_fingerprint.clone(),
        })
    }

    pub(super) fn resolved_java_runtime(&self) -> Result<ResolvedJavaRuntime> {
        let user_config = self.state.user_config_snapshot()?;
        resolve_java_runtime(&self.state.managed_paths.bundle_root, &user_config.java_bin)
    }
}
