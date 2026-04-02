use std::sync::atomic::Ordering;

use super::*;
use crate::app::BackendHealthStatusExt;
use crate::app::simulations::runtime::events::{
    annotate_simulation_start_error, starting_simulation_record,
};

impl JcimApp {
    /// Start one simulation from a JCIM project.
    pub async fn start_project_simulation(
        &self,
        selector: &ProjectSelectorInput,
    ) -> Result<SimulationSummary> {
        let prepared = self.prepare_project_simulation(selector)?;
        let reset_after_start = prepared.runtime_config.reader_name.is_some()
            && self
                .resolve_project(selector)
                .map(|resolved| resolved.config.simulator.reset_after_start)
                .unwrap_or(false);
        self.start_prepared_simulation(prepared, reset_after_start)
            .await
    }

    /// Resolve a project into the runtime configuration needed for one simulation launch.
    fn prepare_project_simulation(
        &self,
        selector: &ProjectSelectorInput,
    ) -> Result<PreparedSimulation> {
        let resolved = self.resolve_project(selector)?;
        let simulation_id = self.next_simulation_id();
        let build_metadata =
            self.resolve_simulation_artifacts(&resolved.project_root, &resolved.config)?;
        let cap_path = required_artifact_path(
            &resolved.project_root,
            build_metadata.cap_path.as_ref(),
            "project build did not emit a CAP artifact required for simulation",
        )?;
        let mut runtime_config = self.runtime_config_for_simulation(
            resolved.config.metadata.profile,
            Some(resolved.config.metadata.name.clone()),
            cap_path.clone(),
            resolved.project_root.join(&build_metadata.classes_path),
            build_metadata
                .runtime_classpath
                .iter()
                .map(|path| resolved.project_root.join(path))
                .collect(),
            resolved
                .project_root
                .join(&build_metadata.simulator_metadata_path),
        )?;
        runtime_config.backend.kind = BackendKind::Simulator;
        Ok(PreparedSimulation {
            summary: SimulationSummary {
                simulation_id,
                project_id: resolved.project_id,
                project_path: resolved.project_root,
                status: SimulationStatusKind::Starting,
                reader_name: runtime_config
                    .reader_name
                    .clone()
                    .unwrap_or_else(|| "JCIM Simulation".to_string()),
                health: "starting".to_string(),
                atr: None,
                active_protocol: None,
                iso_capabilities: IsoCapabilities::default(),
                session_state: IsoSessionState::default(),
                package_count: 0,
                applet_count: 0,
                package_name: build_metadata.package_name,
                package_aid: build_metadata.package_aid.to_hex(),
                recent_events: vec!["info: simulation prepared from project".to_string()],
            },
            runtime_config,
        })
    }

    /// Reserve state, perform backend startup, and commit the resulting running or failed record.
    async fn start_prepared_simulation(
        &self,
        prepared: PreparedSimulation,
        reset_after_start: bool,
    ) -> Result<SimulationSummary> {
        // Reserve the simulation record first, perform async backend startup without a held guard,
        // then commit either the running record or a retained failed state in one short update.
        self.state
            .store_simulation(starting_simulation_record(&prepared))?;

        match self
            .run_prepared_simulation_start(&prepared, reset_after_start)
            .await
        {
            Ok(record) => self.state.store_simulation(record),
            Err(error) => {
                let message = format!("simulation start failed: {error}");
                let _ = self.state.update_simulation_status(
                    &prepared.summary.simulation_id,
                    SimulationStatusKind::Failed,
                    format!("failed: {error}"),
                    "error",
                    message,
                    None,
                );
                Err(error)
            }
        }
    }

    /// Execute the backend startup handshake and convert its snapshot into a running record.
    async fn run_prepared_simulation_start(
        &self,
        prepared: &PreparedSimulation,
        reset_after_start: bool,
    ) -> Result<SimulationRecord> {
        let bundle_dir = prepared.runtime_config.backend_bundle_dir();
        ensure_host_simulator_environment(&bundle_dir, prepared.runtime_config.profile_id)
            .map_err(|error| annotate_simulation_start_error(prepared, error))?;

        let handle = BackendHandle::from_config(prepared.runtime_config.clone())
            .map_err(|error| annotate_simulation_start_error(prepared, error))?;

        let startup = async {
            let handshake = handle.handshake(ProtocolVersion::current()).await?;
            if reset_after_start {
                let _ = handle.reset().await?;
            }
            let health = handle.backend_health().await?;
            let snapshot = handle.snapshot().await?;
            let packages = handle.list_packages().await.unwrap_or_default();
            let applets = handle.list_applets().await.unwrap_or_default();
            Ok::<_, JcimError>((handshake, health, snapshot, packages, applets))
        }
        .await;

        let (handshake, health, snapshot, packages, applets) = match startup {
            Ok(startup) => startup,
            Err(error) => {
                let _ = handle.shutdown().await;
                return Err(annotate_simulation_start_error(prepared, error));
            }
        };

        let atr = snapshot
            .session_state
            .atr
            .clone()
            .or_else(|| Atr::parse(&snapshot.atr).ok());
        let active_protocol = snapshot
            .session_state
            .active_protocol
            .clone()
            .or_else(|| atr.as_ref().map(ProtocolParameters::from_atr));
        let prior_events = self
            .state
            .with_simulation(&prepared.summary.simulation_id, |simulation| {
                simulation.recent_events.clone()
            })
            .unwrap_or_default();

        let mut record = SimulationRecord {
            simulation_id: prepared.summary.simulation_id.clone(),
            project_id: prepared.summary.project_id.clone(),
            project_path: prepared.summary.project_path.clone(),
            status: SimulationStatusKind::Running,
            reader_name: handshake.reader_name,
            health: format!("{} ({})", health.message, health.status.status_string()),
            atr,
            active_protocol,
            iso_capabilities: snapshot.iso_capabilities.clone(),
            session_state: snapshot.session_state,
            package_count: packages.len() as u32,
            applet_count: applets.len() as u32,
            package_name: packages
                .first()
                .map(|package| package.package_name.clone())
                .unwrap_or_else(|| prepared.summary.package_name.clone()),
            package_aid: packages
                .first()
                .map(|package| package.package_aid.to_hex())
                .unwrap_or_else(|| prepared.summary.package_aid.clone()),
            recent_events: prior_events,
            handle: Some(handle),
        };
        remember_event(
            &mut record.recent_events,
            "info",
            format!("simulation started for project {}", record.project_id),
        );
        Ok(record)
    }

    /// Assemble the runtime config passed to the managed simulator backend.
    fn runtime_config_for_simulation(
        &self,
        profile_id: jcim_core::model::CardProfileId,
        reader_name: Option<String>,
        cap_path: PathBuf,
        classes_path: PathBuf,
        runtime_classpath: Vec<PathBuf>,
        simulator_metadata_path: PathBuf,
    ) -> Result<RuntimeConfig> {
        let user_config = self.effective_user_config()?;
        let mut runtime_config = RuntimeConfig {
            profile_id,
            cap_path: Some(cap_path),
            classes_path: Some(classes_path),
            runtime_classpath,
            simulator_metadata_path: Some(simulator_metadata_path),
            reader_name,
            ..RuntimeConfig::default()
        };
        runtime_config.backend.java_bin = user_config.java_bin;
        runtime_config.backend.bundle_root =
            user_config.bundle_root.unwrap_or_else(default_bundle_root);
        Ok(runtime_config)
    }

    /// Resolve or build the artifacts required to start a simulation for one project.
    fn resolve_simulation_artifacts(
        &self,
        project_root: &Path,
        config: &ProjectConfig,
    ) -> Result<ArtifactMetadata> {
        if !config.simulator.auto_build {
            let metadata = load_artifact_metadata(project_root)?.ok_or_else(|| {
                JcimError::Unsupported(
                    "this project disables automatic simulator builds and has no recorded artifacts; run `jcim build` first".to_string(),
                )
            })?;
            return validate_simulation_artifacts(project_root, metadata);
        }

        let request = artifact_metadata_from_project(project_root, config)?;
        let toolchain = build_toolchain_layout()?;
        let java_runtime = self.resolved_java_runtime()?;
        let outcome = build_project_artifacts_if_stale_with_java_bin(
            &request,
            &toolchain,
            &java_runtime.java_bin,
        )?;
        validate_simulation_artifacts(project_root, outcome.metadata)
    }

    /// Allocate the next locally unique simulation id from the app state counter.
    fn next_simulation_id(&self) -> String {
        let id = self
            .state
            .next_simulation_id
            .fetch_add(1, Ordering::Relaxed);
        format!("sim-{id:016x}")
    }
}

#[cfg(test)]
mod tests {
    use jcim_build::{ArtifactMetadata, BuildAppletMetadata};
    use jcim_config::project::BuildKind;

    use super::*;
    use crate::app::testsupport::{
        acquire_local_service_lock, load_test_app, project_selector, read_project_config,
        simulation_selector, temp_root, write_project_config,
    };

    #[test]
    fn resolve_simulation_artifacts_uses_cached_metadata_when_auto_build_is_disabled() {
        let _service_lock = acquire_local_service_lock();
        let root = temp_root("sim-artifacts-cache");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");
        let selector = project_selector(&project_root);
        let _ = app.build_project(&selector).expect("build project");

        let mut config = read_project_config(&project_root);
        config.simulator.auto_build = false;
        write_project_config(&project_root, &config);

        let resolved = app.resolve_project(&selector).expect("resolve project");
        let metadata = app
            .resolve_simulation_artifacts(&resolved.project_root, &resolved.config)
            .expect("resolve cached metadata");

        assert!(metadata.cap_path.is_some());
        assert!(project_root.join(&metadata.classes_path).exists());
        assert!(
            project_root
                .join(&metadata.simulator_metadata_path)
                .exists()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_simulation_artifacts_fails_closed_when_required_files_are_missing() {
        let project_root = temp_root("sim-artifacts-missing");
        std::fs::create_dir_all(project_root.join("build/classes")).expect("create classes dir");
        std::fs::create_dir_all(project_root.join("deps")).expect("create deps dir");
        std::fs::write(project_root.join("build/demo.cap"), []).expect("write cap");
        std::fs::write(project_root.join("build/simulator.json"), "{}").expect("write metadata");
        std::fs::write(project_root.join("deps/runtime.jar"), []).expect("write runtime");

        let metadata = ArtifactMetadata {
            build_kind: BuildKind::Native,
            profile: CardProfileId::Classic304,
            package_name: "com.jcim.demo".to_string(),
            package_aid: Aid::from_slice(&[0xF0, 0x00, 0x00, 0x00, 0x01]).expect("aid"),
            version: "1.0".to_string(),
            applets: vec![BuildAppletMetadata {
                class_name: "com.jcim.demo.DemoApplet".to_string(),
                aid: Aid::from_slice(&[0xF0, 0x00, 0x00, 0x00, 0x01, 0x01]).expect("aid"),
            }],
            cap_path: Some(PathBuf::from("build/demo.cap")),
            classes_path: PathBuf::from("build/classes"),
            runtime_classpath: vec![PathBuf::from("deps/runtime.jar")],
            simulator_metadata_path: PathBuf::from("build/simulator.json"),
            source_fingerprint: "fingerprint".to_string(),
        };

        validate_simulation_artifacts(&project_root, metadata.clone()).expect("valid metadata");

        std::fs::remove_file(project_root.join("build/demo.cap")).expect("remove cap");
        let cap_error = validate_simulation_artifacts(&project_root, metadata.clone())
            .expect_err("missing cap should fail");
        assert!(
            cap_error
                .to_string()
                .contains("expected CAP artifact is missing")
        );

        std::fs::write(project_root.join("build/demo.cap"), []).expect("restore cap");
        std::fs::remove_dir_all(project_root.join("build/classes")).expect("remove classes");
        let classes_error = validate_simulation_artifacts(&project_root, metadata.clone())
            .expect_err("missing classes should fail");
        assert!(
            classes_error
                .to_string()
                .contains("expected compiled classes are missing")
        );

        std::fs::create_dir_all(project_root.join("build/classes")).expect("restore classes");
        std::fs::remove_file(project_root.join("deps/runtime.jar")).expect("remove runtime jar");
        let runtime_error = validate_simulation_artifacts(&project_root, metadata)
            .expect_err("missing runtime dependency should fail");
        assert!(
            runtime_error
                .to_string()
                .contains("expected simulator runtime classpath entry is missing")
        );

        let _ = std::fs::remove_dir_all(project_root);
    }

    #[tokio::test]
    async fn start_project_simulation_runs_with_reset_after_start_enabled() {
        let _service_lock = acquire_local_service_lock();
        let root = temp_root("sim-reset-after-start");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");
        let selector = project_selector(&project_root);

        let mut config = read_project_config(&project_root);
        config.simulator.reset_after_start = true;
        write_project_config(&project_root, &config);

        let simulation = app
            .start_project_simulation(&selector)
            .await
            .expect("start simulation");
        let events = app
            .simulation_events(&simulation_selector(simulation.simulation_id.clone()))
            .expect("simulation events");

        assert_eq!(simulation.status, SimulationStatusKind::Running);
        assert!(
            events
                .iter()
                .any(|event| event.message.contains("simulation started"))
        );

        let _ = app
            .stop_simulation(&simulation_selector(simulation.simulation_id))
            .await;
        let _ = std::fs::remove_dir_all(root);
    }
}
