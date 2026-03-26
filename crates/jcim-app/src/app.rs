//! Local JCIM application services.
#![allow(clippy::missing_docs_in_private_items)]
// This module is the dense internal orchestration layer for the transport-neutral app service.
// We keep the public façade documented and avoid line-by-line docs on private glue code here.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_backends::backend::BackendHandle;
use jcim_build::{
    ArtifactMetadata, artifact_metadata_from_project,
    build_project_artifacts_if_stale_with_java_bin, build_toolchain_layout, load_artifact_metadata,
};
use jcim_cap::prelude::CapPackage;
use jcim_config::config::RuntimeConfig;
use jcim_config::project::{
    BuildKind, ManagedPaths, PROJECT_MANIFEST_NAME, ProjectConfig, UserConfig,
    find_project_manifest, resolve_project_path,
};
use jcim_core::aid::Aid;
use jcim_core::apdu::CommandApdu;
use jcim_core::error::{JcimError, Result};
use jcim_core::globalplatform;
use jcim_core::iso7816;
use jcim_core::iso7816::{
    Atr, IsoCapabilities, IsoSessionState, ProtocolParameters, SecureMessagingProtocol,
    SecureMessagingState, apply_response_to_session,
};
use jcim_core::model::{BackendHealthStatus, BackendKind, CardProfileId, ProtocolVersion};

use crate::card::{
    JavaPhysicalCardAdapter, PhysicalCardAdapter, ResolvedGpKeyset, gppro_jar_path, helper_jar_path,
};
use crate::java_runtime::{JavaRuntimeSource, ResolvedJavaRuntime, resolve_java_runtime};
use crate::model::{
    ApduExchangeSummary, AppletSummary, ArtifactSummary, CardAppletInventory, CardDeleteSummary,
    CardInstallSummary, CardPackageInventory, CardReaderSummary, CardStatusSummary, EventLine,
    GpSecureChannelSummary, ManageChannelSummary, OverviewSummary, ProjectDetails,
    ProjectSelectorInput, ProjectSummary, ResetSummary, SecureMessagingSummary,
    ServiceStatusSummary, SetupSummary, SimulationEngineMode, SimulationSelectorInput,
    SimulationSourceKind, SimulationStatusKind, SimulationSummary,
};
use crate::registry::{ProjectRegistry, normalize_project_root};

const EVENT_LIMIT: usize = 32;

/// Transport-neutral application façade for the JCIM 0.2 local platform.
#[derive(Clone)]
pub struct JcimApp {
    /// Shared mutable application state.
    state: Arc<AppState>,
}

struct AppState {
    managed_paths: ManagedPaths,
    service_binary_path: PathBuf,
    service_binary_fingerprint: String,
    user_config: RwLock<UserConfig>,
    registry: RwLock<ProjectRegistry>,
    simulations: Mutex<HashMap<String, SimulationRecord>>,
    build_events: Mutex<HashMap<String, VecDeque<EventLine>>>,
    card_sessions: Mutex<HashMap<String, CardSessionRecord>>,
    card_adapter: Arc<dyn PhysicalCardAdapter>,
    next_simulation_id: AtomicU64,
}

struct SimulationRecord {
    simulation_id: String,
    source_kind: SimulationSourceKind,
    project_id: Option<String>,
    project_path: Option<PathBuf>,
    cap_path: PathBuf,
    engine_mode: SimulationEngineMode,
    status: SimulationStatusKind,
    reader_name: String,
    health: String,
    atr: Option<Atr>,
    active_protocol: Option<ProtocolParameters>,
    iso_capabilities: IsoCapabilities,
    session_state: IsoSessionState,
    package_count: u32,
    applet_count: u32,
    package_name: String,
    package_aid: String,
    recent_events: VecDeque<EventLine>,
    handle: Option<BackendHandle>,
}

struct ResolvedProject {
    project_id: String,
    project_root: PathBuf,
    manifest_toml: String,
    config: ProjectConfig,
}

struct CardSessionRecord {
    session_state: IsoSessionState,
    gp_secure_channel: Option<globalplatform::EstablishedSecureChannel>,
}

struct PreparedSimulation {
    summary: SimulationSummary,
    runtime_config: RuntimeConfig,
}

impl JcimApp {
    /// Load the local application state from the managed JCIM directories.
    pub fn load() -> Result<Self> {
        Self::load_with_paths(ManagedPaths::discover()?)
    }

    /// Load the application state using an explicit managed root layout.
    pub fn load_with_paths(managed_paths: ManagedPaths) -> Result<Self> {
        Self::load_with_paths_and_card_adapter(managed_paths, Arc::new(JavaPhysicalCardAdapter))
    }

    /// Load the application state using an explicit managed root layout and card adapter.
    pub fn load_with_paths_and_card_adapter(
        managed_paths: ManagedPaths,
        card_adapter: Arc<dyn PhysicalCardAdapter>,
    ) -> Result<Self> {
        std::fs::create_dir_all(&managed_paths.root)?;
        std::fs::create_dir_all(managed_paths.service_socket_path.parent().ok_or_else(|| {
            JcimError::Unsupported(
                "managed service socket path has no parent directory".to_string(),
            )
        })?)?;
        std::fs::create_dir_all(&managed_paths.log_dir)?;
        std::fs::create_dir_all(&managed_paths.bundle_root)?;

        let user_config = UserConfig::load_or_default(&managed_paths.config_path)?;
        let registry = ProjectRegistry::load_or_default(&managed_paths.registry_path)?;
        let (service_binary_path, service_binary_fingerprint) = current_service_binary_identity()?;
        let next_simulation_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            state: Arc::new(AppState {
                managed_paths,
                service_binary_path,
                service_binary_fingerprint,
                user_config: RwLock::new(user_config),
                registry: RwLock::new(registry),
                simulations: Mutex::new(HashMap::new()),
                build_events: Mutex::new(HashMap::new()),
                card_sessions: Mutex::new(HashMap::new()),
                card_adapter,
                next_simulation_id: AtomicU64::new(next_simulation_id),
            }),
        })
    }

    /// Return the managed machine-local paths used by this application instance.
    pub fn managed_paths(&self) -> &ManagedPaths {
        &self.state.managed_paths
    }

    /// Return a high-level overview of the managed project and simulation state.
    pub fn overview(&self) -> Result<OverviewSummary> {
        Ok(OverviewSummary {
            known_project_count: self.list_projects()?.len() as u32,
            active_simulation_count: self.active_simulation_count(),
        })
    }

    /// Return the registered project list with current manifest metadata.
    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let entries = self
            .state
            .registry
            .read()
            .map_err(lock_poisoned)?
            .projects
            .clone();
        let mut projects = Vec::new();
        for entry in entries {
            if let Ok(resolved) = self.load_project_by_root(&entry.project_path) {
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
        let events = self
            .state
            .build_events
            .lock()
            .map_err(lock_poisoned)?
            .get(&resolved.project_id)
            .cloned()
            .unwrap_or_default();
        Ok(events.into_iter().collect())
    }

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

    /// Return the current simulation list.
    pub fn list_simulations(&self) -> Result<Vec<SimulationSummary>> {
        let simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
        let mut values = simulations
            .values()
            .map(SimulationRecord::summary)
            .collect::<Vec<_>>();
        values.sort_by(|left, right| left.simulation_id.cmp(&right.simulation_id));
        Ok(values)
    }

    /// Return one managed simulation by id.
    pub fn get_simulation(&self, selector: &SimulationSelectorInput) -> Result<SimulationSummary> {
        let simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations.get(&selector.simulation_id).ok_or_else(|| {
            JcimError::Unsupported(format!(
                "unknown simulation id `{}`",
                selector.simulation_id
            ))
        })?;
        Ok(simulation.summary())
    }

    /// Return retained simulation events for one running simulation.
    pub fn simulation_events(&self, selector: &SimulationSelectorInput) -> Result<Vec<EventLine>> {
        let simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations.get(&selector.simulation_id).ok_or_else(|| {
            JcimError::Unsupported(format!(
                "unknown simulation id `{}`",
                selector.simulation_id
            ))
        })?;
        Ok(simulation.recent_events.iter().cloned().collect())
    }

    /// Stop one managed simulation.
    pub async fn stop_simulation(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<SimulationSummary> {
        let handle = {
            let simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
            let simulation = simulations.get(&selector.simulation_id).ok_or_else(|| {
                JcimError::Unsupported(format!(
                    "unknown simulation id `{}`",
                    selector.simulation_id
                ))
            })?;
            simulation.handle.clone()
        };

        if let Some(handle) = handle {
            let _ = handle.shutdown().await;
        }

        let mut simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations
            .get_mut(&selector.simulation_id)
            .ok_or_else(|| {
                JcimError::Unsupported(format!(
                    "unknown simulation id `{}`",
                    selector.simulation_id
                ))
            })?;
        simulation.handle = None;
        simulation.status = SimulationStatusKind::Stopped;
        simulation.health = "stopped".to_string();
        remember_event(&mut simulation.recent_events, "info", "simulation stopped");
        Ok(simulation.summary())
    }

    /// Send one APDU to the selected simulation.
    pub async fn transmit_apdu(
        &self,
        selector: &SimulationSelectorInput,
        apdu_hex: &str,
    ) -> Result<String> {
        let command = CommandApdu::parse(&hex::decode(apdu_hex)?)?;
        let exchange = self.transmit_command(selector, &command).await?;
        Ok(hex::encode_upper(exchange.response.to_bytes()))
    }

    /// Send one typed command APDU to the selected simulation.
    pub async fn transmit_command(
        &self,
        selector: &SimulationSelectorInput,
        command: &CommandApdu,
    ) -> Result<ApduExchangeSummary> {
        let handle = self.simulation_handle(selector)?;
        let exchange = handle.transmit_typed_apdu(command.clone()).await?;
        let response = exchange.response;
        let session_state = exchange.session_state;
        if let Ok(mut simulations) = self.state.simulations.lock()
            && let Some(simulation) = simulations.get_mut(&selector.simulation_id)
        {
            apply_authoritative_simulation_session(simulation, &session_state);
            remember_event(
                &mut simulation.recent_events,
                "info",
                format!(
                    "apdu exchange {}",
                    truncate_hex(&hex::encode_upper(command.to_bytes()))
                ),
            );
        }
        Ok(ApduExchangeSummary {
            command: command.clone(),
            response,
            session_state,
        })
    }

    /// Return the current tracked ISO/IEC 7816 session state for one simulation.
    pub fn simulation_session_state(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<IsoSessionState> {
        let simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations.get(&selector.simulation_id).ok_or_else(|| {
            JcimError::Unsupported(format!(
                "unknown simulation id `{}`",
                selector.simulation_id
            ))
        })?;
        Ok(simulation.session_state.clone())
    }

    /// Open or close one logical channel on a running simulation.
    pub async fn manage_simulation_channel(
        &self,
        selector: &SimulationSelectorInput,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<ManageChannelSummary> {
        let handle = self.simulation_handle(selector)?;
        let exchange = handle.manage_channel(open, channel_number).await?;
        let channel_number = if open {
            exchange.response.data.first().copied().or(channel_number)
        } else {
            channel_number
        };
        if let Ok(mut simulations) = self.state.simulations.lock()
            && let Some(simulation) = simulations.get_mut(&selector.simulation_id)
        {
            apply_authoritative_simulation_session(simulation, &exchange.session_state);
            remember_event(
                &mut simulation.recent_events,
                "info",
                if open {
                    format!(
                        "opened logical channel {}",
                        channel_number.map_or_else(|| "?".to_string(), |value| value.to_string())
                    )
                } else {
                    format!(
                        "closed logical channel {}",
                        channel_number.map_or_else(|| "?".to_string(), |value| value.to_string())
                    )
                },
            );
        }
        Ok(ManageChannelSummary {
            channel_number,
            response: exchange.response,
            session_state: exchange.session_state,
        })
    }

    /// Mark one secure-messaging session as open for one running simulation.
    pub async fn open_simulation_secure_messaging(
        &self,
        selector: &SimulationSelectorInput,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<SecureMessagingSummary> {
        let handle = self.simulation_handle(selector)?;
        let summary = handle
            .open_secure_messaging(protocol, security_level, session_id)
            .await?;
        let mut simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations
            .get_mut(&selector.simulation_id)
            .ok_or_else(|| {
                JcimError::Unsupported(format!(
                    "unknown simulation id `{}`",
                    selector.simulation_id
                ))
            })?;
        apply_authoritative_simulation_session(simulation, &summary.session_state);
        remember_event(
            &mut simulation.recent_events,
            "info",
            "simulation secure messaging opened",
        );
        Ok(SecureMessagingSummary {
            session_state: summary.session_state,
        })
    }

    /// Advance the tracked secure-messaging command counter for one simulation.
    pub async fn advance_simulation_secure_messaging(
        &self,
        selector: &SimulationSelectorInput,
        increment_by: u32,
    ) -> Result<SecureMessagingSummary> {
        let handle = self.simulation_handle(selector)?;
        let summary = handle.advance_secure_messaging(increment_by).await?;
        let mut simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations
            .get_mut(&selector.simulation_id)
            .ok_or_else(|| {
                JcimError::Unsupported(format!(
                    "unknown simulation id `{}`",
                    selector.simulation_id
                ))
            })?;
        apply_authoritative_simulation_session(simulation, &summary.session_state);
        remember_event(
            &mut simulation.recent_events,
            "info",
            "simulation secure messaging advanced",
        );
        Ok(SecureMessagingSummary {
            session_state: summary.session_state,
        })
    }

    /// Close the tracked secure-messaging session for one simulation.
    pub async fn close_simulation_secure_messaging(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<SecureMessagingSummary> {
        let handle = self.simulation_handle(selector)?;
        let summary = handle.close_secure_messaging().await?;
        let mut simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations
            .get_mut(&selector.simulation_id)
            .ok_or_else(|| {
                JcimError::Unsupported(format!(
                    "unknown simulation id `{}`",
                    selector.simulation_id
                ))
            })?;
        apply_authoritative_simulation_session(simulation, &summary.session_state);
        remember_event(
            &mut simulation.recent_events,
            "info",
            "simulation secure messaging closed",
        );
        Ok(SecureMessagingSummary {
            session_state: summary.session_state,
        })
    }

    /// Open one typed GP secure channel on a running simulation.
    pub async fn open_gp_secure_channel_on_simulation(
        &self,
        selector: &SimulationSelectorInput,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
    ) -> Result<GpSecureChannelSummary> {
        let keyset = ResolvedGpKeyset::resolve(keyset_name)?;
        let security_level = gp_security_level(security_level.unwrap_or(0x01));
        let selected_aid = Aid::from_slice(&globalplatform::ISSUER_SECURITY_DOMAIN_AID)?;
        self.transmit_command(selector, &globalplatform::select_issuer_security_domain())
            .await?;
        let host_challenge = gp_host_challenge();
        let initialize_update = self
            .transmit_command(selector, &globalplatform::initialize_update(host_challenge))
            .await?;
        let initialize_update =
            globalplatform::parse_initialize_update(keyset.mode, &initialize_update.response)?;
        let derived = globalplatform::derive_session_context(
            keyset.metadata(),
            security_level,
            host_challenge,
            initialize_update,
        );
        let secure_channel = globalplatform::establish_secure_channel(
            &derived,
            format!("sim-gp-{}", selector.simulation_id),
        );
        self.transmit_command(
            selector,
            &globalplatform::external_authenticate(security_level, [0x00; 8]),
        )
        .await?;
        let summary = self
            .open_simulation_secure_messaging(
                selector,
                Some(keyset.protocol()),
                Some(security_level.as_byte()),
                Some(secure_channel.session_id.clone()),
            )
            .await?;
        Ok(GpSecureChannelSummary {
            secure_channel,
            selected_aid,
            session_state: summary.session_state,
        })
    }

    /// Close one typed GP secure channel on a running simulation.
    pub async fn close_gp_secure_channel_on_simulation(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<SecureMessagingSummary> {
        self.close_simulation_secure_messaging(selector).await
    }

    /// Reset the selected simulation and return the current ATR.
    pub async fn reset_simulation_summary(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<ResetSummary> {
        let handle = self.simulation_handle(selector)?;
        let reset = handle.reset().await?;
        let parsed_atr = reset
            .atr
            .clone()
            .or_else(|| reset.session_state.atr.clone());
        let session_state = reset.session_state;
        if let Ok(mut simulations) = self.state.simulations.lock()
            && let Some(simulation) = simulations.get_mut(&selector.simulation_id)
        {
            apply_authoritative_simulation_session(simulation, &session_state);
            remember_event(&mut simulation.recent_events, "info", "simulation reset");
        }
        Ok(ResetSummary {
            atr: parsed_atr,
            session_state,
        })
    }

    /// List physical PC/SC readers.
    pub async fn list_readers(&self) -> Result<Vec<CardReaderSummary>> {
        let user_config = self.effective_user_config()?;
        self.state.card_adapter.list_readers(&user_config).await
    }

    /// Return physical-card status for one reader.
    pub async fn card_status(&self, reader_name: Option<&str>) -> Result<CardStatusSummary> {
        let user_config = self.effective_user_config()?;
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        self.state
            .card_adapter
            .card_status(&user_config, effective_reader.as_deref())
            .await
            .inspect(|status| {
                if let Ok(mut sessions) = self.state.card_sessions.lock() {
                    let gp_secure_channel = sessions
                        .get(&status.reader_name)
                        .and_then(|record| record.gp_secure_channel.clone());
                    sessions.insert(
                        status.reader_name.clone(),
                        CardSessionRecord {
                            session_state: status.session_state.clone(),
                            gp_secure_channel,
                        },
                    );
                }
            })
    }

    /// Install one project's CAP onto a physical card.
    pub async fn install_project_cap(
        &self,
        selector: &ProjectSelectorInput,
        reader_name: Option<&str>,
    ) -> Result<CardInstallSummary> {
        let effective_cap = self.resolve_install_cap_path(selector)?;
        self.install_cap_from_path(&effective_cap, reader_name, Some(selector))
            .await
    }

    /// Install one explicit CAP onto a physical card.
    pub async fn install_cap_from_path(
        &self,
        cap_path: &Path,
        reader_name: Option<&str>,
        selector: Option<&ProjectSelectorInput>,
    ) -> Result<CardInstallSummary> {
        let effective_cap = self.resolve_input_path(cap_path)?;
        let effective_reader = self.effective_card_reader(reader_name, selector)?;
        let cap_package = CapPackage::from_path(&effective_cap)?;
        let user_config = self.effective_user_config()?;
        let output_lines = self
            .state
            .card_adapter
            .install_cap(&user_config, effective_reader.as_deref(), &effective_cap)
            .await?;
        Ok(CardInstallSummary {
            reader_name: effective_reader.unwrap_or_default(),
            cap_path: effective_cap,
            package_name: cap_package.package_name,
            package_aid: cap_package.package_aid.to_hex(),
            applets: cap_package
                .applets
                .into_iter()
                .map(|applet| AppletSummary {
                    class_name: applet.name.unwrap_or_else(|| "InstalledApplet".to_string()),
                    aid: applet.aid.to_hex(),
                })
                .collect(),
            output_lines,
        })
    }

    /// Delete one package from a physical card.
    pub async fn delete_item(
        &self,
        reader_name: Option<&str>,
        aid: &str,
    ) -> Result<CardDeleteSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let output_lines = self
            .state
            .card_adapter
            .delete_item(&user_config, effective_reader.as_deref(), aid)
            .await?;
        Ok(CardDeleteSummary {
            reader_name: effective_reader.unwrap_or_default(),
            aid: aid.to_string(),
            deleted: true,
            output_lines,
        })
    }

    /// List packages visible on a physical card.
    pub async fn list_packages(&self, reader_name: Option<&str>) -> Result<CardPackageInventory> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let mut inventory = self
            .state
            .card_adapter
            .list_packages(&user_config, effective_reader.as_deref())
            .await?;
        if inventory.reader_name.is_empty() {
            inventory.reader_name = effective_reader.unwrap_or_default();
        }
        Ok(inventory)
    }

    /// List applets visible on a physical card.
    pub async fn list_applets(&self, reader_name: Option<&str>) -> Result<CardAppletInventory> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let mut inventory = self
            .state
            .card_adapter
            .list_applets(&user_config, effective_reader.as_deref())
            .await?;
        if inventory.reader_name.is_empty() {
            inventory.reader_name = effective_reader.unwrap_or_default();
        }
        Ok(inventory)
    }

    /// Send one APDU to a physical card.
    pub async fn card_apdu(&self, reader_name: Option<&str>, apdu_hex: &str) -> Result<String> {
        let command = CommandApdu::parse(&hex::decode(apdu_hex)?)?;
        let exchange = self.card_command(reader_name, &command).await?;
        Ok(hex::encode_upper(exchange.response.to_bytes()))
    }

    /// Send one typed command APDU to a physical card.
    pub async fn card_command(
        &self,
        reader_name: Option<&str>,
        command: &CommandApdu,
    ) -> Result<ApduExchangeSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let reader_key = effective_reader.clone().unwrap_or_default();
        let gp_secure_channel = self
            .state
            .card_sessions
            .lock()
            .map_err(lock_poisoned)?
            .get(&reader_key)
            .and_then(|record| record.gp_secure_channel.clone());
        let response = self
            .transmit_card_command_with_optional_gp_auth(
                &user_config,
                effective_reader.as_deref(),
                gp_secure_channel.as_ref(),
                command,
            )
            .await?;
        let session_state = if let Ok(mut sessions) = self.state.card_sessions.lock() {
            let entry = sessions
                .entry(reader_key.clone())
                .or_insert_with(|| CardSessionRecord {
                    session_state: IsoSessionState::default(),
                    gp_secure_channel: None,
                });
            let _ = apply_response_to_session(&mut entry.session_state, command, &response);
            if entry.session_state.secure_messaging.active {
                entry.session_state.secure_messaging.command_counter = entry
                    .session_state
                    .secure_messaging
                    .command_counter
                    .saturating_add(1);
            }
            entry.session_state.clone()
        } else {
            self.card_session_state(Some(&reader_key))?
        };
        Ok(ApduExchangeSummary {
            command: command.clone(),
            response,
            session_state,
        })
    }

    /// Return the current tracked ISO/IEC 7816 session state for one physical card reader.
    pub fn card_session_state(&self, reader_name: Option<&str>) -> Result<IsoSessionState> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let key = effective_reader.unwrap_or_default();
        let sessions = self.state.card_sessions.lock().map_err(lock_poisoned)?;
        Ok(sessions
            .get(&key)
            .map(|record| record.session_state.clone())
            .unwrap_or_default())
    }

    /// Open or close one logical channel on a physical card.
    pub async fn manage_card_channel(
        &self,
        reader_name: Option<&str>,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<ManageChannelSummary> {
        let command = if open {
            iso7816::manage_channel_open()
        } else {
            iso7816::manage_channel_close(channel_number.unwrap_or_default())
        };
        let exchange = self.card_command(reader_name, &command).await?;
        let channel_number = if open {
            exchange.response.data.first().copied().or(channel_number)
        } else {
            channel_number
        };
        Ok(ManageChannelSummary {
            channel_number,
            response: exchange.response,
            session_state: exchange.session_state,
        })
    }

    /// Mark one secure-messaging session as open for one physical card reader.
    pub fn open_card_secure_messaging(
        &self,
        reader_name: Option<&str>,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<SecureMessagingSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let key = effective_reader.unwrap_or_default();
        let mut sessions = self.state.card_sessions.lock().map_err(lock_poisoned)?;
        let entry = sessions.entry(key).or_insert_with(|| CardSessionRecord {
            session_state: IsoSessionState::default(),
            gp_secure_channel: None,
        });
        entry.session_state.secure_messaging = SecureMessagingState {
            active: true,
            protocol,
            security_level,
            session_id,
            command_counter: 0,
        };
        entry.gp_secure_channel = None;
        Ok(SecureMessagingSummary {
            session_state: entry.session_state.clone(),
        })
    }

    /// Advance the tracked secure-messaging command counter for one physical card reader.
    pub fn advance_card_secure_messaging(
        &self,
        reader_name: Option<&str>,
        increment_by: u32,
    ) -> Result<SecureMessagingSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let key = effective_reader.unwrap_or_default();
        let mut sessions = self.state.card_sessions.lock().map_err(lock_poisoned)?;
        let entry = sessions.entry(key).or_insert_with(|| CardSessionRecord {
            session_state: IsoSessionState::default(),
            gp_secure_channel: None,
        });
        entry.session_state.secure_messaging.command_counter = entry
            .session_state
            .secure_messaging
            .command_counter
            .saturating_add(increment_by.max(1));
        Ok(SecureMessagingSummary {
            session_state: entry.session_state.clone(),
        })
    }

    /// Close the tracked secure-messaging session for one physical card reader.
    pub fn close_card_secure_messaging(
        &self,
        reader_name: Option<&str>,
    ) -> Result<SecureMessagingSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let key = effective_reader.unwrap_or_default();
        let mut sessions = self.state.card_sessions.lock().map_err(lock_poisoned)?;
        let entry = sessions.entry(key).or_insert_with(|| CardSessionRecord {
            session_state: IsoSessionState::default(),
            gp_secure_channel: None,
        });
        entry.session_state.secure_messaging = SecureMessagingState::default();
        entry.gp_secure_channel = None;
        Ok(SecureMessagingSummary {
            session_state: entry.session_state.clone(),
        })
    }

    /// Open one typed GP secure channel on a physical card.
    pub async fn open_gp_secure_channel_on_card(
        &self,
        reader_name: Option<&str>,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
    ) -> Result<GpSecureChannelSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let keyset = ResolvedGpKeyset::resolve(keyset_name)?;
        let security_level_byte = security_level.unwrap_or(0x01);
        let security_level = gp_security_level(security_level_byte);
        let selected_aid = Aid::from_slice(&globalplatform::ISSUER_SECURITY_DOMAIN_AID)?;
        let secure_channel = globalplatform::EstablishedSecureChannel {
            keyset: keyset.metadata(),
            security_level,
            session_id: format!(
                "card-gp-{}",
                effective_reader
                    .clone()
                    .unwrap_or_else(|| "default".to_string())
            ),
        };

        self.state
            .card_adapter
            .open_gp_secure_channel(
                &user_config,
                effective_reader.as_deref(),
                &keyset,
                security_level_byte,
            )
            .await?;
        self.card_status(effective_reader.as_deref()).await?;

        let reader_key = effective_reader.clone().unwrap_or_default();
        let mut sessions = self.state.card_sessions.lock().map_err(lock_poisoned)?;
        let entry = sessions
            .entry(reader_key)
            .or_insert_with(|| CardSessionRecord {
                session_state: IsoSessionState::default(),
                gp_secure_channel: None,
            });
        entry.session_state.selected_aid = Some(selected_aid.clone());
        entry.session_state.current_file = None;
        if let Some(channel) = entry
            .session_state
            .open_channels
            .iter_mut()
            .find(|channel| channel.channel_number == 0)
        {
            channel.selected_aid = Some(selected_aid.clone());
            channel.current_file = None;
        }
        entry.session_state.secure_messaging = SecureMessagingState {
            active: true,
            protocol: Some(keyset.protocol()),
            security_level: Some(security_level.as_byte()),
            session_id: Some(secure_channel.session_id.clone()),
            command_counter: 0,
        };
        entry.gp_secure_channel = Some(secure_channel.clone());
        Ok(GpSecureChannelSummary {
            secure_channel,
            selected_aid,
            session_state: entry.session_state.clone(),
        })
    }

    /// Close one typed GP secure channel on a physical card.
    pub fn close_gp_secure_channel_on_card(
        &self,
        reader_name: Option<&str>,
    ) -> Result<SecureMessagingSummary> {
        self.close_card_secure_messaging(reader_name)
    }

    /// Reset a physical card and return the ATR.
    pub async fn reset_card_summary(&self, reader_name: Option<&str>) -> Result<ResetSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let summary = self
            .state
            .card_adapter
            .reset_card_summary(&user_config, effective_reader.as_deref())
            .await?;
        if let Ok(mut sessions) = self.state.card_sessions.lock() {
            sessions.insert(
                effective_reader.unwrap_or_default(),
                CardSessionRecord {
                    session_state: summary.session_state.clone(),
                    gp_secure_channel: None,
                },
            );
        }
        Ok(summary)
    }

    /// Persist machine-local toolchain settings.
    pub fn setup_toolchains(&self, java_bin: Option<&str>) -> Result<SetupSummary> {
        let mut user_config = self.state.user_config.write().map_err(lock_poisoned)?;
        if let Some(java_bin) = java_bin {
            user_config.java_bin = java_bin.to_string();
        }
        if user_config.bundle_root.is_none() {
            user_config.bundle_root = Some(default_bundle_root());
        }
        user_config.save_to_path(&self.state.managed_paths.config_path)?;
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
        let user_config = self
            .state
            .user_config
            .read()
            .map_err(lock_poisoned)?
            .clone();
        let java_runtime = self.resolved_java_runtime()?;
        let java_source = match java_runtime.source {
            JavaRuntimeSource::Bundled => "bundled",
            JavaRuntimeSource::Configured => "configured",
        };
        Ok(vec![
            format!("Managed root: {}", self.state.managed_paths.root.display()),
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

    fn resolved_java_runtime(&self) -> Result<ResolvedJavaRuntime> {
        let user_config = self
            .state
            .user_config
            .read()
            .map_err(lock_poisoned)?
            .clone();
        resolve_java_runtime(&self.state.managed_paths.bundle_root, &user_config.java_bin)
    }

    fn effective_user_config(&self) -> Result<UserConfig> {
        let mut user_config = self
            .state
            .user_config
            .read()
            .map_err(lock_poisoned)?
            .clone();
        user_config.bundle_root = Some(user_config.bundle_root.unwrap_or_else(default_bundle_root));
        user_config.java_bin = self.resolved_java_runtime()?.java_bin.display().to_string();
        Ok(user_config)
    }

    fn resolve_project(&self, selector: &ProjectSelectorInput) -> Result<ResolvedProject> {
        if let Some(project_path) = &selector.project_path {
            return self.load_project_from_input(project_path);
        }
        if let Some(project_id) = &selector.project_id {
            let project_path = self
                .state
                .registry
                .read()
                .map_err(lock_poisoned)?
                .by_id(project_id)
                .map(|entry| entry.project_path.clone())
                .ok_or_else(|| {
                    JcimError::Unsupported(format!("unknown project id `{project_id}`"))
                })?;
            return self.load_project_by_root(&project_path);
        }
        Err(JcimError::Unsupported(
            "missing project selector; pass a project path or id".to_string(),
        ))
    }

    fn load_project_from_input(&self, input: &Path) -> Result<ResolvedProject> {
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

    fn load_project_by_root(&self, project_root: &Path) -> Result<ResolvedProject> {
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

    fn register_project(&self, project_root: &Path) -> Result<String> {
        let mut registry = self.state.registry.write().map_err(lock_poisoned)?;
        let record = registry.upsert(project_root)?;
        registry.save_to_path(&self.state.managed_paths.registry_path)?;
        Ok(record.project_id)
    }

    fn project_summary(&self, resolved: &ResolvedProject) -> ProjectSummary {
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
                source_kind: SimulationSourceKind::Project,
                project_id: Some(resolved.project_id),
                project_path: Some(resolved.project_root),
                cap_path,
                engine_mode: engine_mode_for_current_host(),
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

    async fn start_prepared_simulation(
        &self,
        prepared: PreparedSimulation,
        reset_after_start: bool,
    ) -> Result<SimulationSummary> {
        let bundle_dir = prepared.runtime_config.backend_bundle_dir();
        ensure_host_simulator_environment(
            prepared.summary.engine_mode,
            &bundle_dir,
            prepared.runtime_config.profile_id,
        )?;
        let handle = BackendHandle::from_config(prepared.runtime_config)?;
        let handshake = handle.handshake(ProtocolVersion::current()).await?;
        if reset_after_start {
            let _ = handle.reset().await?;
        }
        let health = handle.backend_health().await?;
        let snapshot = handle.snapshot().await?;
        let packages = handle.list_packages().await.unwrap_or_default();
        let applets = handle.list_applets().await.unwrap_or_default();
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
        let iso_capabilities = snapshot.iso_capabilities.clone();
        let session_state = snapshot.session_state.clone();

        let mut record = SimulationRecord {
            simulation_id: prepared.summary.simulation_id.clone(),
            source_kind: prepared.summary.source_kind,
            project_id: prepared.summary.project_id.clone(),
            project_path: prepared.summary.project_path.clone(),
            cap_path: prepared.summary.cap_path.clone(),
            engine_mode: prepared.summary.engine_mode,
            status: SimulationStatusKind::Running,
            reader_name: handshake.reader_name,
            health: format!("{} ({})", health.message, health.status.status_string()),
            atr,
            active_protocol,
            iso_capabilities,
            session_state,
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
            recent_events: VecDeque::new(),
            handle: Some(handle),
        };
        remember_event(
            &mut record.recent_events,
            "info",
            format!("simulation started from {}", record.source_kind.as_str()),
        );

        let summary = record.summary();
        self.state
            .simulations
            .lock()
            .map_err(lock_poisoned)?
            .insert(record.simulation_id.clone(), record);
        Ok(summary)
    }

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

    fn resolve_install_cap_path(&self, selector: &ProjectSelectorInput) -> Result<PathBuf> {
        let resolved = self.resolve_project(selector)?;
        if let Some(cap_path) = &resolved.config.card.default_cap_path {
            return Ok(resolve_project_path(&resolved.project_root, cap_path));
        }

        let metadata = if resolved.config.card.auto_build_before_install {
            let request = artifact_metadata_from_project(&resolved.project_root, &resolved.config)?;
            let toolchain = build_toolchain_layout()?;
            let java_runtime = self.resolved_java_runtime()?;
            build_project_artifacts_if_stale_with_java_bin(
                &request,
                &toolchain,
                &java_runtime.java_bin,
            )?
            .metadata
        } else {
            load_artifact_metadata(&resolved.project_root)?.ok_or_else(|| {
                JcimError::Unsupported(
                    "no CAP artifact is recorded for this project and automatic card builds are disabled".to_string(),
                )
            })?
        };

        required_artifact_path(
            &resolved.project_root,
            metadata.cap_path.as_ref(),
            "the selected project does not provide a CAP artifact for card install",
        )
    }

    fn effective_card_reader(
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
        Ok(self
            .state
            .user_config
            .read()
            .map_err(lock_poisoned)?
            .default_reader
            .clone())
    }

    fn resolve_input_path(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(std::env::current_dir()?.join(path))
        }
    }

    fn simulation_handle(&self, selector: &SimulationSelectorInput) -> Result<BackendHandle> {
        let simulations = self.state.simulations.lock().map_err(lock_poisoned)?;
        let simulation = simulations.get(&selector.simulation_id).ok_or_else(|| {
            JcimError::Unsupported(format!(
                "unknown simulation id `{}`",
                selector.simulation_id
            ))
        })?;
        simulation.handle.clone().ok_or_else(|| {
            JcimError::BackendUnavailable(format!(
                "simulation `{}` is no longer running",
                selector.simulation_id
            ))
        })
    }

    async fn transmit_card_command_with_optional_gp_auth(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        secure_channel: Option<&globalplatform::EstablishedSecureChannel>,
        command: &CommandApdu,
    ) -> Result<jcim_core::apdu::ResponseApdu> {
        if let Some(secure_channel) = secure_channel
            && matches!(
                iso7816::describe_command(command).domain,
                iso7816::CommandDomain::GlobalPlatform
            )
        {
            let keyset = ResolvedGpKeyset::resolve(Some(&secure_channel.keyset.name))?;
            match self
                .state
                .card_adapter
                .transmit_gp_secure_command(
                    user_config,
                    reader_name,
                    &keyset,
                    secure_channel.security_level.as_byte(),
                    command,
                )
                .await
            {
                Ok(response) => return Ok(response),
                Err(JcimError::Unsupported(_)) => {
                    return Err(JcimError::Unsupported(
                        "tracked GP secure channel requires authenticated GP transport support from the active physical-card adapter".to_string(),
                    ))
                }
                Err(error) => return Err(error),
            }
        }

        self.state
            .card_adapter
            .transmit_command(user_config, reader_name, command)
            .await
    }

    fn active_simulation_count(&self) -> u32 {
        self.state
            .simulations
            .lock()
            .map(|simulations| {
                simulations
                    .values()
                    .filter(|simulation| {
                        matches!(
                            simulation.status,
                            SimulationStatusKind::Starting | SimulationStatusKind::Running
                        )
                    })
                    .count() as u32
            })
            .unwrap_or(0)
    }

    fn remember_build_event(&self, project_id: &str, level: &str, message: String) {
        if let Ok(mut events) = self.state.build_events.lock() {
            let queue = events.entry(project_id.to_string()).or_default();
            remember_event(queue, level, message);
        }
    }

    fn next_simulation_id(&self) -> String {
        let id = self
            .state
            .next_simulation_id
            .fetch_add(1, Ordering::Relaxed);
        format!("sim-{id:016x}")
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

fn current_service_binary_identity() -> Result<(PathBuf, String)> {
    let path = std::env::current_exe()?;
    let metadata = std::fs::metadata(&path)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Ok((
        path,
        format!(
            "{}:{}:{}",
            metadata.len(),
            modified.as_secs(),
            modified.subsec_nanos()
        ),
    ))
}

impl SimulationRecord {
    fn summary(&self) -> SimulationSummary {
        SimulationSummary {
            simulation_id: self.simulation_id.clone(),
            source_kind: self.source_kind,
            project_id: self.project_id.clone(),
            project_path: self.project_path.clone(),
            cap_path: self.cap_path.clone(),
            engine_mode: self.engine_mode,
            status: self.status,
            reader_name: self.reader_name.clone(),
            health: self.health.clone(),
            atr: self.atr.clone(),
            active_protocol: self.active_protocol.clone(),
            iso_capabilities: self.iso_capabilities.clone(),
            session_state: self.session_state.clone(),
            package_count: self.package_count,
            applet_count: self.applet_count,
            package_name: self.package_name.clone(),
            package_aid: self.package_aid.clone(),
            recent_events: self
                .recent_events
                .iter()
                .map(|event| format!("{}: {}", event.level, event.message))
                .collect(),
        }
    }
}

fn apply_authoritative_simulation_session(
    simulation: &mut SimulationRecord,
    session_state: &IsoSessionState,
) {
    simulation.atr = session_state.atr.clone();
    simulation.active_protocol = session_state
        .active_protocol
        .clone()
        .or_else(|| simulation.atr.as_ref().map(ProtocolParameters::from_atr));
    simulation.session_state = session_state.clone();
}

trait BackendHealthStatusExt {
    fn status_string(self) -> &'static str;
}

impl BackendHealthStatusExt for BackendHealthStatus {
    fn status_string(self) -> &'static str {
        match self {
            BackendHealthStatus::Ready => "ready",
            BackendHealthStatus::Degraded => "degraded",
            BackendHealthStatus::Unavailable => "unavailable",
            _ => "unknown",
        }
    }
}

fn artifacts_from_metadata(
    project_root: &Path,
    metadata: &ArtifactMetadata,
) -> Vec<ArtifactSummary> {
    let mut artifacts = Vec::new();
    if let Some(path) = &metadata.cap_path {
        artifacts.push(ArtifactSummary {
            kind: "cap".to_string(),
            path: project_root.join(path),
        });
    }
    artifacts
}

fn required_artifact_path(
    project_root: &Path,
    relative: Option<&PathBuf>,
    message: &str,
) -> Result<PathBuf> {
    let relative = relative.ok_or_else(|| JcimError::Unsupported(message.to_string()))?;
    Ok(project_root.join(relative))
}

fn engine_mode_for_current_host() -> SimulationEngineMode {
    SimulationEngineMode::ManagedJava
}

fn ensure_host_simulator_environment(
    _engine_mode: SimulationEngineMode,
    _bundle_dir: &Path,
    _profile_id: CardProfileId,
) -> Result<()> {
    Ok(())
}

fn validate_simulation_artifacts(
    project_root: &Path,
    metadata: ArtifactMetadata,
) -> Result<ArtifactMetadata> {
    let cap_path = required_artifact_path(
        project_root,
        metadata.cap_path.as_ref(),
        "project build did not emit a CAP artifact required for simulation",
    )?;
    if !cap_path.exists() {
        return Err(JcimError::Unsupported(format!(
            "expected CAP artifact is missing at {}",
            cap_path.display()
        )));
    }
    let metadata_path = project_root.join(&metadata.simulator_metadata_path);
    if !metadata_path.exists() {
        return Err(JcimError::Unsupported(format!(
            "expected simulator metadata is missing at {}",
            metadata_path.display()
        )));
    }
    let classes_path = project_root.join(&metadata.classes_path);
    if !classes_path.exists() {
        return Err(JcimError::Unsupported(format!(
            "expected compiled classes are missing at {}",
            classes_path.display()
        )));
    }
    for dependency in &metadata.runtime_classpath {
        let dependency_path = project_root.join(dependency);
        if !dependency_path.exists() {
            return Err(JcimError::Unsupported(format!(
                "expected simulator runtime classpath entry is missing at {}",
                dependency_path.display()
            )));
        }
    }
    Ok(metadata)
}

fn remember_event(queue: &mut VecDeque<EventLine>, level: &str, message: impl Into<String>) {
    queue.push_back(EventLine {
        level: level.to_string(),
        message: message.into(),
    });
    while queue.len() > EVENT_LIMIT {
        queue.pop_front();
    }
}

fn default_bundle_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-backends")
}

fn gp_security_level(value: u8) -> globalplatform::SecurityLevel {
    match value {
        0x00 => globalplatform::SecurityLevel::None,
        0x01 => globalplatform::SecurityLevel::CommandMac,
        0x11 => globalplatform::SecurityLevel::CommandAndResponseMac,
        0x03 => globalplatform::SecurityLevel::CommandMacAndEncryption,
        0x13 => globalplatform::SecurityLevel::CommandAndResponseMacWithEncryption,
        other => globalplatform::SecurityLevel::Raw(other),
    }
}

fn gp_host_challenge() -> [u8; 8] {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut challenge = [0u8; 8];
    for (index, byte) in challenge.iter_mut().enumerate() {
        *byte = (nanos >> (index * 8)) as u8;
    }
    challenge
}

fn lock_poisoned<T>(_: T) -> JcimError {
    JcimError::Unsupported("internal application state lock was poisoned".to_string())
}

fn truncate_hex(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 16 {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..16])
    }
}

fn split_class_name(value: &str) -> Result<(String, String)> {
    if let Some((package_name, class_name)) = value.rsplit_once('.') {
        if class_name.is_empty() {
            return Err(JcimError::Unsupported(format!(
                "invalid applet class name `{value}`"
            )));
        }
        Ok((package_name.to_string(), class_name.to_string()))
    } else {
        Ok((String::new(), value.to_string()))
    }
}

fn sample_applet_source(package_name: &str, class_name: &str) -> String {
    let mut source = String::new();
    if !package_name.is_empty() {
        source.push_str(&format!("package {package_name};\n\n"));
    }
    source.push_str(
        "import javacard.framework.APDU;\n\
         import javacard.framework.Applet;\n\n",
    );
    source.push_str(&format!(
        "public final class {class_name} extends Applet {{\n\
             private {class_name}() {{}}\n\n\
             public static void install(byte[] buffer, short offset, byte length) {{\n\
                 new {class_name}().register();\n\
             }}\n\n\
             @Override\n\
             public void process(APDU apdu) {{\n\
                 if (selectingApplet()) {{\n\
                     return;\n\
                 }}\n\
                 apdu.setOutgoingAndSend((short) 0, (short) 0);\n\
             }}\n\
         }}\n"
    ));
    source
}

#[cfg(test)]
mod tests {
    use jcim_core::model::CardProfileId;

    use super::{
        SimulationEngineMode, engine_mode_for_current_host, ensure_host_simulator_environment,
    };

    #[test]
    fn simulator_engine_defaults_to_managed_java() {
        assert_eq!(
            engine_mode_for_current_host(),
            SimulationEngineMode::ManagedJava
        );
    }

    #[test]
    fn host_environment_check_is_noop_for_managed_java() {
        ensure_host_simulator_environment(
            SimulationEngineMode::ManagedJava,
            std::path::Path::new("/tmp/jcim/bundled-backends/simulator"),
            CardProfileId::Classic304,
        )
        .expect("managed java simulator environment");
    }
}
