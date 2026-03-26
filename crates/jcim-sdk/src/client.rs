//! Service bootstrap and lifecycle methods for the JCIM SDK.

#![allow(clippy::missing_docs_in_private_items)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tokio::time::sleep;
use tonic::Code;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use jcim_api::v0_2::build_service_client::BuildServiceClient;
use jcim_api::v0_2::card_service_client::CardServiceClient;
use jcim_api::v0_2::install_cap_request::Input as InstallCapInput;
use jcim_api::v0_2::project_service_client::ProjectServiceClient;
use jcim_api::v0_2::simulator_service_client::SimulatorServiceClient;
use jcim_api::v0_2::system_service_client::SystemServiceClient;
use jcim_api::v0_2::workspace_service_client::WorkspaceServiceClient;
use jcim_api::v0_2::{
    BuildProjectRequest, CardApduRequest, CardManageChannelRequest, CardRawApduRequest,
    CardSecureMessagingAdvanceRequest, CardSecureMessagingRequest, CardSelector, CardStatusRequest,
    CleanProjectResponse, CreateProjectRequest, Empty, GetServiceStatusResponse, InstallCapRequest,
    ListAppletsRequest, ListPackagesRequest, ManageChannelRequest, OpenCardGpSecureChannelRequest,
    OpenGpSecureChannelRequest, ProjectSelector, ResetCardRequest, ResetCardResponse,
    ResetSimulationResponse, SecureMessagingAdvanceRequest, SecureMessagingRequest,
    SetupToolchainsRequest, SimulationSelector, StartSimulationRequest, TransmitApduRequest,
    TransmitRawApduRequest,
};
use jcim_config::project::ManagedPaths;
use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::error::JcimError;
use jcim_core::globalplatform;
use jcim_core::iso7816::{
    self, Atr, CommandDomain, CommandKind, FileSelection, IsoCapabilities, IsoSessionState,
    LogicalChannelState, PowerState, ProtocolParameters, RetryCounterState,
    SecureMessagingProtocol, SecureMessagingState, StatusWord, TransportProtocol,
};

use crate::connection::CardConnection;
use crate::error::{JcimSdkError, Result};
use crate::types::{
    ApduExchangeSummary, AppletSummary, ArtifactSummary, BuildSummary, CardAppletInventory,
    CardAppletSummary, CardConnectionLocator, CardConnectionTarget, CardDeleteSummary,
    CardInstallSource, CardInstallSummary, CardPackageInventory, CardPackageSummary,
    CardReaderSummary, CardStatusSummary, EventLine, GpSecureChannelSummary, ManageChannelSummary,
    OverviewSummary, ProjectDetails, ProjectRef, ProjectSummary, ReaderRef, ResetSummary,
    SecureMessagingSummary, ServiceStatusSummary, SetupSummary, SimulationEngineMode,
    SimulationInput, SimulationRef, SimulationSourceKind, SimulationStatus, SimulationSummary,
    owned_path,
};

/// Canonical async Rust client for the local JCIM service.
#[derive(Clone)]
pub struct JcimClient {
    managed_paths: ManagedPaths,
    channel: Channel,
}

impl JcimClient {
    /// Connect to an already-running local JCIM service using the default managed paths.
    pub async fn connect() -> Result<Self> {
        let managed_paths = ManagedPaths::discover()?;
        Self::connect_with_paths(managed_paths).await
    }

    /// Connect to an already-running local JCIM service using explicit managed paths.
    pub async fn connect_with_paths(managed_paths: ManagedPaths) -> Result<Self> {
        let channel = connect_channel(&managed_paths.service_socket_path).await?;
        Ok(Self {
            managed_paths,
            channel,
        })
    }

    /// Connect to the local JCIM service, starting it if needed, using the default managed paths.
    pub async fn connect_or_start() -> Result<Self> {
        let managed_paths = ManagedPaths::discover()?;
        Self::connect_or_start_with_paths(managed_paths).await
    }

    /// Connect to the local JCIM service, starting it if needed, using explicit managed paths.
    pub async fn connect_or_start_with_paths(managed_paths: ManagedPaths) -> Result<Self> {
        if let Ok(channel) = connect_channel(&managed_paths.service_socket_path).await {
            let client = Self {
                managed_paths: managed_paths.clone(),
                channel,
            };
            if client.connected_service_matches_current_binary().await? {
                return Ok(client);
            }
            drop(client);
            disconnect_stale_service_socket(&managed_paths)?;
        }

        let mut service = spawn_service(&managed_paths)?;
        for _ in 0..40 {
            if let Ok(channel) = connect_channel(&managed_paths.service_socket_path).await {
                return Ok(Self {
                    managed_paths,
                    channel,
                });
            }
            if let Some(status) = service.child.try_wait().map_err(|error| {
                JcimSdkError::Bootstrap(format!("unable to observe jcimd startup status: {error}"))
            })? {
                let log_tail = read_bootstrap_log_tail(&service.stderr_log_path);
                return Err(JcimSdkError::Bootstrap(match log_tail {
                    Some(log_tail) => format!(
                        "jcimd exited during startup with status {status}. stderr from {}:\n{log_tail}",
                        service.stderr_log_path.display()
                    ),
                    None => format!(
                        "jcimd exited during startup with status {status}. no stderr was captured at {}",
                        service.stderr_log_path.display()
                    ),
                }));
            }
            sleep(Duration::from_millis(100)).await;
        }

        Err(JcimSdkError::Bootstrap(format!(
            "unable to connect to the JCIM local service at {} after startup; stderr log: {}",
            managed_paths.service_socket_path.display(),
            service.stderr_log_path.display()
        )))
    }

    /// Return the managed local paths associated with this client.
    pub fn managed_paths(&self) -> &ManagedPaths {
        &self.managed_paths
    }

    /// Open one unified APDU connection against a real reader or one virtual simulation.
    pub async fn open_card_connection(
        &self,
        target: CardConnectionTarget,
    ) -> Result<CardConnection> {
        let locator = match target {
            CardConnectionTarget::Reader(reader) => {
                let status = self.validated_card_status_for_connection(reader).await?;
                let reader_name = status.reader_name.trim().to_string();
                if reader_name.is_empty() {
                    return Err(JcimSdkError::InvalidResponse(
                        "service returned an empty reader name for an opened card connection"
                            .to_string(),
                    ));
                }
                CardConnectionLocator::Reader { reader_name }
            }
            CardConnectionTarget::ExistingSimulation(simulation) => {
                let summary = self.validated_running_simulation(simulation).await?;
                CardConnectionLocator::Simulation {
                    simulation: summary.simulation_ref(),
                    owned: false,
                }
            }
            CardConnectionTarget::StartSimulation(input) => {
                let summary = self.start_simulation(input).await?;
                if summary.status != SimulationStatus::Running {
                    let _ = self.stop_simulation(summary.simulation_ref()).await;
                    return Err(invalid_connection_target(format!(
                        "simulation `{}` is not running; current status is {:?}",
                        summary.simulation_id, summary.status
                    )));
                }
                CardConnectionLocator::Simulation {
                    simulation: summary.simulation_ref(),
                    owned: true,
                }
            }
        };
        Ok(CardConnection::new(self.clone(), locator))
    }

    /// Fetch a high-level overview of the local JCIM service state.
    pub async fn overview(&self) -> Result<OverviewSummary> {
        let overview = WorkspaceServiceClient::new(self.channel.clone())
            .get_overview(Empty {})
            .await?
            .into_inner()
            .overview
            .ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no overview".to_string())
            })?;
        Ok(OverviewSummary {
            known_project_count: overview.known_project_count,
            active_simulation_count: overview.active_simulation_count,
        })
    }

    /// List known projects.
    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let response = WorkspaceServiceClient::new(self.channel.clone())
            .list_projects(Empty {})
            .await?
            .into_inner();
        response.projects.into_iter().map(project_summary).collect()
    }

    /// List active simulations.
    pub async fn list_simulations(&self) -> Result<Vec<SimulationSummary>> {
        let response = WorkspaceServiceClient::new(self.channel.clone())
            .list_simulations(Empty {})
            .await?
            .into_inner();
        response
            .simulations
            .into_iter()
            .map(simulation_summary)
            .collect()
    }

    /// Create and register one project skeleton.
    pub async fn create_project(
        &self,
        name: &str,
        directory: impl AsRef<Path>,
    ) -> Result<ProjectSummary> {
        let project = ProjectServiceClient::new(self.channel.clone())
            .create_project(CreateProjectRequest {
                name: name.to_string(),
                directory: directory.as_ref().display().to_string(),
            })
            .await?
            .into_inner()
            .project
            .ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no project".to_string())
            })?;
        project_summary(project)
    }

    /// Load one project.
    pub async fn get_project(&self, project: &ProjectRef) -> Result<ProjectDetails> {
        let response = ProjectServiceClient::new(self.channel.clone())
            .get_project(project_selector(project))
            .await?
            .into_inner();
        Ok(ProjectDetails {
            project: project_summary(response.project.ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no project".to_string())
            })?)?,
            manifest_toml: response.manifest_toml,
        })
    }

    /// Clean one project's generated local state.
    pub async fn clean_project(&self, project: &ProjectRef) -> Result<PathBuf> {
        let CleanProjectResponse { cleaned_path } = ProjectServiceClient::new(self.channel.clone())
            .clean_project(project_selector(project))
            .await?
            .into_inner();
        Ok(owned_path(cleaned_path))
    }

    /// Build one project and return the current artifact set.
    pub async fn build_project(&self, project: &ProjectRef) -> Result<BuildSummary> {
        let response = BuildServiceClient::new(self.channel.clone())
            .build_project(BuildProjectRequest {
                project: Some(project_selector(project)),
            })
            .await?
            .into_inner();
        let artifacts = response
            .artifacts
            .into_iter()
            .map(artifact_summary)
            .collect::<Result<Vec<_>>>()?;
        Ok(BuildSummary {
            project: project_summary(response.project.ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no project".to_string())
            })?)?,
            artifacts,
            rebuilt: response.rebuilt,
        })
    }

    /// Return the current recorded artifact set for one project without rebuilding it.
    pub async fn get_artifacts(&self, project: &ProjectRef) -> Result<Vec<ArtifactSummary>> {
        let response = BuildServiceClient::new(self.channel.clone())
            .get_artifacts(project_selector(project))
            .await?
            .into_inner();
        response
            .artifacts
            .into_iter()
            .map(artifact_summary)
            .collect()
    }

    /// Start one simulation from a JCIM project.
    pub async fn start_simulation(&self, input: SimulationInput) -> Result<SimulationSummary> {
        let request = StartSimulationRequest {
            project: Some(match input {
                SimulationInput::Project(project) => project_selector(&project),
            }),
        };
        let simulation = SimulatorServiceClient::new(self.channel.clone())
            .start_simulation(request)
            .await?
            .into_inner()
            .simulation
            .ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no simulation".to_string())
            })?;
        simulation_summary(simulation)
    }

    /// Get one simulation by id.
    pub async fn get_simulation(&self, simulation: SimulationRef) -> Result<SimulationSummary> {
        let simulation = SimulatorServiceClient::new(self.channel.clone())
            .get_simulation(SimulationSelector {
                simulation_id: simulation.simulation_id,
            })
            .await?
            .into_inner()
            .simulation
            .ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no simulation".to_string())
            })?;
        simulation_summary(simulation)
    }

    /// Stop one simulation.
    pub async fn stop_simulation(&self, simulation: SimulationRef) -> Result<SimulationSummary> {
        let simulation = SimulatorServiceClient::new(self.channel.clone())
            .stop_simulation(SimulationSelector {
                simulation_id: simulation.simulation_id,
            })
            .await?
            .into_inner()
            .simulation
            .ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no simulation".to_string())
            })?;
        simulation_summary(simulation)
    }

    /// Return retained simulation event lines.
    pub async fn simulation_events(&self, simulation: SimulationRef) -> Result<Vec<EventLine>> {
        let mut stream = SimulatorServiceClient::new(self.channel.clone())
            .stream_simulation_events(SimulationSelector {
                simulation_id: simulation.simulation_id,
            })
            .await?
            .into_inner();
        let mut events = Vec::new();
        while let Some(event) = stream.message().await? {
            events.push(EventLine {
                level: event.level,
                message: event.message,
            });
        }
        Ok(events)
    }

    /// Send one APDU to a running simulation.
    pub async fn transmit_sim_apdu(
        &self,
        simulation: SimulationRef,
        apdu: &CommandApdu,
    ) -> Result<ResponseApdu> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .transmit_apdu(TransmitApduRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: simulation.simulation_id,
                }),
                command: Some(command_apdu_frame(apdu)),
            })
            .await?
            .into_inner()
            .response;
        response_apdu_from_proto(response)
    }

    /// Fetch the current tracked ISO/IEC 7816 session state for one running simulation.
    pub async fn get_simulation_session_state(
        &self,
        simulation: SimulationRef,
    ) -> Result<IsoSessionState> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .get_session_state(SimulationSelector {
                simulation_id: simulation.simulation_id,
            })
            .await?
            .into_inner();
        iso_session_state_from_proto(response.session_state)
    }

    /// Send one raw APDU byte sequence to a running simulation.
    pub async fn transmit_raw_sim_apdu(
        &self,
        simulation: SimulationRef,
        apdu: &[u8],
    ) -> Result<ApduExchangeSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .transmit_raw_apdu(TransmitRawApduRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: simulation.simulation_id,
                }),
                apdu: apdu.to_vec(),
            })
            .await?
            .into_inner();
        Ok(ApduExchangeSummary {
            command: CommandApdu::parse(&response.apdu)?,
            response: response_apdu_from_proto(response.response)?,
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Open or close one logical channel on a running simulation.
    pub async fn manage_simulation_channel(
        &self,
        simulation: SimulationRef,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<ManageChannelSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .manage_channel(ManageChannelRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: simulation.simulation_id,
                }),
                open,
                channel_number: channel_number.map(u32::from),
            })
            .await?
            .into_inner();
        Ok(ManageChannelSummary {
            channel_number: response.channel_number.map(|value| value as u8),
            response: response_apdu_from_proto(response.response)?,
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Mark secure messaging as active for one running simulation.
    pub async fn open_simulation_secure_messaging(
        &self,
        simulation: SimulationRef,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<SecureMessagingSummary> {
        let (protocol, protocol_label) = secure_messaging_protocol_fields(protocol.as_ref());
        let response = SimulatorServiceClient::new(self.channel.clone())
            .open_secure_messaging(SecureMessagingRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: simulation.simulation_id,
                }),
                protocol,
                security_level: security_level.map(u32::from),
                session_id: session_id.unwrap_or_default(),
                protocol_label,
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Advance the secure-messaging command counter for one running simulation.
    pub async fn advance_simulation_secure_messaging(
        &self,
        simulation: SimulationRef,
        increment_by: u32,
    ) -> Result<SecureMessagingSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .advance_secure_messaging(SecureMessagingAdvanceRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: simulation.simulation_id,
                }),
                increment_by,
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Clear the tracked secure-messaging state for one running simulation.
    pub async fn close_simulation_secure_messaging(
        &self,
        simulation: SimulationRef,
    ) -> Result<SecureMessagingSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .close_secure_messaging(SimulationSelector {
                simulation_id: simulation.simulation_id,
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Open one typed GP secure channel on a running simulation.
    pub async fn open_gp_secure_channel_on_simulation(
        &self,
        simulation: SimulationRef,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
    ) -> Result<GpSecureChannelSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .open_gp_secure_channel(OpenGpSecureChannelRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: simulation.simulation_id,
                }),
                keyset_name: keyset_name.unwrap_or_default().to_string(),
                security_level: security_level.map(u32::from),
            })
            .await?
            .into_inner();
        gp_secure_channel_from_proto(response.secure_channel)
    }

    /// Close one typed GP secure channel on a running simulation.
    pub async fn close_gp_secure_channel_on_simulation(
        &self,
        simulation: SimulationRef,
    ) -> Result<SecureMessagingSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .close_gp_secure_channel(SimulationSelector {
                simulation_id: simulation.simulation_id,
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Send one ISO/IEC 7816 `SELECT` by application identifier to a running simulation.
    pub async fn iso_select_application_on_simulation(
        &self,
        simulation: SimulationRef,
        aid: &Aid,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(simulation, &iso7816::select_by_name(aid))
            .await
    }

    /// Send one GlobalPlatform `SELECT` for the issuer security domain to a running simulation.
    pub async fn gp_select_issuer_security_domain_on_simulation(
        &self,
        simulation: SimulationRef,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(simulation, &globalplatform::select_issuer_security_domain())
            .await
    }

    /// Run one typed GlobalPlatform `GET STATUS` request against a running simulation.
    pub async fn gp_get_status_on_simulation(
        &self,
        simulation: SimulationRef,
        kind: globalplatform::RegistryKind,
        occurrence: globalplatform::GetStatusOccurrence,
    ) -> Result<globalplatform::GetStatusResponse> {
        let response = self
            .transmit_sim_apdu(simulation, &globalplatform::get_status(kind, occurrence))
            .await?;
        Ok(globalplatform::parse_get_status(kind, &response)?)
    }

    /// Set one GlobalPlatform card life cycle state inside a running simulation.
    pub async fn gp_set_card_status_on_simulation(
        &self,
        simulation: SimulationRef,
        state: globalplatform::CardLifeCycle,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(simulation, &globalplatform::set_card_status(state))
            .await
    }

    /// Lock or unlock one application inside a running simulation.
    pub async fn gp_set_application_status_on_simulation(
        &self,
        simulation: SimulationRef,
        aid: &Aid,
        transition: globalplatform::LockTransition,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(
            simulation,
            &globalplatform::set_application_status(aid, transition),
        )
        .await
    }

    /// Lock or unlock one security domain and its applications inside a running simulation.
    pub async fn gp_set_security_domain_status_on_simulation(
        &self,
        simulation: SimulationRef,
        aid: &Aid,
        transition: globalplatform::LockTransition,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(
            simulation,
            &globalplatform::set_security_domain_status(aid, transition),
        )
        .await
    }

    /// Reset one running simulation and return the typed reset summary.
    pub async fn reset_simulation_summary(
        &self,
        simulation: SimulationRef,
    ) -> Result<ResetSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .reset_simulation(SimulationSelector {
                simulation_id: simulation.simulation_id,
            })
            .await?
            .into_inner();
        reset_summary_from_simulation_proto(response)
    }

    /// List visible physical readers.
    pub async fn list_readers(&self) -> Result<Vec<CardReaderSummary>> {
        let response = CardServiceClient::new(self.channel.clone())
            .list_readers(Empty {})
            .await?
            .into_inner();
        Ok(response
            .readers
            .into_iter()
            .map(|reader| CardReaderSummary {
                name: reader.name,
                card_present: reader.card_present,
            })
            .collect())
    }

    /// Fetch physical-card status using the configured default reader.
    pub async fn get_card_status(&self) -> Result<CardStatusSummary> {
        self.get_card_status_on(ReaderRef::Default).await
    }

    /// Fetch physical-card status using one explicit reader selector.
    pub async fn get_card_status_on(&self, reader: ReaderRef) -> Result<CardStatusSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .get_card_status(CardStatusRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(CardStatusSummary {
            reader_name: response.reader_name,
            card_present: response.card_present,
            atr: atr_from_proto(response.atr)?,
            active_protocol: protocol_parameters_from_proto(response.active_protocol),
            iso_capabilities: iso_capabilities_from_proto(response.iso_capabilities),
            session_state: iso_session_state_from_proto(response.session_state)?,
            lines: response.lines,
        })
    }

    /// Install a CAP onto a physical card using the configured default reader.
    pub async fn install_cap(&self, source: CardInstallSource) -> Result<CardInstallSummary> {
        self.install_cap_on(source, ReaderRef::Default).await
    }

    /// Install a CAP onto a physical card using one explicit reader selector.
    pub async fn install_cap_on(
        &self,
        source: CardInstallSource,
        reader: ReaderRef,
    ) -> Result<CardInstallSummary> {
        let request = InstallCapRequest {
            input: Some(match source {
                CardInstallSource::Project(project) => {
                    InstallCapInput::Project(project_selector(&project))
                }
                CardInstallSource::Cap(cap_path) => {
                    InstallCapInput::CapPath(cap_path.display().to_string())
                }
            }),
            reader_name: reader.as_deref().unwrap_or_default().to_string(),
        };
        let response = CardServiceClient::new(self.channel.clone())
            .install_cap(request)
            .await?
            .into_inner();
        Ok(CardInstallSummary {
            reader_name: response.reader_name,
            cap_path: owned_path(response.cap_path),
            package_name: response.package_name,
            package_aid: response.package_aid,
            applets: response
                .applets
                .into_iter()
                .map(|applet| AppletSummary {
                    class_name: applet.class_name,
                    aid: applet.aid,
                })
                .collect(),
            output_lines: response.output_lines,
        })
    }

    /// Delete one item using the configured default reader.
    pub async fn delete_item(&self, aid: &str) -> Result<CardDeleteSummary> {
        self.delete_item_on(aid, ReaderRef::Default).await
    }

    /// Delete one item using one explicit reader selector.
    pub async fn delete_item_on(&self, aid: &str, reader: ReaderRef) -> Result<CardDeleteSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .delete_item(jcim_api::v0_2::DeleteItemRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                aid: aid.to_string(),
            })
            .await?
            .into_inner();
        Ok(CardDeleteSummary {
            reader_name: response.reader_name,
            aid: response.aid,
            deleted: response.deleted,
            output_lines: response.output_lines,
        })
    }

    /// List packages using the configured default reader.
    pub async fn list_packages(&self) -> Result<CardPackageInventory> {
        self.list_packages_on(ReaderRef::Default).await
    }

    /// List packages using one explicit reader selector.
    pub async fn list_packages_on(&self, reader: ReaderRef) -> Result<CardPackageInventory> {
        let response = CardServiceClient::new(self.channel.clone())
            .list_packages(ListPackagesRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(CardPackageInventory {
            reader_name: response.reader_name,
            packages: response
                .packages
                .into_iter()
                .map(|package| CardPackageSummary {
                    aid: package.aid,
                    description: package.description,
                })
                .collect(),
            output_lines: response.output_lines,
        })
    }

    /// List applets using the configured default reader.
    pub async fn list_applets(&self) -> Result<CardAppletInventory> {
        self.list_applets_on(ReaderRef::Default).await
    }

    /// List applets using one explicit reader selector.
    pub async fn list_applets_on(&self, reader: ReaderRef) -> Result<CardAppletInventory> {
        let response = CardServiceClient::new(self.channel.clone())
            .list_applets(ListAppletsRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(CardAppletInventory {
            reader_name: response.reader_name,
            applets: response
                .applets
                .into_iter()
                .map(|applet| CardAppletSummary {
                    aid: applet.aid,
                    description: applet.description,
                })
                .collect(),
            output_lines: response.output_lines,
        })
    }

    /// Send one APDU using the configured default reader.
    pub async fn transmit_card_apdu(&self, apdu: &CommandApdu) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(apdu, ReaderRef::Default).await
    }

    /// Send one APDU using one explicit reader selector.
    pub async fn transmit_card_apdu_on(
        &self,
        apdu: &CommandApdu,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        let response = CardServiceClient::new(self.channel.clone())
            .transmit_apdu(CardApduRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                command: Some(command_apdu_frame(apdu)),
            })
            .await?
            .into_inner()
            .response;
        response_apdu_from_proto(response)
    }

    /// Fetch the current tracked ISO/IEC 7816 session state using the configured default reader.
    pub async fn get_card_session_state(&self) -> Result<IsoSessionState> {
        self.get_card_session_state_on(ReaderRef::Default).await
    }

    /// Fetch the current tracked ISO/IEC 7816 session state using one explicit reader.
    pub async fn get_card_session_state_on(&self, reader: ReaderRef) -> Result<IsoSessionState> {
        let response = CardServiceClient::new(self.channel.clone())
            .get_session_state(CardSelector {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        iso_session_state_from_proto(response.session_state)
    }

    /// Send one raw APDU byte sequence using the configured default reader.
    pub async fn transmit_raw_card_apdu(&self, apdu: &[u8]) -> Result<ApduExchangeSummary> {
        self.transmit_raw_card_apdu_on(apdu, ReaderRef::Default)
            .await
    }

    /// Send one raw APDU byte sequence using one explicit reader.
    pub async fn transmit_raw_card_apdu_on(
        &self,
        apdu: &[u8],
        reader: ReaderRef,
    ) -> Result<ApduExchangeSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .transmit_raw_apdu(CardRawApduRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                apdu: apdu.to_vec(),
            })
            .await?
            .into_inner();
        Ok(ApduExchangeSummary {
            command: CommandApdu::parse(&response.apdu)?,
            response: response_apdu_from_proto(response.response)?,
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Open or close one logical channel using the configured default reader.
    pub async fn manage_card_channel(
        &self,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<ManageChannelSummary> {
        self.manage_card_channel_on(open, channel_number, ReaderRef::Default)
            .await
    }

    /// Open or close one logical channel using one explicit reader.
    pub async fn manage_card_channel_on(
        &self,
        open: bool,
        channel_number: Option<u8>,
        reader: ReaderRef,
    ) -> Result<ManageChannelSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .manage_channel(CardManageChannelRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                open,
                channel_number: channel_number.map(u32::from),
            })
            .await?
            .into_inner();
        Ok(ManageChannelSummary {
            channel_number: response.channel_number.map(|value| value as u8),
            response: response_apdu_from_proto(response.response)?,
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Mark secure messaging as active using the configured default reader.
    pub async fn open_card_secure_messaging(
        &self,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<SecureMessagingSummary> {
        self.open_card_secure_messaging_on(protocol, security_level, session_id, ReaderRef::Default)
            .await
    }

    /// Mark secure messaging as active using one explicit reader.
    pub async fn open_card_secure_messaging_on(
        &self,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
        reader: ReaderRef,
    ) -> Result<SecureMessagingSummary> {
        let (protocol, protocol_label) = secure_messaging_protocol_fields(protocol.as_ref());
        let response = CardServiceClient::new(self.channel.clone())
            .open_secure_messaging(CardSecureMessagingRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                protocol,
                security_level: security_level.map(u32::from),
                session_id: session_id.unwrap_or_default(),
                protocol_label,
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Advance the secure-messaging command counter using the configured default reader.
    pub async fn advance_card_secure_messaging(
        &self,
        increment_by: u32,
    ) -> Result<SecureMessagingSummary> {
        self.advance_card_secure_messaging_on(increment_by, ReaderRef::Default)
            .await
    }

    /// Advance the secure-messaging command counter using one explicit reader.
    pub async fn advance_card_secure_messaging_on(
        &self,
        increment_by: u32,
        reader: ReaderRef,
    ) -> Result<SecureMessagingSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .advance_secure_messaging(CardSecureMessagingAdvanceRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                increment_by,
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Clear the tracked secure-messaging state using the configured default reader.
    pub async fn close_card_secure_messaging(&self) -> Result<SecureMessagingSummary> {
        self.close_card_secure_messaging_on(ReaderRef::Default)
            .await
    }

    /// Clear the tracked secure-messaging state using one explicit reader.
    pub async fn close_card_secure_messaging_on(
        &self,
        reader: ReaderRef,
    ) -> Result<SecureMessagingSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .close_secure_messaging(CardSelector {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Open one typed GP secure channel using the configured default reader.
    pub async fn open_gp_secure_channel_on_card(
        &self,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
    ) -> Result<GpSecureChannelSummary> {
        self.open_gp_secure_channel_on_card_with_reader(
            keyset_name,
            security_level,
            ReaderRef::Default,
        )
        .await
    }

    /// Open one typed GP secure channel using one explicit reader.
    pub async fn open_gp_secure_channel_on_card_with_reader(
        &self,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
        reader: ReaderRef,
    ) -> Result<GpSecureChannelSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .open_gp_secure_channel(OpenCardGpSecureChannelRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                keyset_name: keyset_name.unwrap_or_default().to_string(),
                security_level: security_level.map(u32::from),
            })
            .await?
            .into_inner();
        gp_secure_channel_from_proto(response.secure_channel)
    }

    /// Close one typed GP secure channel using the configured default reader.
    pub async fn close_gp_secure_channel_on_card(&self) -> Result<SecureMessagingSummary> {
        self.close_gp_secure_channel_on_card_with_reader(ReaderRef::Default)
            .await
    }

    /// Close one typed GP secure channel using one explicit reader.
    pub async fn close_gp_secure_channel_on_card_with_reader(
        &self,
        reader: ReaderRef,
    ) -> Result<SecureMessagingSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .close_gp_secure_channel(CardSelector {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Send one ISO/IEC 7816 `SELECT` by application identifier using the configured default reader.
    pub async fn iso_select_application_on_card(&self, aid: &Aid) -> Result<ResponseApdu> {
        self.iso_select_application_on_card_with_reader(aid, ReaderRef::Default)
            .await
    }

    /// Send one ISO/IEC 7816 `SELECT` by application identifier using one explicit reader.
    pub async fn iso_select_application_on_card_with_reader(
        &self,
        aid: &Aid,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(&iso7816::select_by_name(aid), reader)
            .await
    }

    /// Send one GlobalPlatform `SELECT` for the issuer security domain using the configured default reader.
    pub async fn gp_select_issuer_security_domain_on_card(&self) -> Result<ResponseApdu> {
        self.gp_select_issuer_security_domain_on_card_with_reader(ReaderRef::Default)
            .await
    }

    /// Send one GlobalPlatform `SELECT` for the issuer security domain using one explicit reader.
    pub async fn gp_select_issuer_security_domain_on_card_with_reader(
        &self,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(&globalplatform::select_issuer_security_domain(), reader)
            .await
    }

    /// Run one typed GlobalPlatform `GET STATUS` request using the configured default reader.
    pub async fn gp_get_status_on_card(
        &self,
        kind: globalplatform::RegistryKind,
        occurrence: globalplatform::GetStatusOccurrence,
    ) -> Result<globalplatform::GetStatusResponse> {
        self.gp_get_status_on_card_with_reader(kind, occurrence, ReaderRef::Default)
            .await
    }

    /// Run one typed GlobalPlatform `GET STATUS` request using one explicit reader.
    pub async fn gp_get_status_on_card_with_reader(
        &self,
        kind: globalplatform::RegistryKind,
        occurrence: globalplatform::GetStatusOccurrence,
        reader: ReaderRef,
    ) -> Result<globalplatform::GetStatusResponse> {
        let response = self
            .transmit_card_apdu_on(&globalplatform::get_status(kind, occurrence), reader)
            .await?;
        Ok(globalplatform::parse_get_status(kind, &response)?)
    }

    /// Set one GlobalPlatform card life cycle state using the configured default reader.
    pub async fn gp_set_card_status_on_card(
        &self,
        state: globalplatform::CardLifeCycle,
    ) -> Result<ResponseApdu> {
        self.gp_set_card_status_on_card_with_reader(state, ReaderRef::Default)
            .await
    }

    /// Set one GlobalPlatform card life cycle state using one explicit reader.
    pub async fn gp_set_card_status_on_card_with_reader(
        &self,
        state: globalplatform::CardLifeCycle,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(&globalplatform::set_card_status(state), reader)
            .await
    }

    /// Lock or unlock one application using the configured default reader.
    pub async fn gp_set_application_status_on_card(
        &self,
        aid: &Aid,
        transition: globalplatform::LockTransition,
    ) -> Result<ResponseApdu> {
        self.gp_set_application_status_on_card_with_reader(aid, transition, ReaderRef::Default)
            .await
    }

    /// Lock or unlock one application using one explicit reader.
    pub async fn gp_set_application_status_on_card_with_reader(
        &self,
        aid: &Aid,
        transition: globalplatform::LockTransition,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(
            &globalplatform::set_application_status(aid, transition),
            reader,
        )
        .await
    }

    /// Lock or unlock one security domain and its applications using the configured default reader.
    pub async fn gp_set_security_domain_status_on_card(
        &self,
        aid: &Aid,
        transition: globalplatform::LockTransition,
    ) -> Result<ResponseApdu> {
        self.gp_set_security_domain_status_on_card_with_reader(aid, transition, ReaderRef::Default)
            .await
    }

    /// Lock or unlock one security domain and its applications using one explicit reader.
    pub async fn gp_set_security_domain_status_on_card_with_reader(
        &self,
        aid: &Aid,
        transition: globalplatform::LockTransition,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(
            &globalplatform::set_security_domain_status(aid, transition),
            reader,
        )
        .await
    }

    /// Reset the configured default reader and return the typed reset summary.
    pub async fn reset_card_summary(&self) -> Result<ResetSummary> {
        self.reset_card_summary_on(ReaderRef::Default).await
    }

    /// Reset one explicit reader and return the typed reset summary.
    pub async fn reset_card_summary_on(&self, reader: ReaderRef) -> Result<ResetSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .reset_card(ResetCardRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        reset_summary_from_card_proto(response)
    }

    /// Persist machine-local toolchain settings.
    pub async fn setup_toolchains(&self, java_bin: Option<&str>) -> Result<SetupSummary> {
        let response = SystemServiceClient::new(self.channel.clone())
            .setup_toolchains(SetupToolchainsRequest {
                java_bin: java_bin.unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(SetupSummary {
            config_path: owned_path(response.config_path),
            message: response.message,
        })
    }

    /// Return a doctor report for the current environment.
    pub async fn doctor(&self) -> Result<Vec<String>> {
        Ok(SystemServiceClient::new(self.channel.clone())
            .doctor(Empty {})
            .await?
            .into_inner()
            .lines)
    }

    /// Return current service status without mutating service state.
    pub async fn service_status(&self) -> Result<ServiceStatusSummary> {
        let GetServiceStatusResponse {
            socket_path,
            running,
            known_project_count,
            active_simulation_count,
            service_binary_path,
            service_binary_fingerprint,
        } = SystemServiceClient::new(self.channel.clone())
            .get_service_status(Empty {})
            .await?
            .into_inner();
        Ok(ServiceStatusSummary {
            socket_path: owned_path(socket_path),
            running,
            known_project_count,
            active_simulation_count,
            service_binary_path: owned_path(service_binary_path),
            service_binary_fingerprint,
        })
    }
}

impl JcimClient {
    async fn connected_service_matches_current_binary(&self) -> Result<bool> {
        let status = match self.service_status().await {
            Ok(status) => status,
            Err(JcimSdkError::Status(status)) if status.code() == Code::Unimplemented => {
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        let expected_identity = local_service_binary_identity(&service_binary_path()?)?;
        Ok(service_status_matches_binary(&status, &expected_identity))
    }

    async fn validated_card_status_for_connection(
        &self,
        reader: ReaderRef,
    ) -> Result<CardStatusSummary> {
        if let ReaderRef::Named(reader_name) = &reader
            && reader_name.trim().is_empty()
        {
            return Err(invalid_connection_target(
                "reader connection requires a non-empty reader name".to_string(),
            ));
        }
        let status = self.get_card_status_on(reader).await?;
        if !status.card_present {
            let reader_name = status.reader_name.trim();
            let message = if reader_name.is_empty() {
                "reader connection requires a present card".to_string()
            } else {
                format!("reader `{reader_name}` has no present card")
            };
            return Err(invalid_connection_target(message));
        }
        Ok(status)
    }

    async fn validated_running_simulation(
        &self,
        simulation: SimulationRef,
    ) -> Result<SimulationSummary> {
        if simulation.simulation_id.trim().is_empty() {
            return Err(invalid_connection_target(
                "simulation connection requires a non-empty simulation id".to_string(),
            ));
        }
        let summary = self.get_simulation(simulation).await?;
        if summary.status != SimulationStatus::Running {
            return Err(invalid_connection_target(format!(
                "simulation `{}` is not running; current status is {:?}",
                summary.simulation_id, summary.status
            )));
        }
        Ok(summary)
    }
}

async fn connect_channel(
    socket_path: &Path,
) -> std::result::Result<Channel, tonic::transport::Error> {
    let socket_path = socket_path.to_path_buf();
    Endpoint::try_from("http://[::]:50051")?
        .connect_with_connector(service_fn(move |_| {
            let socket_path = socket_path.clone();
            async move { UnixStream::connect(socket_path).await.map(TokioIo::new) }
        }))
        .await
}

fn invalid_connection_target(message: String) -> JcimSdkError {
    JcimError::Unsupported(message).into()
}

struct SpawnedService {
    child: std::process::Child,
    stderr_log_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ServiceBinaryIdentity {
    path: PathBuf,
    fingerprint: String,
}

fn spawn_service(managed_paths: &ManagedPaths) -> Result<SpawnedService> {
    let binary = service_binary_path()?;
    std::fs::create_dir_all(&managed_paths.log_dir)?;
    let stderr_log_path = managed_paths.log_dir.join("jcimd-bootstrap.stderr.log");
    let stderr_file = std::fs::File::create(&stderr_log_path).map_err(|error| {
        JcimSdkError::Bootstrap(format!(
            "unable to create jcimd bootstrap log at {}: {error}",
            stderr_log_path.display()
        ))
    })?;
    Command::new(binary)
        .arg("--socket-path")
        .arg(&managed_paths.service_socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map(|child| SpawnedService {
            child,
            stderr_log_path,
        })
        .map_err(|error| JcimSdkError::Bootstrap(format!("unable to launch jcimd: {error}")))
}

fn read_bootstrap_log_tail(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn disconnect_stale_service_socket(managed_paths: &ManagedPaths) -> Result<()> {
    best_effort_terminate_stale_socket_owners(&managed_paths.service_socket_path);
    match std::fs::remove_file(&managed_paths.service_socket_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn best_effort_terminate_stale_socket_owners(socket_path: &Path) {
    let output = match Command::new("lsof")
        .arg("-t")
        .arg(socket_path)
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(_) => return,
    };
    if !output.status.success() {
        return;
    }

    let current_pid = std::process::id();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(pid) = line.trim().parse::<u32>() else {
            continue;
        };
        if pid == current_pid {
            continue;
        }
        let _ = Command::new("kill")
            .arg(pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn local_service_binary_identity(path: &Path) -> Result<ServiceBinaryIdentity> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(ServiceBinaryIdentity {
        path: path.to_path_buf(),
        fingerprint: format!(
            "{}:{}:{}",
            metadata.len(),
            modified.as_secs(),
            modified.subsec_nanos()
        ),
    })
}

fn service_status_matches_binary(
    status: &ServiceStatusSummary,
    identity: &ServiceBinaryIdentity,
) -> bool {
    status.service_binary_path == identity.path
        && !status.service_binary_fingerprint.trim().is_empty()
        && status.service_binary_fingerprint == identity.fingerprint
}

fn service_binary_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("JCIM_SERVICE_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_jcimd") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }

    let current = std::env::current_exe()?;
    for candidate in binary_candidates(&current, "jcimd") {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(JcimSdkError::Bootstrap(format!(
        "unable to find jcimd near {} or from JCIM_SERVICE_BIN",
        current.display()
    )))
}

fn binary_candidates(current_exe: &Path, name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(parent) = current_exe.parent() {
        candidates.push(parent.join(name));
        if let Some(grandparent) = parent.parent() {
            candidates.push(grandparent.join(name));
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join(format!("{name}.exe")));
            if let Some(grandparent) = parent.parent() {
                candidates.push(grandparent.join(format!("{name}.exe")));
            }
        }
    }
    candidates
}

fn project_selector(project: &ProjectRef) -> ProjectSelector {
    ProjectSelector {
        project_path: project
            .project_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        project_id: project.project_id.clone().unwrap_or_default(),
    }
}

fn project_summary(project: jcim_api::v0_2::ProjectInfo) -> Result<ProjectSummary> {
    Ok(ProjectSummary {
        project_id: project.project_id,
        name: project.name,
        project_path: owned_path(project.project_path),
        profile: project.profile,
        build_kind: project.build_kind,
        package_name: project.package_name,
        package_aid: project.package_aid,
        applets: project
            .applets
            .into_iter()
            .map(|applet| AppletSummary {
                class_name: applet.class_name,
                aid: applet.aid,
            })
            .collect(),
    })
}

fn artifact_summary(artifact: jcim_api::v0_2::Artifact) -> Result<ArtifactSummary> {
    Ok(ArtifactSummary {
        kind: artifact.kind,
        path: owned_path(artifact.path),
    })
}

fn simulation_summary(simulation: jcim_api::v0_2::SimulationInfo) -> Result<SimulationSummary> {
    Ok(SimulationSummary {
        simulation_id: simulation.simulation_id,
        source_kind: match jcim_api::v0_2::SimulationSourceKind::try_from(simulation.source_kind) {
            Ok(jcim_api::v0_2::SimulationSourceKind::Project) => SimulationSourceKind::Project,
            Ok(jcim_api::v0_2::SimulationSourceKind::Cap) => SimulationSourceKind::Cap,
            _ => SimulationSourceKind::Unknown,
        },
        project_id: (!simulation.project_id.is_empty()).then_some(simulation.project_id),
        project_path: (!simulation.project_path.is_empty())
            .then(|| owned_path(simulation.project_path)),
        cap_path: owned_path(simulation.cap_path),
        engine_mode: match jcim_api::v0_2::SimulationEngineMode::try_from(simulation.engine_mode) {
            Ok(jcim_api::v0_2::SimulationEngineMode::Native) => SimulationEngineMode::Native,
            Ok(jcim_api::v0_2::SimulationEngineMode::Container) => SimulationEngineMode::Container,
            Ok(jcim_api::v0_2::SimulationEngineMode::ManagedJava) => {
                SimulationEngineMode::ManagedJava
            }
            _ => SimulationEngineMode::Unknown,
        },
        status: match jcim_api::v0_2::SimulationStatus::try_from(simulation.status) {
            Ok(jcim_api::v0_2::SimulationStatus::Starting) => SimulationStatus::Starting,
            Ok(jcim_api::v0_2::SimulationStatus::Running) => SimulationStatus::Running,
            Ok(jcim_api::v0_2::SimulationStatus::Stopped) => SimulationStatus::Stopped,
            Ok(jcim_api::v0_2::SimulationStatus::Failed) => SimulationStatus::Failed,
            _ => SimulationStatus::Unknown,
        },
        reader_name: simulation.reader_name,
        health: simulation.health,
        atr: atr_from_proto(simulation.atr)?,
        active_protocol: protocol_parameters_from_proto(simulation.active_protocol),
        iso_capabilities: iso_capabilities_from_proto(simulation.iso_capabilities),
        session_state: iso_session_state_from_proto(simulation.session_state)?,
        package_count: simulation.package_count,
        applet_count: simulation.applet_count,
        package_name: simulation.package_name,
        package_aid: simulation.package_aid,
        recent_events: simulation.recent_events,
    })
}

fn secure_messaging_protocol_fields(protocol: Option<&SecureMessagingProtocol>) -> (i32, String) {
    match protocol {
        Some(SecureMessagingProtocol::Iso7816) => (
            jcim_api::v0_2::SecureMessagingProtocol::Iso7816 as i32,
            String::new(),
        ),
        Some(SecureMessagingProtocol::Scp02) => (
            jcim_api::v0_2::SecureMessagingProtocol::Scp02 as i32,
            String::new(),
        ),
        Some(SecureMessagingProtocol::Scp03) => (
            jcim_api::v0_2::SecureMessagingProtocol::Scp03 as i32,
            String::new(),
        ),
        Some(SecureMessagingProtocol::Other(label)) => (
            jcim_api::v0_2::SecureMessagingProtocol::Other as i32,
            label.clone(),
        ),
        None => (
            jcim_api::v0_2::SecureMessagingProtocol::Unspecified as i32,
            String::new(),
        ),
    }
}

fn command_apdu_frame(apdu: &CommandApdu) -> jcim_api::v0_2::CommandApduFrame {
    let descriptor = iso7816::describe_command(apdu);
    jcim_api::v0_2::CommandApduFrame {
        raw: apdu.to_bytes(),
        cla: u32::from(apdu.cla),
        ins: u32::from(apdu.ins),
        p1: u32::from(apdu.p1),
        p2: u32::from(apdu.p2),
        data: apdu.data.clone(),
        ne: apdu.ne.map(|value| value as u32),
        encoding: match apdu.encoding {
            jcim_core::apdu::ApduEncoding::Short => jcim_api::v0_2::ApduEncoding::Short as i32,
            jcim_core::apdu::ApduEncoding::Extended => {
                jcim_api::v0_2::ApduEncoding::Extended as i32
            }
        },
        apdu_case: match apdu.apdu_case() {
            jcim_core::apdu::CommandApduCase::Case1 => {
                jcim_api::v0_2::CommandApduCase::CommandApduCase1 as i32
            }
            jcim_core::apdu::CommandApduCase::Case2Short => {
                jcim_api::v0_2::CommandApduCase::CommandApduCase2Short as i32
            }
            jcim_core::apdu::CommandApduCase::Case3Short => {
                jcim_api::v0_2::CommandApduCase::CommandApduCase3Short as i32
            }
            jcim_core::apdu::CommandApduCase::Case4Short => {
                jcim_api::v0_2::CommandApduCase::CommandApduCase4Short as i32
            }
            jcim_core::apdu::CommandApduCase::Case2Extended => {
                jcim_api::v0_2::CommandApduCase::CommandApduCase2Extended as i32
            }
            jcim_core::apdu::CommandApduCase::Case3Extended => {
                jcim_api::v0_2::CommandApduCase::CommandApduCase3Extended as i32
            }
            jcim_core::apdu::CommandApduCase::Case4Extended => {
                jcim_api::v0_2::CommandApduCase::CommandApduCase4Extended as i32
            }
        },
        domain: match descriptor.domain {
            CommandDomain::Iso7816 => jcim_api::v0_2::CommandDomain::Iso7816 as i32,
            CommandDomain::GlobalPlatform => jcim_api::v0_2::CommandDomain::GlobalPlatform as i32,
            CommandDomain::Opaque => jcim_api::v0_2::CommandDomain::Opaque as i32,
        },
        kind: match descriptor.kind {
            CommandKind::Select => jcim_api::v0_2::CommandKind::Select as i32,
            CommandKind::ManageChannel => jcim_api::v0_2::CommandKind::ManageChannel as i32,
            CommandKind::GetResponse => jcim_api::v0_2::CommandKind::GetResponse as i32,
            CommandKind::ReadBinary => jcim_api::v0_2::CommandKind::ReadBinary as i32,
            CommandKind::WriteBinary => jcim_api::v0_2::CommandKind::WriteBinary as i32,
            CommandKind::UpdateBinary => jcim_api::v0_2::CommandKind::UpdateBinary as i32,
            CommandKind::EraseBinary => jcim_api::v0_2::CommandKind::EraseBinary as i32,
            CommandKind::ReadRecord => jcim_api::v0_2::CommandKind::ReadRecord as i32,
            CommandKind::UpdateRecord => jcim_api::v0_2::CommandKind::UpdateRecord as i32,
            CommandKind::AppendRecord => jcim_api::v0_2::CommandKind::AppendRecord as i32,
            CommandKind::SearchRecord => jcim_api::v0_2::CommandKind::SearchRecord as i32,
            CommandKind::GetData => jcim_api::v0_2::CommandKind::GetData as i32,
            CommandKind::PutData => jcim_api::v0_2::CommandKind::PutData as i32,
            CommandKind::Verify => jcim_api::v0_2::CommandKind::Verify as i32,
            CommandKind::ChangeReferenceData => {
                jcim_api::v0_2::CommandKind::ChangeReferenceData as i32
            }
            CommandKind::ResetRetryCounter => jcim_api::v0_2::CommandKind::ResetRetryCounter as i32,
            CommandKind::InternalAuthenticate => {
                jcim_api::v0_2::CommandKind::InternalAuthenticate as i32
            }
            CommandKind::ExternalAuthenticate => {
                jcim_api::v0_2::CommandKind::ExternalAuthenticate as i32
            }
            CommandKind::GetChallenge => jcim_api::v0_2::CommandKind::GetChallenge as i32,
            CommandKind::Envelope => jcim_api::v0_2::CommandKind::Envelope as i32,
            CommandKind::GpGetStatus => jcim_api::v0_2::CommandKind::GpGetStatus as i32,
            CommandKind::GpSetStatus => jcim_api::v0_2::CommandKind::GpSetStatus as i32,
            CommandKind::GpInitializeUpdate => {
                jcim_api::v0_2::CommandKind::GpInitializeUpdate as i32
            }
            CommandKind::GpExternalAuthenticate => {
                jcim_api::v0_2::CommandKind::GpExternalAuthenticate as i32
            }
            CommandKind::Opaque => jcim_api::v0_2::CommandKind::Opaque as i32,
        },
        logical_channel: u32::from(descriptor.logical_channel),
    }
}

fn response_apdu_from_proto(
    frame: Option<jcim_api::v0_2::ResponseApduFrame>,
) -> Result<ResponseApdu> {
    let frame = frame.ok_or_else(|| {
        JcimSdkError::InvalidResponse("service returned no response APDU".to_string())
    })?;
    if !frame.raw.is_empty() {
        return Ok(ResponseApdu::parse(&frame.raw)?);
    }
    Ok(ResponseApdu {
        data: frame.data,
        sw: frame.sw as u16,
    })
}

fn reset_summary_from_simulation_proto(response: ResetSimulationResponse) -> Result<ResetSummary> {
    let atr = atr_from_proto(response.atr)?;
    let session_state = iso_session_state_from_proto(response.session_state)?;
    Ok(ResetSummary {
        atr: atr.or_else(|| session_state.atr.clone()),
        session_state,
    })
}

fn reset_summary_from_card_proto(response: ResetCardResponse) -> Result<ResetSummary> {
    let atr = atr_from_proto(response.atr)?;
    let session_state = iso_session_state_from_proto(response.session_state)?;
    Ok(ResetSummary {
        atr: atr.or_else(|| session_state.atr.clone()),
        session_state,
    })
}

fn gp_secure_channel_from_proto(
    info: Option<jcim_api::v0_2::GpSecureChannelInfo>,
) -> Result<GpSecureChannelSummary> {
    let info = info.ok_or_else(|| {
        JcimSdkError::InvalidResponse(
            "missing GP secure-channel summary in service response".to_string(),
        )
    })?;
    let protocol = match jcim_api::v0_2::SecureMessagingProtocol::try_from(info.protocol).ok() {
        Some(jcim_api::v0_2::SecureMessagingProtocol::Scp02) => globalplatform::ScpMode::Scp02,
        Some(jcim_api::v0_2::SecureMessagingProtocol::Scp03) => globalplatform::ScpMode::Scp03,
        _ => {
            return Err(JcimSdkError::InvalidResponse(
                "service returned a non-GP secure-messaging protocol for GP auth".to_string(),
            ));
        }
    };
    Ok(GpSecureChannelSummary {
        secure_channel: globalplatform::EstablishedSecureChannel {
            keyset: globalplatform::GpKeysetMetadata {
                name: info.keyset_name,
                mode: protocol,
            },
            security_level: globalplatform::SecurityLevel::Raw(info.security_level as u8),
            session_id: info.session_id,
        },
        selected_aid: aid_from_proto(info.selected_aid)?.ok_or_else(|| {
            JcimSdkError::InvalidResponse(
                "service omitted GP secure-channel selected AID".to_string(),
            )
        })?,
        session_state: iso_session_state_from_proto(info.session_state)?,
    })
}

fn atr_from_proto(info: Option<jcim_api::v0_2::AtrInfo>) -> Result<Option<Atr>> {
    info.map(|value| Atr::parse(&value.raw).map_err(JcimSdkError::from))
        .transpose()
}

fn protocol_parameters_from_proto(
    info: Option<jcim_api::v0_2::ProtocolParametersInfo>,
) -> Option<ProtocolParameters> {
    let info = info?;
    Some(ProtocolParameters {
        protocol: transport_protocol_from_proto(info.protocol),
        fi: info.fi.map(|value| value as u8),
        di: info.di.map(|value| value as u8),
        waiting_integer: info.waiting_integer.map(|value| value as u8),
        ifsc: info.ifsc.map(|value| value as u8),
    })
}

fn iso_capabilities_from_proto(
    info: Option<jcim_api::v0_2::IsoCapabilitiesInfo>,
) -> IsoCapabilities {
    let Some(info) = info else {
        return IsoCapabilities::default();
    };
    IsoCapabilities {
        protocols: info
            .protocols
            .into_iter()
            .filter_map(transport_protocol_from_proto)
            .collect(),
        extended_length: info.extended_length,
        logical_channels: info.logical_channels,
        max_logical_channels: info.max_logical_channels as u8,
        secure_messaging: info.secure_messaging,
        file_model_visibility: info.file_model_visibility,
        raw_apdu: info.raw_apdu,
    }
}

fn iso_session_state_from_proto(
    info: Option<jcim_api::v0_2::IsoSessionStateInfo>,
) -> Result<IsoSessionState> {
    let Some(info) = info else {
        return Ok(IsoSessionState::default());
    };
    Ok(IsoSessionState {
        power_state: match jcim_api::v0_2::PowerState::try_from(info.power_state) {
            Ok(jcim_api::v0_2::PowerState::On) => PowerState::On,
            _ => PowerState::Off,
        },
        atr: atr_from_proto(info.atr)?,
        active_protocol: protocol_parameters_from_proto(info.active_protocol),
        selected_aid: aid_from_proto(info.selected_aid)?,
        current_file: file_selection_from_proto(info.current_file),
        open_channels: info
            .open_channels
            .into_iter()
            .map(|entry| -> Result<LogicalChannelState> {
                Ok(LogicalChannelState {
                    channel_number: entry.channel_number as u8,
                    selected_aid: aid_from_proto(entry.selected_aid)?,
                    current_file: file_selection_from_proto(entry.current_file),
                })
            })
            .collect::<Result<Vec<_>>>()?,
        secure_messaging: SecureMessagingState {
            active: info
                .secure_messaging
                .as_ref()
                .is_some_and(|state| state.active),
            protocol: info.secure_messaging.as_ref().and_then(|state| {
                secure_messaging_protocol_from_proto(state.protocol, &state.protocol_label)
            }),
            security_level: info
                .secure_messaging
                .as_ref()
                .and_then(|state| state.security_level.map(|value| value as u8)),
            session_id: info.secure_messaging.as_ref().and_then(|state| {
                (!state.session_id.is_empty()).then_some(state.session_id.clone())
            }),
            command_counter: info
                .secure_messaging
                .as_ref()
                .map(|state| state.command_counter)
                .unwrap_or_default(),
        },
        verified_references: info
            .verified_references
            .into_iter()
            .map(|value| value as u8)
            .collect(),
        retry_counters: info
            .retry_counters
            .into_iter()
            .map(|counter| RetryCounterState {
                reference: counter.reference as u8,
                remaining: counter.remaining as u8,
            })
            .collect(),
        last_status: info
            .last_status
            .as_ref()
            .map(|status| StatusWord::new(status.value as u16)),
    })
}

fn aid_from_proto(info: Option<jcim_api::v0_2::AidInfo>) -> Result<Option<Aid>> {
    let Some(info) = info else {
        return Ok(None);
    };
    if info.raw.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Aid::from_slice(&info.raw)?))
    }
}

fn file_selection_from_proto(
    info: Option<jcim_api::v0_2::FileSelectionInfo>,
) -> Option<FileSelection> {
    use jcim_api::v0_2::file_selection_info::Selection;

    match info.and_then(|info| info.selection) {
        Some(Selection::ByName(bytes)) => Some(FileSelection::ByName(bytes)),
        Some(Selection::FileId(file_id)) => Some(FileSelection::FileId(file_id as u16)),
        Some(Selection::Path(bytes)) => Some(FileSelection::Path(bytes)),
        None => None,
    }
}

fn transport_protocol_from_proto(value: i32) -> Option<TransportProtocol> {
    match jcim_api::v0_2::TransportProtocol::try_from(value).ok()? {
        jcim_api::v0_2::TransportProtocol::T0 => Some(TransportProtocol::T0),
        jcim_api::v0_2::TransportProtocol::T1 => Some(TransportProtocol::T1),
        jcim_api::v0_2::TransportProtocol::T2 => Some(TransportProtocol::T2),
        jcim_api::v0_2::TransportProtocol::T3 => Some(TransportProtocol::T3),
        jcim_api::v0_2::TransportProtocol::T14 => Some(TransportProtocol::T14),
        jcim_api::v0_2::TransportProtocol::Other => Some(TransportProtocol::Other(0xFF)),
        jcim_api::v0_2::TransportProtocol::Unspecified => None,
    }
}

fn secure_messaging_protocol_from_proto(
    value: i32,
    label: &str,
) -> Option<SecureMessagingProtocol> {
    match jcim_api::v0_2::SecureMessagingProtocol::try_from(value).ok()? {
        jcim_api::v0_2::SecureMessagingProtocol::Iso7816 => Some(SecureMessagingProtocol::Iso7816),
        jcim_api::v0_2::SecureMessagingProtocol::Scp02 => Some(SecureMessagingProtocol::Scp02),
        jcim_api::v0_2::SecureMessagingProtocol::Scp03 => Some(SecureMessagingProtocol::Scp03),
        jcim_api::v0_2::SecureMessagingProtocol::Other => {
            Some(SecureMessagingProtocol::Other(label.to_string()))
        }
        jcim_api::v0_2::SecureMessagingProtocol::Unspecified => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::types::ServiceStatusSummary;

    use super::{ServiceBinaryIdentity, binary_candidates, service_status_matches_binary};

    #[test]
    fn binary_candidates_check_parent_and_grandparent() {
        let candidates = binary_candidates(Path::new("/tmp/target/debug/examples/demo"), "jcimd");
        assert!(candidates.contains(&PathBuf::from("/tmp/target/debug/examples/jcimd")));
        assert!(candidates.contains(&PathBuf::from("/tmp/target/debug/jcimd")));
    }

    #[test]
    fn service_status_requires_matching_binary_identity() {
        let identity = ServiceBinaryIdentity {
            path: PathBuf::from("/tmp/jcimd"),
            fingerprint: "123:456:789".to_string(),
        };
        let matching = ServiceStatusSummary {
            socket_path: PathBuf::from("/tmp/jcimd.sock"),
            running: true,
            known_project_count: 0,
            active_simulation_count: 0,
            service_binary_path: identity.path.clone(),
            service_binary_fingerprint: identity.fingerprint.clone(),
        };
        let missing_fingerprint = ServiceStatusSummary {
            service_binary_fingerprint: String::new(),
            ..matching.clone()
        };
        let wrong_path = ServiceStatusSummary {
            service_binary_path: PathBuf::from("/tmp/other-jcimd"),
            ..matching.clone()
        };

        assert!(service_status_matches_binary(&matching, &identity));
        assert!(!service_status_matches_binary(
            &missing_fingerprint,
            &identity
        ));
        assert!(!service_status_matches_binary(&wrong_path, &identity));
    }
}
