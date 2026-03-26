//! Local gRPC control plane for JCIM 0.2.
//!
//! # Why this exists
//! JCIM 0.2 uses one user-local service as its control plane. This crate hosts the local gRPC
//! server that exposes the task-oriented API consumed by the CLI and future desktop UI.
#![allow(clippy::missing_docs_in_private_items)]
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::pin::Pin;

use tokio::net::UnixListener;
use tokio_stream::Stream;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use jcim_api::v0_2::build_service_server::{BuildService, BuildServiceServer};
use jcim_api::v0_2::card_service_server::{CardService, CardServiceServer};
use jcim_api::v0_2::file_selection_info::Selection as FileSelectionProto;
use jcim_api::v0_2::install_cap_request::Input as InstallCapInput;
use jcim_api::v0_2::project_service_server::{ProjectService, ProjectServiceServer};
use jcim_api::v0_2::simulator_service_server::{SimulatorService, SimulatorServiceServer};
use jcim_api::v0_2::system_service_server::{SystemService, SystemServiceServer};
use jcim_api::v0_2::workspace_service_server::{WorkspaceService, WorkspaceServiceServer};
use jcim_api::v0_2::{
    AidInfo, AppletInfo, Artifact, AtrInfo, AtrInterfaceGroup, BuildEvent, BuildProjectRequest,
    BuildProjectResponse, CardApduRequest, CardApduResponse, CardAppletInfo,
    CardManageChannelRequest, CardManageChannelResponse, CardPackageInfo, CardRawApduRequest,
    CardRawApduResponse, CardSecureMessagingAdvanceRequest, CardSecureMessagingRequest,
    CardSecureMessagingResponse, CardSelector, CardStatusRequest, CardStatusResponse,
    CleanProjectResponse, CommandApduFrame, CreateProjectRequest, CreateProjectResponse,
    DeleteItemRequest, DeleteItemResponse, DoctorResponse, Empty, FileSelectionInfo,
    GetArtifactsResponse, GetCardSessionStateResponse, GetOverviewResponse, GetProjectResponse,
    GetServiceStatusResponse, GetSimulationResponse, GetSimulationSessionStateResponse,
    GpSecureChannelInfo, InstallCapRequest, InstallCapResponse, IsoCapabilitiesInfo,
    IsoSessionStateInfo, ListAppletsRequest, ListAppletsResponse, ListPackagesRequest,
    ListPackagesResponse, ListProjectsResponse, ListReadersResponse, ListSimulationsResponse,
    LogicalChannelStateInfo, ManageChannelRequest, ManageChannelResponse,
    OpenCardGpSecureChannelRequest, OpenCardGpSecureChannelResponse, OpenGpSecureChannelRequest,
    OpenGpSecureChannelResponse, Overview, ProjectInfo, ProjectSelector, ProtocolParametersInfo,
    ReaderInfo, ResetCardRequest, ResetCardResponse, ResetSimulationResponse, ResponseApduFrame,
    RetryCounterInfo, SecureMessagingAdvanceRequest, SecureMessagingRequest,
    SecureMessagingResponse, SecureMessagingStateInfo, SetupToolchainsRequest,
    SetupToolchainsResponse, SimulationEngineMode, SimulationEvent, SimulationInfo,
    SimulationSelector, SimulationSourceKind, SimulationStatus, StartSimulationRequest,
    StartSimulationResponse, StatusWordInfo, StopSimulationResponse, TransmitApduRequest,
    TransmitApduResponse, TransmitRawApduRequest, TransmitRawApduResponse,
};
use jcim_app::{
    ArtifactSummary, CardAppletInventory, CardDeleteSummary, CardInstallSummary,
    CardPackageInventory, GpSecureChannelSummary, JcimApp, ProjectDetails, ProjectSelectorInput,
    ProjectSummary, ServiceStatusSummary, SimulationSelectorInput, SimulationSummary,
};
use jcim_core::aid::Aid;
use jcim_core::apdu::{ApduEncoding, CommandApdu, CommandApduCase, ResponseApdu};
use jcim_core::error::JcimError;
use jcim_core::iso7816::{
    self, Atr, FileSelection, IsoCapabilities, IsoSessionState, LogicalChannelState, PowerState,
    ProtocolParameters, RetryCounterState, SecureMessagingProtocol, SecureMessagingState,
    StatusWord, StatusWordClass, TransmissionConvention, TransportProtocol,
};

type RpcStream<T> = Pin<Box<dyn Stream<Item = std::result::Result<T, Status>> + Send + 'static>>;

/// Serve the local JCIM gRPC API over one Unix-domain socket.
pub async fn serve_local_service(app: JcimApp, socket_path: &Path) -> Result<(), JcimError> {
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    let rpc = LocalRpc { app };
    Server::builder()
        .add_service(WorkspaceServiceServer::new(rpc.clone()))
        .add_service(ProjectServiceServer::new(rpc.clone()))
        .add_service(BuildServiceServer::new(rpc.clone()))
        .add_service(SimulatorServiceServer::new(rpc.clone()))
        .add_service(CardServiceServer::new(rpc.clone()))
        .add_service(SystemServiceServer::new(rpc))
        .serve_with_incoming(UnixListenerStream::new(listener))
        .await
        .map_err(|error| JcimError::Unsupported(format!("gRPC server failed: {error}")))
}

#[derive(Clone)]
struct LocalRpc {
    app: JcimApp,
}

#[tonic::async_trait]
impl WorkspaceService for LocalRpc {
    async fn get_overview(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<GetOverviewResponse>, Status> {
        let app = self.app.clone();
        let overview = blocking(move || app.overview()).await?;
        Ok(Response::new(GetOverviewResponse {
            overview: Some(Overview {
                known_project_count: overview.known_project_count,
                active_simulation_count: overview.active_simulation_count,
            }),
        }))
    }

    async fn list_projects(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListProjectsResponse>, Status> {
        let app = self.app.clone();
        let projects = blocking(move || app.list_projects()).await?;
        Ok(Response::new(ListProjectsResponse {
            projects: projects.into_iter().map(project_info).collect(),
        }))
    }

    async fn list_simulations(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListSimulationsResponse>, Status> {
        let app = self.app.clone();
        let simulations = blocking(move || app.list_simulations()).await?;
        Ok(Response::new(ListSimulationsResponse {
            simulations: simulations.into_iter().map(simulation_info).collect(),
        }))
    }
}

#[tonic::async_trait]
impl ProjectService for LocalRpc {
    async fn create_project(
        &self,
        request: Request<CreateProjectRequest>,
    ) -> Result<Response<CreateProjectResponse>, Status> {
        let CreateProjectRequest { name, directory } = request.into_inner();
        let app = self.app.clone();
        let details = blocking(move || app.create_project(&name, Path::new(&directory))).await?;
        Ok(Response::new(CreateProjectResponse {
            project: Some(project_info(details.project)),
        }))
    }

    async fn get_project(
        &self,
        request: Request<ProjectSelector>,
    ) -> Result<Response<GetProjectResponse>, Status> {
        let selector = into_project_selector(request.into_inner());
        let app = self.app.clone();
        let details = blocking(move || app.get_project(&selector)).await?;
        Ok(Response::new(project_details_response(details)))
    }

    async fn clean_project(
        &self,
        request: Request<ProjectSelector>,
    ) -> Result<Response<CleanProjectResponse>, Status> {
        let selector = into_project_selector(request.into_inner());
        let app = self.app.clone();
        let cleaned_path = blocking(move || app.clean_project(&selector)).await?;
        Ok(Response::new(CleanProjectResponse {
            cleaned_path: cleaned_path.display().to_string(),
        }))
    }
}

#[tonic::async_trait]
impl BuildService for LocalRpc {
    type StreamBuildEventsStream = RpcStream<BuildEvent>;

    async fn build_project(
        &self,
        request: Request<BuildProjectRequest>,
    ) -> Result<Response<BuildProjectResponse>, Status> {
        let selector = into_project_selector(request.into_inner().project.unwrap_or_default());
        let app = self.app.clone();
        let (project, artifacts, rebuilt) = blocking(move || app.build_project(&selector)).await?;
        Ok(Response::new(BuildProjectResponse {
            project: Some(project_info(project)),
            artifacts: artifacts.into_iter().map(artifact_info).collect(),
            rebuilt,
        }))
    }

    async fn get_artifacts(
        &self,
        request: Request<ProjectSelector>,
    ) -> Result<Response<GetArtifactsResponse>, Status> {
        let selector = into_project_selector(request.into_inner());
        let app = self.app.clone();
        let (project, artifacts) = blocking(move || app.get_artifacts(&selector)).await?;
        Ok(Response::new(GetArtifactsResponse {
            project: Some(project_info(project)),
            artifacts: artifacts.into_iter().map(artifact_info).collect(),
        }))
    }

    async fn stream_build_events(
        &self,
        request: Request<ProjectSelector>,
    ) -> Result<Response<Self::StreamBuildEventsStream>, Status> {
        let selector = into_project_selector(request.into_inner());
        let app = self.app.clone();
        let project = blocking({
            let app = app.clone();
            let selector = selector.clone();
            move || app.get_project(&selector)
        })
        .await?;
        let events = blocking(move || app.build_events(&selector)).await?;
        let project_id = project.project.project_id;
        let stream = tokio_stream::iter(events.into_iter().map(move |event| {
            Ok(BuildEvent {
                project_id: project_id.clone(),
                level: event.level,
                message: event.message,
            })
        }));
        Ok(Response::new(Box::pin(stream)))
    }
}

#[tonic::async_trait]
impl SimulatorService for LocalRpc {
    type StreamSimulationEventsStream = RpcStream<SimulationEvent>;

    async fn start_simulation(
        &self,
        request: Request<StartSimulationRequest>,
    ) -> Result<Response<StartSimulationResponse>, Status> {
        let request = request.into_inner();
        let project = request.project.ok_or_else(|| {
            Status::invalid_argument("missing simulator input; provide a project selector")
        })?;
        let simulation = self
            .app
            .start_project_simulation(&into_project_selector(project))
            .await
            .map_err(to_status)?;
        Ok(Response::new(StartSimulationResponse {
            simulation: Some(simulation_info(simulation)),
        }))
    }

    async fn stop_simulation(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<StopSimulationResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let simulation = self
            .app
            .stop_simulation(&selector)
            .await
            .map_err(to_status)?;
        Ok(Response::new(StopSimulationResponse {
            simulation: Some(simulation_info(simulation)),
        }))
    }

    async fn get_simulation(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<GetSimulationResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let app = self.app.clone();
        let simulation = blocking(move || app.get_simulation(&selector)).await?;
        Ok(Response::new(GetSimulationResponse {
            simulation: Some(simulation_info(simulation)),
        }))
    }

    async fn stream_simulation_events(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<Self::StreamSimulationEventsStream>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let simulation_id = selector.simulation_id.clone();
        let app = self.app.clone();
        let events = blocking(move || app.simulation_events(&selector)).await?;
        let stream = tokio_stream::iter(events.into_iter().map(move |event| {
            Ok(SimulationEvent {
                simulation_id: simulation_id.clone(),
                level: event.level,
                message: event.message,
            })
        }));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn transmit_apdu(
        &self,
        request: Request<TransmitApduRequest>,
    ) -> Result<Response<TransmitApduResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let command = command_apdu_from_proto(request.command)?;
        let exchange = self
            .app
            .transmit_command(&selector, &command)
            .await
            .map_err(to_status)?;
        Ok(Response::new(TransmitApduResponse {
            response: Some(response_apdu_frame(&exchange.response)),
            session_state: Some(iso_session_state_info(&exchange.session_state)),
        }))
    }

    async fn transmit_raw_apdu(
        &self,
        request: Request<TransmitRawApduRequest>,
    ) -> Result<Response<TransmitRawApduResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let command = CommandApdu::parse(&request.apdu).map_err(to_status)?;
        let exchange = self
            .app
            .transmit_command(&selector, &command)
            .await
            .map_err(to_status)?;
        Ok(Response::new(TransmitRawApduResponse {
            apdu: request.apdu,
            response: Some(response_apdu_frame(&exchange.response)),
            session_state: Some(iso_session_state_info(&exchange.session_state)),
        }))
    }

    async fn get_session_state(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<GetSimulationSessionStateResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let session_state = self
            .app
            .simulation_session_state(&selector)
            .map_err(to_status)?;
        Ok(Response::new(GetSimulationSessionStateResponse {
            session_state: Some(iso_session_state_info(&session_state)),
        }))
    }

    async fn manage_channel(
        &self,
        request: Request<ManageChannelRequest>,
    ) -> Result<Response<ManageChannelResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let channel_number = request
            .channel_number
            .map(u8::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("channel number must fit in one byte"))?;
        let summary = self
            .app
            .manage_simulation_channel(&selector, request.open, channel_number)
            .await
            .map_err(to_status)?;
        Ok(Response::new(ManageChannelResponse {
            channel_number: summary.channel_number.map(u32::from),
            response: Some(response_apdu_frame(&summary.response)),
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn open_secure_messaging(
        &self,
        request: Request<SecureMessagingRequest>,
    ) -> Result<Response<SecureMessagingResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let summary = self
            .app
            .open_simulation_secure_messaging(
                &selector,
                secure_messaging_protocol_from_proto(request.protocol, &request.protocol_label),
                request
                    .security_level
                    .map(u8::try_from)
                    .transpose()
                    .map_err(|_| {
                        Status::invalid_argument("secure messaging level must fit in one byte")
                    })?,
                (!request.session_id.is_empty()).then_some(request.session_id),
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(SecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn advance_secure_messaging(
        &self,
        request: Request<SecureMessagingAdvanceRequest>,
    ) -> Result<Response<SecureMessagingResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let summary = self
            .app
            .advance_simulation_secure_messaging(&selector, request.increment_by)
            .await
            .map_err(to_status)?;
        Ok(Response::new(SecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn close_secure_messaging(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<SecureMessagingResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let summary = self
            .app
            .close_simulation_secure_messaging(&selector)
            .await
            .map_err(to_status)?;
        Ok(Response::new(SecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn open_gp_secure_channel(
        &self,
        request: Request<OpenGpSecureChannelRequest>,
    ) -> Result<Response<OpenGpSecureChannelResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let summary = self
            .app
            .open_gp_secure_channel_on_simulation(
                &selector,
                (!request.keyset_name.is_empty()).then_some(request.keyset_name.as_str()),
                request
                    .security_level
                    .map(u8::try_from)
                    .transpose()
                    .map_err(|_| {
                        Status::invalid_argument("GP security level must fit in one byte")
                    })?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(OpenGpSecureChannelResponse {
            secure_channel: Some(gp_secure_channel_info(&summary)),
        }))
    }

    async fn close_gp_secure_channel(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<SecureMessagingResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let summary = self
            .app
            .close_gp_secure_channel_on_simulation(&selector)
            .await
            .map_err(to_status)?;
        Ok(Response::new(SecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn reset_simulation(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<ResetSimulationResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let summary = self
            .app
            .reset_simulation_summary(&selector)
            .await
            .map_err(to_status)?;
        Ok(Response::new(ResetSimulationResponse {
            atr: summary.atr.as_ref().map(atr_info),
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }
}

#[tonic::async_trait]
impl CardService for LocalRpc {
    async fn list_readers(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListReadersResponse>, Status> {
        let readers = self.app.list_readers().await.map_err(to_status)?;
        Ok(Response::new(ListReadersResponse {
            readers: readers
                .into_iter()
                .map(|reader| ReaderInfo {
                    name: reader.name,
                    card_present: reader.card_present,
                })
                .collect(),
        }))
    }

    async fn get_card_status(
        &self,
        request: Request<CardStatusRequest>,
    ) -> Result<Response<CardStatusResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let status = self
            .app
            .card_status(reader_name.as_deref())
            .await
            .map_err(to_status)?;
        Ok(Response::new(CardStatusResponse {
            reader_name: status.reader_name,
            card_present: status.card_present,
            atr: status.atr.as_ref().map(atr_info),
            active_protocol: status
                .active_protocol
                .as_ref()
                .map(protocol_parameters_info),
            iso_capabilities: Some(iso_capabilities_info(&status.iso_capabilities)),
            session_state: Some(iso_session_state_info(&status.session_state)),
            lines: status.lines,
        }))
    }

    async fn install_cap(
        &self,
        request: Request<InstallCapRequest>,
    ) -> Result<Response<InstallCapResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = match request.input {
            Some(InstallCapInput::Project(project)) => self
                .app
                .install_project_cap(&into_project_selector(project), reader_name.as_deref())
                .await
                .map_err(to_status)?,
            Some(InstallCapInput::CapPath(cap_path)) => self
                .app
                .install_cap_from_path(Path::new(&cap_path), reader_name.as_deref(), None)
                .await
                .map_err(to_status)?,
            None => {
                return Err(Status::invalid_argument(
                    "missing card install input; provide a project selector or CAP path",
                ));
            }
        };
        Ok(Response::new(install_cap_response(summary)))
    }

    async fn delete_item(
        &self,
        request: Request<DeleteItemRequest>,
    ) -> Result<Response<DeleteItemResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = self
            .app
            .delete_item(reader_name.as_deref(), &request.aid)
            .await
            .map_err(to_status)?;
        Ok(Response::new(delete_item_response(summary)))
    }

    async fn list_packages(
        &self,
        request: Request<ListPackagesRequest>,
    ) -> Result<Response<ListPackagesResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let inventory = self
            .app
            .list_packages((!reader_name.is_empty()).then_some(reader_name).as_deref())
            .await
            .map_err(to_status)?;
        Ok(Response::new(package_inventory_response(inventory)))
    }

    async fn list_applets(
        &self,
        request: Request<ListAppletsRequest>,
    ) -> Result<Response<ListAppletsResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let inventory = self
            .app
            .list_applets((!reader_name.is_empty()).then_some(reader_name).as_deref())
            .await
            .map_err(to_status)?;
        Ok(Response::new(applet_inventory_response(inventory)))
    }

    async fn transmit_apdu(
        &self,
        request: Request<CardApduRequest>,
    ) -> Result<Response<CardApduResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let command = command_apdu_from_proto(request.command)?;
        let exchange = self
            .app
            .card_command(reader_name.as_deref(), &command)
            .await
            .map_err(to_status)?;
        Ok(Response::new(CardApduResponse {
            response: Some(response_apdu_frame(&exchange.response)),
            session_state: Some(iso_session_state_info(&exchange.session_state)),
        }))
    }

    async fn transmit_raw_apdu(
        &self,
        request: Request<CardRawApduRequest>,
    ) -> Result<Response<CardRawApduResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let command = CommandApdu::parse(&request.apdu).map_err(to_status)?;
        let exchange = self
            .app
            .card_command(reader_name.as_deref(), &command)
            .await
            .map_err(to_status)?;
        Ok(Response::new(CardRawApduResponse {
            apdu: request.apdu,
            response: Some(response_apdu_frame(&exchange.response)),
            session_state: Some(iso_session_state_info(&exchange.session_state)),
        }))
    }

    async fn get_session_state(
        &self,
        request: Request<CardSelector>,
    ) -> Result<Response<GetCardSessionStateResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let session_state = self
            .app
            .card_session_state((!reader_name.is_empty()).then_some(reader_name).as_deref())
            .map_err(to_status)?;
        Ok(Response::new(GetCardSessionStateResponse {
            session_state: Some(iso_session_state_info(&session_state)),
        }))
    }

    async fn manage_channel(
        &self,
        request: Request<CardManageChannelRequest>,
    ) -> Result<Response<CardManageChannelResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let channel_number = request
            .channel_number
            .map(u8::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("channel number must fit in one byte"))?;
        let summary = self
            .app
            .manage_card_channel(reader_name.as_deref(), request.open, channel_number)
            .await
            .map_err(to_status)?;
        Ok(Response::new(CardManageChannelResponse {
            channel_number: summary.channel_number.map(u32::from),
            response: Some(response_apdu_frame(&summary.response)),
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn open_secure_messaging(
        &self,
        request: Request<CardSecureMessagingRequest>,
    ) -> Result<Response<CardSecureMessagingResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = self
            .app
            .open_card_secure_messaging(
                reader_name.as_deref(),
                secure_messaging_protocol_from_proto(request.protocol, &request.protocol_label),
                request
                    .security_level
                    .map(u8::try_from)
                    .transpose()
                    .map_err(|_| {
                        Status::invalid_argument("secure messaging level must fit in one byte")
                    })?,
                (!request.session_id.is_empty()).then_some(request.session_id),
            )
            .map_err(to_status)?;
        Ok(Response::new(CardSecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn advance_secure_messaging(
        &self,
        request: Request<CardSecureMessagingAdvanceRequest>,
    ) -> Result<Response<CardSecureMessagingResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = self
            .app
            .advance_card_secure_messaging(reader_name.as_deref(), request.increment_by)
            .map_err(to_status)?;
        Ok(Response::new(CardSecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn close_secure_messaging(
        &self,
        request: Request<CardSelector>,
    ) -> Result<Response<CardSecureMessagingResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let summary = self
            .app
            .close_card_secure_messaging(
                (!reader_name.is_empty()).then_some(reader_name).as_deref(),
            )
            .map_err(to_status)?;
        Ok(Response::new(CardSecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn open_gp_secure_channel(
        &self,
        request: Request<OpenCardGpSecureChannelRequest>,
    ) -> Result<Response<OpenCardGpSecureChannelResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = self
            .app
            .open_gp_secure_channel_on_card(
                reader_name.as_deref(),
                (!request.keyset_name.is_empty()).then_some(request.keyset_name.as_str()),
                request
                    .security_level
                    .map(u8::try_from)
                    .transpose()
                    .map_err(|_| {
                        Status::invalid_argument("GP security level must fit in one byte")
                    })?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(OpenCardGpSecureChannelResponse {
            secure_channel: Some(gp_secure_channel_info(&summary)),
        }))
    }

    async fn close_gp_secure_channel(
        &self,
        request: Request<CardSelector>,
    ) -> Result<Response<CardSecureMessagingResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let summary = self
            .app
            .close_gp_secure_channel_on_card(
                (!reader_name.is_empty()).then_some(reader_name).as_deref(),
            )
            .map_err(to_status)?;
        Ok(Response::new(CardSecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn reset_card(
        &self,
        request: Request<ResetCardRequest>,
    ) -> Result<Response<ResetCardResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let reader_name = (!reader_name.is_empty()).then_some(reader_name);
        let summary = self
            .app
            .reset_card_summary(reader_name.as_deref())
            .await
            .map_err(to_status)?;
        Ok(Response::new(ResetCardResponse {
            atr: summary.atr.as_ref().map(atr_info),
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }
}

#[tonic::async_trait]
impl SystemService for LocalRpc {
    async fn setup_toolchains(
        &self,
        request: Request<SetupToolchainsRequest>,
    ) -> Result<Response<SetupToolchainsResponse>, Status> {
        let request = request.into_inner();
        let java_bin = (!request.java_bin.is_empty()).then_some(request.java_bin);
        let app = self.app.clone();
        let setup = blocking(move || app.setup_toolchains(java_bin.as_deref())).await?;
        Ok(Response::new(SetupToolchainsResponse {
            config_path: setup.config_path.display().to_string(),
            message: setup.message,
        }))
    }

    async fn doctor(&self, _request: Request<Empty>) -> Result<Response<DoctorResponse>, Status> {
        let app = self.app.clone();
        let lines = blocking(move || app.doctor()).await?;
        Ok(Response::new(DoctorResponse { lines }))
    }

    async fn get_service_status(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<GetServiceStatusResponse>, Status> {
        let app = self.app.clone();
        let status = blocking(move || app.service_status()).await?;
        Ok(Response::new(service_status_response(status)))
    }
}

async fn blocking<F, T>(work: F) -> Result<T, Status>
where
    F: FnOnce() -> Result<T, JcimError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| Status::internal(format!("blocking task failed: {error}")))?
        .map_err(to_status)
}

fn into_project_selector(selector: ProjectSelector) -> ProjectSelectorInput {
    ProjectSelectorInput {
        project_path: (!selector.project_path.is_empty())
            .then_some(PathBuf::from(selector.project_path)),
        project_id: (!selector.project_id.is_empty()).then_some(selector.project_id),
    }
}

fn into_simulation_selector(selector: SimulationSelector) -> SimulationSelectorInput {
    SimulationSelectorInput {
        simulation_id: selector.simulation_id,
    }
}

fn project_details_response(details: ProjectDetails) -> GetProjectResponse {
    GetProjectResponse {
        project: Some(project_info(details.project)),
        manifest_toml: details.manifest_toml,
    }
}

fn project_info(project: ProjectSummary) -> ProjectInfo {
    ProjectInfo {
        project_id: project.project_id,
        name: project.name,
        project_path: project.project_path.display().to_string(),
        profile: project.profile,
        build_kind: project.build_kind,
        package_name: project.package_name,
        package_aid: project.package_aid,
        applets: project
            .applets
            .into_iter()
            .map(|applet| AppletInfo {
                class_name: applet.class_name,
                aid: applet.aid,
            })
            .collect(),
    }
}

fn artifact_info(artifact: ArtifactSummary) -> Artifact {
    Artifact {
        kind: artifact.kind,
        path: artifact.path.display().to_string(),
    }
}

fn simulation_info(simulation: SimulationSummary) -> SimulationInfo {
    SimulationInfo {
        simulation_id: simulation.simulation_id,
        source_kind: match simulation.source_kind {
            jcim_app::SimulationSourceKind::Project => SimulationSourceKind::Project as i32,
            jcim_app::SimulationSourceKind::Cap => SimulationSourceKind::Cap as i32,
        },
        project_id: simulation.project_id.unwrap_or_default(),
        project_path: simulation
            .project_path
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        cap_path: simulation.cap_path.display().to_string(),
        engine_mode: match simulation.engine_mode {
            jcim_app::SimulationEngineMode::Native => SimulationEngineMode::Native as i32,
            jcim_app::SimulationEngineMode::Container => SimulationEngineMode::Container as i32,
            jcim_app::SimulationEngineMode::ManagedJava => SimulationEngineMode::ManagedJava as i32,
        },
        status: match simulation.status {
            jcim_app::SimulationStatusKind::Starting => SimulationStatus::Starting as i32,
            jcim_app::SimulationStatusKind::Running => SimulationStatus::Running as i32,
            jcim_app::SimulationStatusKind::Stopped => SimulationStatus::Stopped as i32,
            jcim_app::SimulationStatusKind::Failed => SimulationStatus::Failed as i32,
        },
        reader_name: simulation.reader_name,
        health: simulation.health,
        atr: simulation.atr.as_ref().map(atr_info),
        active_protocol: simulation
            .active_protocol
            .as_ref()
            .map(protocol_parameters_info),
        iso_capabilities: Some(iso_capabilities_info(&simulation.iso_capabilities)),
        session_state: Some(iso_session_state_info(&simulation.session_state)),
        package_count: simulation.package_count,
        applet_count: simulation.applet_count,
        package_name: simulation.package_name,
        package_aid: simulation.package_aid,
        recent_events: simulation.recent_events,
    }
}

fn install_cap_response(summary: CardInstallSummary) -> InstallCapResponse {
    InstallCapResponse {
        reader_name: summary.reader_name,
        cap_path: summary.cap_path.display().to_string(),
        package_name: summary.package_name,
        package_aid: summary.package_aid,
        applets: summary
            .applets
            .into_iter()
            .map(|applet| AppletInfo {
                class_name: applet.class_name,
                aid: applet.aid,
            })
            .collect(),
        output_lines: summary.output_lines,
    }
}

fn delete_item_response(summary: CardDeleteSummary) -> DeleteItemResponse {
    DeleteItemResponse {
        reader_name: summary.reader_name,
        aid: summary.aid,
        deleted: summary.deleted,
        output_lines: summary.output_lines,
    }
}

fn package_inventory_response(inventory: CardPackageInventory) -> ListPackagesResponse {
    ListPackagesResponse {
        reader_name: inventory.reader_name,
        packages: inventory
            .packages
            .into_iter()
            .map(|package| CardPackageInfo {
                aid: package.aid,
                description: package.description,
            })
            .collect(),
        output_lines: inventory.output_lines,
    }
}

fn applet_inventory_response(inventory: CardAppletInventory) -> ListAppletsResponse {
    ListAppletsResponse {
        reader_name: inventory.reader_name,
        applets: inventory
            .applets
            .into_iter()
            .map(|applet| CardAppletInfo {
                aid: applet.aid,
                description: applet.description,
            })
            .collect(),
        output_lines: inventory.output_lines,
    }
}

// `tonic::Status` is the maintained transport-edge error type for these conversion helpers.
#[allow(clippy::result_large_err)]
fn command_apdu_from_proto(frame: Option<CommandApduFrame>) -> Result<CommandApdu, Status> {
    let frame = frame.ok_or_else(|| Status::invalid_argument("missing command APDU"))?;
    let data = frame.data.clone();
    let command = if !frame.raw.is_empty() {
        CommandApdu::parse(&frame.raw).map_err(to_status)?
    } else {
        let cla = u8::try_from(frame.cla)
            .map_err(|_| Status::invalid_argument("CLA must fit in one byte"))?;
        let ins = u8::try_from(frame.ins)
            .map_err(|_| Status::invalid_argument("INS must fit in one byte"))?;
        let p1 = u8::try_from(frame.p1)
            .map_err(|_| Status::invalid_argument("P1 must fit in one byte"))?;
        let p2 = u8::try_from(frame.p2)
            .map_err(|_| Status::invalid_argument("P2 must fit in one byte"))?;
        let ne = frame
            .ne
            .map(usize::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("Ne does not fit on this platform"))?;
        match jcim_api::v0_2::ApduEncoding::try_from(frame.encoding).ok() {
            Some(jcim_api::v0_2::ApduEncoding::Short) => CommandApdu::new_with_encoding(
                cla,
                ins,
                p1,
                p2,
                data.clone(),
                ne,
                ApduEncoding::Short,
            )
            .map_err(to_status)?,
            Some(jcim_api::v0_2::ApduEncoding::Extended) => CommandApdu::new_with_encoding(
                cla,
                ins,
                p1,
                p2,
                data.clone(),
                ne,
                ApduEncoding::Extended,
            )
            .map_err(to_status)?,
            _ => CommandApdu::new(cla, ins, p1, p2, data, ne),
        }
    };

    let descriptor = iso7816::describe_command(&command);
    let apdu_case = jcim_api::v0_2::CommandApduCase::try_from(frame.apdu_case)
        .ok()
        .and_then(command_apdu_case_from_proto);
    if let Some(apdu_case) = apdu_case
        && apdu_case != command.apdu_case()
    {
        return Err(Status::invalid_argument(
            "command APDU metadata did not match the encoded APDU case",
        ));
    }
    let domain = jcim_api::v0_2::CommandDomain::try_from(frame.domain)
        .ok()
        .and_then(command_domain_from_proto);
    if let Some(domain) = domain
        && domain != descriptor.domain
    {
        return Err(Status::invalid_argument(
            "command APDU metadata did not match the encoded command domain",
        ));
    }
    let kind = jcim_api::v0_2::CommandKind::try_from(frame.kind)
        .ok()
        .and_then(command_kind_from_proto);
    if let Some(kind) = kind
        && kind != descriptor.kind
    {
        return Err(Status::invalid_argument(
            "command APDU metadata did not match the encoded command kind",
        ));
    }
    if frame.logical_channel != u32::from(descriptor.logical_channel) {
        return Err(Status::invalid_argument(
            "command APDU logical channel metadata did not match the CLA byte",
        ));
    }

    Ok(command)
}

fn response_apdu_frame(response: &ResponseApdu) -> ResponseApduFrame {
    let status = response.status_word();
    ResponseApduFrame {
        raw: response.to_bytes(),
        data: response.data.clone(),
        sw: u32::from(response.sw),
        status: Some(status_word_info(status)),
    }
}

fn aid_info(aid: &Aid) -> AidInfo {
    AidInfo {
        raw: aid.as_bytes().to_vec(),
        hex: aid.to_hex(),
    }
}

fn file_selection_info(selection: &FileSelection) -> FileSelectionInfo {
    FileSelectionInfo {
        selection: Some(match selection {
            FileSelection::ByName(bytes) => FileSelectionProto::ByName(bytes.clone()),
            FileSelection::FileId(file_id) => FileSelectionProto::FileId(u32::from(*file_id)),
            FileSelection::Path(path) => FileSelectionProto::Path(path.clone()),
        }),
    }
}

fn status_word_info(status: StatusWord) -> StatusWordInfo {
    StatusWordInfo {
        value: u32::from(status.as_u16()),
        class: match status.class() {
            StatusWordClass::NormalProcessing => {
                jcim_api::v0_2::StatusWordClass::NormalProcessing as i32
            }
            StatusWordClass::Warning => jcim_api::v0_2::StatusWordClass::Warning as i32,
            StatusWordClass::ExecutionError => {
                jcim_api::v0_2::StatusWordClass::ExecutionError as i32
            }
            StatusWordClass::CheckingError => jcim_api::v0_2::StatusWordClass::CheckingError as i32,
            StatusWordClass::Unknown => jcim_api::v0_2::StatusWordClass::Unknown as i32,
        },
        label: status.label().to_string(),
        success: status.is_success(),
        warning: status.is_warning(),
        remaining_response_bytes: status.remaining_response_bytes().map(|value| value as u32),
        retry_counter: status.retry_counter().map(u32::from),
        exact_length_hint: status.exact_length_hint().map(|value| value as u32),
    }
}

fn atr_info(atr: &Atr) -> AtrInfo {
    AtrInfo {
        raw: atr.raw.clone(),
        hex: atr.to_hex(),
        convention: match atr.convention {
            TransmissionConvention::Direct => jcim_api::v0_2::TransmissionConvention::Direct as i32,
            TransmissionConvention::Inverse => {
                jcim_api::v0_2::TransmissionConvention::Inverse as i32
            }
        },
        interface_groups: atr
            .interface_groups
            .iter()
            .map(|group| AtrInterfaceGroup {
                index: u32::from(group.index),
                ta: group.ta.map(u32::from),
                tb: group.tb.map(u32::from),
                tc: group.tc.map(u32::from),
                td: group.td.map(u32::from),
                protocol: group.protocol.map_or(
                    jcim_api::v0_2::TransportProtocol::Unspecified as i32,
                    transport_protocol_value,
                ),
            })
            .collect(),
        historical_bytes: atr.historical_bytes.clone(),
        checksum_tck: atr.checksum_tck.map(u32::from),
        protocols: atr
            .protocols
            .iter()
            .copied()
            .map(transport_protocol_value)
            .collect(),
    }
}

fn protocol_parameters_info(parameters: &ProtocolParameters) -> ProtocolParametersInfo {
    ProtocolParametersInfo {
        protocol: parameters.protocol.map_or(
            jcim_api::v0_2::TransportProtocol::Unspecified as i32,
            transport_protocol_value,
        ),
        fi: parameters.fi.map(u32::from),
        di: parameters.di.map(u32::from),
        waiting_integer: parameters.waiting_integer.map(u32::from),
        ifsc: parameters.ifsc.map(u32::from),
    }
}

fn iso_capabilities_info(capabilities: &IsoCapabilities) -> IsoCapabilitiesInfo {
    IsoCapabilitiesInfo {
        protocols: capabilities
            .protocols
            .iter()
            .copied()
            .map(transport_protocol_value)
            .collect(),
        extended_length: capabilities.extended_length,
        logical_channels: capabilities.logical_channels,
        max_logical_channels: u32::from(capabilities.max_logical_channels),
        secure_messaging: capabilities.secure_messaging,
        file_model_visibility: capabilities.file_model_visibility,
        raw_apdu: capabilities.raw_apdu,
    }
}

fn iso_session_state_info(state: &IsoSessionState) -> IsoSessionStateInfo {
    IsoSessionStateInfo {
        power_state: match state.power_state {
            PowerState::Off => jcim_api::v0_2::PowerState::Off as i32,
            PowerState::On => jcim_api::v0_2::PowerState::On as i32,
        },
        atr: state.atr.as_ref().map(atr_info),
        active_protocol: state.active_protocol.as_ref().map(protocol_parameters_info),
        selected_aid: state.selected_aid.as_ref().map(aid_info),
        current_file: state.current_file.as_ref().map(file_selection_info),
        open_channels: state
            .open_channels
            .iter()
            .map(logical_channel_state_info)
            .collect(),
        secure_messaging: Some(secure_messaging_state_info(&state.secure_messaging)),
        verified_references: state
            .verified_references
            .iter()
            .copied()
            .map(u32::from)
            .collect(),
        retry_counters: state
            .retry_counters
            .iter()
            .map(retry_counter_info)
            .collect(),
        last_status: state.last_status.map(status_word_info),
    }
}

fn logical_channel_state_info(channel: &LogicalChannelState) -> LogicalChannelStateInfo {
    LogicalChannelStateInfo {
        channel_number: u32::from(channel.channel_number),
        selected_aid: channel.selected_aid.as_ref().map(aid_info),
        current_file: channel.current_file.as_ref().map(file_selection_info),
    }
}

fn retry_counter_info(counter: &RetryCounterState) -> RetryCounterInfo {
    RetryCounterInfo {
        reference: u32::from(counter.reference),
        remaining: u32::from(counter.remaining),
    }
}

fn secure_messaging_state_info(state: &SecureMessagingState) -> SecureMessagingStateInfo {
    let (protocol, protocol_label) = match state.protocol.as_ref() {
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
    };

    SecureMessagingStateInfo {
        active: state.active,
        protocol,
        security_level: state.security_level.map(u32::from),
        session_id: state.session_id.clone().unwrap_or_default(),
        command_counter: state.command_counter,
        protocol_label,
    }
}

fn gp_secure_channel_info(summary: &GpSecureChannelSummary) -> GpSecureChannelInfo {
    let protocol = match summary.secure_channel.keyset.mode {
        jcim_core::globalplatform::ScpMode::Scp02 => {
            jcim_api::v0_2::SecureMessagingProtocol::Scp02 as i32
        }
        jcim_core::globalplatform::ScpMode::Scp03 => {
            jcim_api::v0_2::SecureMessagingProtocol::Scp03 as i32
        }
    };
    GpSecureChannelInfo {
        keyset_name: summary.secure_channel.keyset.name.clone(),
        protocol,
        security_level: u32::from(summary.secure_channel.security_level.as_byte()),
        session_id: summary.secure_channel.session_id.clone(),
        selected_aid: Some(aid_info(&summary.selected_aid)),
        session_state: Some(iso_session_state_info(&summary.session_state)),
    }
}

fn transport_protocol_value(protocol: TransportProtocol) -> i32 {
    match protocol {
        TransportProtocol::T0 => jcim_api::v0_2::TransportProtocol::T0 as i32,
        TransportProtocol::T1 => jcim_api::v0_2::TransportProtocol::T1 as i32,
        TransportProtocol::T2 => jcim_api::v0_2::TransportProtocol::T2 as i32,
        TransportProtocol::T3 => jcim_api::v0_2::TransportProtocol::T3 as i32,
        TransportProtocol::T14 => jcim_api::v0_2::TransportProtocol::T14 as i32,
        TransportProtocol::Other(_) => jcim_api::v0_2::TransportProtocol::Other as i32,
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

fn command_apdu_case_from_proto(value: jcim_api::v0_2::CommandApduCase) -> Option<CommandApduCase> {
    match value {
        jcim_api::v0_2::CommandApduCase::CommandApduCase1 => Some(CommandApduCase::Case1),
        jcim_api::v0_2::CommandApduCase::CommandApduCase2Short => Some(CommandApduCase::Case2Short),
        jcim_api::v0_2::CommandApduCase::CommandApduCase3Short => Some(CommandApduCase::Case3Short),
        jcim_api::v0_2::CommandApduCase::CommandApduCase4Short => Some(CommandApduCase::Case4Short),
        jcim_api::v0_2::CommandApduCase::CommandApduCase2Extended => {
            Some(CommandApduCase::Case2Extended)
        }
        jcim_api::v0_2::CommandApduCase::CommandApduCase3Extended => {
            Some(CommandApduCase::Case3Extended)
        }
        jcim_api::v0_2::CommandApduCase::CommandApduCase4Extended => {
            Some(CommandApduCase::Case4Extended)
        }
        jcim_api::v0_2::CommandApduCase::Unspecified => None,
    }
}

fn command_domain_from_proto(
    value: jcim_api::v0_2::CommandDomain,
) -> Option<iso7816::CommandDomain> {
    match value {
        jcim_api::v0_2::CommandDomain::Iso7816 => Some(iso7816::CommandDomain::Iso7816),
        jcim_api::v0_2::CommandDomain::GlobalPlatform => {
            Some(iso7816::CommandDomain::GlobalPlatform)
        }
        jcim_api::v0_2::CommandDomain::Opaque => Some(iso7816::CommandDomain::Opaque),
        jcim_api::v0_2::CommandDomain::Unspecified => None,
    }
}

fn command_kind_from_proto(value: jcim_api::v0_2::CommandKind) -> Option<iso7816::CommandKind> {
    Some(match value {
        jcim_api::v0_2::CommandKind::Select => iso7816::CommandKind::Select,
        jcim_api::v0_2::CommandKind::ManageChannel => iso7816::CommandKind::ManageChannel,
        jcim_api::v0_2::CommandKind::GetResponse => iso7816::CommandKind::GetResponse,
        jcim_api::v0_2::CommandKind::ReadBinary => iso7816::CommandKind::ReadBinary,
        jcim_api::v0_2::CommandKind::WriteBinary => iso7816::CommandKind::WriteBinary,
        jcim_api::v0_2::CommandKind::UpdateBinary => iso7816::CommandKind::UpdateBinary,
        jcim_api::v0_2::CommandKind::EraseBinary => iso7816::CommandKind::EraseBinary,
        jcim_api::v0_2::CommandKind::ReadRecord => iso7816::CommandKind::ReadRecord,
        jcim_api::v0_2::CommandKind::UpdateRecord => iso7816::CommandKind::UpdateRecord,
        jcim_api::v0_2::CommandKind::AppendRecord => iso7816::CommandKind::AppendRecord,
        jcim_api::v0_2::CommandKind::SearchRecord => iso7816::CommandKind::SearchRecord,
        jcim_api::v0_2::CommandKind::GetData => iso7816::CommandKind::GetData,
        jcim_api::v0_2::CommandKind::PutData => iso7816::CommandKind::PutData,
        jcim_api::v0_2::CommandKind::Verify => iso7816::CommandKind::Verify,
        jcim_api::v0_2::CommandKind::ChangeReferenceData => {
            iso7816::CommandKind::ChangeReferenceData
        }
        jcim_api::v0_2::CommandKind::ResetRetryCounter => iso7816::CommandKind::ResetRetryCounter,
        jcim_api::v0_2::CommandKind::InternalAuthenticate => {
            iso7816::CommandKind::InternalAuthenticate
        }
        jcim_api::v0_2::CommandKind::ExternalAuthenticate => {
            iso7816::CommandKind::ExternalAuthenticate
        }
        jcim_api::v0_2::CommandKind::GetChallenge => iso7816::CommandKind::GetChallenge,
        jcim_api::v0_2::CommandKind::Envelope => iso7816::CommandKind::Envelope,
        jcim_api::v0_2::CommandKind::GpGetStatus => iso7816::CommandKind::GpGetStatus,
        jcim_api::v0_2::CommandKind::GpSetStatus => iso7816::CommandKind::GpSetStatus,
        jcim_api::v0_2::CommandKind::GpInitializeUpdate => iso7816::CommandKind::GpInitializeUpdate,
        jcim_api::v0_2::CommandKind::GpExternalAuthenticate => {
            iso7816::CommandKind::GpExternalAuthenticate
        }
        jcim_api::v0_2::CommandKind::Opaque => iso7816::CommandKind::Opaque,
        jcim_api::v0_2::CommandKind::Unspecified => return None,
    })
}

fn service_status_response(status: ServiceStatusSummary) -> GetServiceStatusResponse {
    GetServiceStatusResponse {
        socket_path: status.socket_path.display().to_string(),
        running: status.running,
        known_project_count: status.known_project_count,
        active_simulation_count: status.active_simulation_count,
        service_binary_path: status.service_binary_path.display().to_string(),
        service_binary_fingerprint: status.service_binary_fingerprint,
    }
}

fn to_status(error: JcimError) -> Status {
    match error {
        JcimError::Unsupported(message)
        | JcimError::InvalidAid(message)
        | JcimError::InvalidApdu(message)
        | JcimError::Gp(message)
        | JcimError::CapFormat(message)
        | JcimError::MalformedBackendReply(message) => Status::invalid_argument(message),
        JcimError::BackendUnavailable(message)
        | JcimError::BackendExited(message)
        | JcimError::BackendStartup(message) => Status::unavailable(message),
        other => Status::internal(other.to_string()),
    }
}
