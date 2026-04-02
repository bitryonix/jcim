use std::path::PathBuf;

use tonic::Status;

use jcim_api::v0_3::file_selection_info::Selection as FileSelectionProto;
use jcim_api::v0_3::{
    AidInfo, AppletInfo, Artifact, AtrInfo, AtrInterfaceGroup, CardAppletInfo, CardPackageInfo,
    CommandApduFrame, DeleteItemResponse, FileSelectionInfo, GetProjectResponse,
    GetServiceStatusResponse, GpSecureChannelInfo, InstallCapResponse, IsoCapabilitiesInfo,
    IsoSessionStateInfo, ListAppletsResponse, ListPackagesResponse, LogicalChannelStateInfo,
    ProjectInfo, ProjectSelector, ProtocolParametersInfo, ResponseApduFrame, RetryCounterInfo,
    SecureMessagingStateInfo, SimulationInfo, SimulationSelector, SimulationStatus, StatusWordInfo,
};
use jcim_app::{
    ArtifactSummary, CardAppletInventory, CardDeleteSummary, CardInstallSummary,
    CardPackageInventory, GpSecureChannelSummary, ProjectDetails, ProjectSelectorInput,
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

/// Convert the RPC project selector into the application-layer selector model.
pub(crate) fn into_project_selector(selector: ProjectSelector) -> ProjectSelectorInput {
    ProjectSelectorInput {
        project_path: (!selector.project_path.is_empty())
            .then_some(PathBuf::from(selector.project_path)),
        project_id: (!selector.project_id.is_empty()).then_some(selector.project_id),
    }
}

/// Convert the RPC simulation selector into the application-layer selector model.
pub(crate) fn into_simulation_selector(selector: SimulationSelector) -> SimulationSelectorInput {
    SimulationSelectorInput {
        simulation_id: selector.simulation_id,
    }
}

/// Encode project details into the RPC response envelope.
pub(crate) fn project_details_response(details: ProjectDetails) -> GetProjectResponse {
    GetProjectResponse {
        project: Some(project_info(details.project)),
        manifest_toml: details.manifest_toml,
    }
}

/// Convert one project summary into the RPC project-info message.
pub(crate) fn project_info(project: ProjectSummary) -> ProjectInfo {
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

/// Convert one artifact summary into the RPC artifact message.
pub(crate) fn artifact_info(artifact: ArtifactSummary) -> Artifact {
    Artifact {
        kind: artifact.kind,
        path: artifact.path.display().to_string(),
    }
}

/// Convert one simulation summary into the RPC simulation-info message.
pub(crate) fn simulation_info(simulation: SimulationSummary) -> SimulationInfo {
    SimulationInfo {
        simulation_id: simulation.simulation_id,
        project_id: simulation.project_id,
        project_path: simulation.project_path.display().to_string(),
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

/// Encode a card-install summary into the RPC install response.
pub(crate) fn install_cap_response(summary: CardInstallSummary) -> InstallCapResponse {
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

/// Encode a card-delete summary into the RPC delete response.
pub(crate) fn delete_item_response(summary: CardDeleteSummary) -> DeleteItemResponse {
    DeleteItemResponse {
        reader_name: summary.reader_name,
        aid: summary.aid,
        deleted: summary.deleted,
        output_lines: summary.output_lines,
    }
}

/// Encode a card-package inventory snapshot into the RPC package-list response.
pub(crate) fn package_inventory_response(inventory: CardPackageInventory) -> ListPackagesResponse {
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

/// Encode a card-applet inventory snapshot into the RPC applet-list response.
pub(crate) fn applet_inventory_response(inventory: CardAppletInventory) -> ListAppletsResponse {
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

/// Decode one structured or raw protobuf APDU frame into the core typed APDU model.
// `tonic::Status` is the maintained transport-edge error type for these conversion helpers.
#[allow(clippy::result_large_err)]
pub(crate) fn command_apdu_from_proto(
    frame: Option<CommandApduFrame>,
) -> Result<CommandApdu, Status> {
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
        match jcim_api::v0_3::ApduEncoding::try_from(frame.encoding).ok() {
            Some(jcim_api::v0_3::ApduEncoding::Short) => CommandApdu::new_with_encoding(
                cla,
                ins,
                p1,
                p2,
                data.clone(),
                ne,
                ApduEncoding::Short,
            )
            .map_err(to_status)?,
            Some(jcim_api::v0_3::ApduEncoding::Extended) => CommandApdu::new_with_encoding(
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
    let apdu_case = jcim_api::v0_3::CommandApduCase::try_from(frame.apdu_case)
        .ok()
        .and_then(command_apdu_case_from_proto);
    if let Some(apdu_case) = apdu_case
        && apdu_case != command.apdu_case()
    {
        return Err(Status::invalid_argument(
            "command APDU metadata did not match the encoded APDU case",
        ));
    }
    let domain = jcim_api::v0_3::CommandDomain::try_from(frame.domain)
        .ok()
        .and_then(command_domain_from_proto);
    if let Some(domain) = domain
        && domain != descriptor.domain
    {
        return Err(Status::invalid_argument(
            "command APDU metadata did not match the encoded command domain",
        ));
    }
    let kind = jcim_api::v0_3::CommandKind::try_from(frame.kind)
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

/// Encode a typed response APDU into the structured RPC response frame.
pub(crate) fn response_apdu_frame(response: &ResponseApdu) -> ResponseApduFrame {
    let status = response.status_word();
    ResponseApduFrame {
        raw: response.to_bytes(),
        data: response.data.clone(),
        sw: u32::from(response.sw),
        status: Some(status_word_info(status)),
    }
}

/// Encode one AID into the RPC helper shape used across multiple transport responses.
fn aid_info(aid: &Aid) -> AidInfo {
    AidInfo {
        raw: aid.as_bytes().to_vec(),
        hex: aid.to_hex(),
    }
}

/// Encode one selected-file descriptor into the RPC session-state shape.
fn file_selection_info(selection: &FileSelection) -> FileSelectionInfo {
    FileSelectionInfo {
        selection: Some(match selection {
            FileSelection::ByName(bytes) => FileSelectionProto::ByName(bytes.clone()),
            FileSelection::FileId(file_id) => FileSelectionProto::FileId(u32::from(*file_id)),
            FileSelection::Path(path) => FileSelectionProto::Path(path.clone()),
        }),
    }
}

/// Encode one status word plus its derived class and hints into the RPC model.
fn status_word_info(status: StatusWord) -> StatusWordInfo {
    StatusWordInfo {
        value: u32::from(status.as_u16()),
        class: match status.class() {
            StatusWordClass::NormalProcessing => {
                jcim_api::v0_3::StatusWordClass::NormalProcessing as i32
            }
            StatusWordClass::Warning => jcim_api::v0_3::StatusWordClass::Warning as i32,
            StatusWordClass::ExecutionError => {
                jcim_api::v0_3::StatusWordClass::ExecutionError as i32
            }
            StatusWordClass::CheckingError => jcim_api::v0_3::StatusWordClass::CheckingError as i32,
            StatusWordClass::Unknown => jcim_api::v0_3::StatusWordClass::Unknown as i32,
        },
        label: status.label().to_string(),
        success: status.is_success(),
        warning: status.is_warning(),
        remaining_response_bytes: status.remaining_response_bytes().map(|value| value as u32),
        retry_counter: status.retry_counter().map(u32::from),
        exact_length_hint: status.exact_length_hint().map(|value| value as u32),
    }
}

/// Encode one ATR into the RPC ATR info message.
pub(crate) fn atr_info(atr: &Atr) -> AtrInfo {
    AtrInfo {
        raw: atr.raw.clone(),
        hex: atr.to_hex(),
        convention: match atr.convention {
            TransmissionConvention::Direct => jcim_api::v0_3::TransmissionConvention::Direct as i32,
            TransmissionConvention::Inverse => {
                jcim_api::v0_3::TransmissionConvention::Inverse as i32
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
                    jcim_api::v0_3::TransportProtocol::Unspecified as i32,
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

/// Encode negotiated protocol parameters into the RPC protocol-parameters message.
pub(crate) fn protocol_parameters_info(parameters: &ProtocolParameters) -> ProtocolParametersInfo {
    ProtocolParametersInfo {
        protocol: parameters.protocol.map_or(
            jcim_api::v0_3::TransportProtocol::Unspecified as i32,
            transport_protocol_value,
        ),
        fi: parameters.fi.map(u32::from),
        di: parameters.di.map(u32::from),
        waiting_integer: parameters.waiting_integer.map(u32::from),
        ifsc: parameters.ifsc.map(u32::from),
    }
}

/// Encode ISO capability flags into the RPC capabilities message.
pub(crate) fn iso_capabilities_info(capabilities: &IsoCapabilities) -> IsoCapabilitiesInfo {
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

/// Encode tracked ISO session state into the RPC session-state message.
pub(crate) fn iso_session_state_info(state: &IsoSessionState) -> IsoSessionStateInfo {
    IsoSessionStateInfo {
        power_state: match state.power_state {
            PowerState::Off => jcim_api::v0_3::PowerState::Off as i32,
            PowerState::On => jcim_api::v0_3::PowerState::On as i32,
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

/// Encode one logical-channel snapshot into the RPC session-state shape.
fn logical_channel_state_info(channel: &LogicalChannelState) -> LogicalChannelStateInfo {
    LogicalChannelStateInfo {
        channel_number: u32::from(channel.channel_number),
        selected_aid: channel.selected_aid.as_ref().map(aid_info),
        current_file: channel.current_file.as_ref().map(file_selection_info),
    }
}

/// Encode one retry-counter snapshot into the RPC session-state shape.
fn retry_counter_info(counter: &RetryCounterState) -> RetryCounterInfo {
    RetryCounterInfo {
        reference: u32::from(counter.reference),
        remaining: u32::from(counter.remaining),
    }
}

/// Encode tracked secure-messaging state into the RPC session-state shape.
fn secure_messaging_state_info(state: &SecureMessagingState) -> SecureMessagingStateInfo {
    let (protocol, protocol_label) = match state.protocol.as_ref() {
        Some(SecureMessagingProtocol::Iso7816) => (
            jcim_api::v0_3::SecureMessagingProtocol::Iso7816 as i32,
            String::new(),
        ),
        Some(SecureMessagingProtocol::Scp02) => (
            jcim_api::v0_3::SecureMessagingProtocol::Scp02 as i32,
            String::new(),
        ),
        Some(SecureMessagingProtocol::Scp03) => (
            jcim_api::v0_3::SecureMessagingProtocol::Scp03 as i32,
            String::new(),
        ),
        Some(SecureMessagingProtocol::Other(label)) => (
            jcim_api::v0_3::SecureMessagingProtocol::Other as i32,
            label.clone(),
        ),
        None => (
            jcim_api::v0_3::SecureMessagingProtocol::Unspecified as i32,
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

/// Encode one established GP secure channel into the RPC transport summary.
pub(crate) fn gp_secure_channel_info(summary: &GpSecureChannelSummary) -> GpSecureChannelInfo {
    let protocol = match summary.secure_channel.keyset.mode {
        jcim_core::globalplatform::ScpMode::Scp02 => {
            jcim_api::v0_3::SecureMessagingProtocol::Scp02 as i32
        }
        jcim_core::globalplatform::ScpMode::Scp03 => {
            jcim_api::v0_3::SecureMessagingProtocol::Scp03 as i32
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

/// Map one transport protocol to the numeric protobuf enum value expected on the wire.
fn transport_protocol_value(protocol: TransportProtocol) -> i32 {
    match protocol {
        TransportProtocol::T0 => jcim_api::v0_3::TransportProtocol::T0 as i32,
        TransportProtocol::T1 => jcim_api::v0_3::TransportProtocol::T1 as i32,
        TransportProtocol::T2 => jcim_api::v0_3::TransportProtocol::T2 as i32,
        TransportProtocol::T3 => jcim_api::v0_3::TransportProtocol::T3 as i32,
        TransportProtocol::T14 => jcim_api::v0_3::TransportProtocol::T14 as i32,
        TransportProtocol::Other(_) => jcim_api::v0_3::TransportProtocol::Other as i32,
    }
}

/// Decode the protobuf secure-messaging protocol enum plus label into the core model.
pub(crate) fn secure_messaging_protocol_from_proto(
    value: i32,
    label: &str,
) -> Option<SecureMessagingProtocol> {
    match jcim_api::v0_3::SecureMessagingProtocol::try_from(value).ok()? {
        jcim_api::v0_3::SecureMessagingProtocol::Iso7816 => Some(SecureMessagingProtocol::Iso7816),
        jcim_api::v0_3::SecureMessagingProtocol::Scp02 => Some(SecureMessagingProtocol::Scp02),
        jcim_api::v0_3::SecureMessagingProtocol::Scp03 => Some(SecureMessagingProtocol::Scp03),
        jcim_api::v0_3::SecureMessagingProtocol::Other => {
            Some(SecureMessagingProtocol::Other(label.to_string()))
        }
        jcim_api::v0_3::SecureMessagingProtocol::Unspecified => None,
    }
}

/// Decode the protobuf APDU-case enum into the core APDU-case model.
fn command_apdu_case_from_proto(value: jcim_api::v0_3::CommandApduCase) -> Option<CommandApduCase> {
    match value {
        jcim_api::v0_3::CommandApduCase::CommandApduCase1 => Some(CommandApduCase::Case1),
        jcim_api::v0_3::CommandApduCase::CommandApduCase2Short => Some(CommandApduCase::Case2Short),
        jcim_api::v0_3::CommandApduCase::CommandApduCase3Short => Some(CommandApduCase::Case3Short),
        jcim_api::v0_3::CommandApduCase::CommandApduCase4Short => Some(CommandApduCase::Case4Short),
        jcim_api::v0_3::CommandApduCase::CommandApduCase2Extended => {
            Some(CommandApduCase::Case2Extended)
        }
        jcim_api::v0_3::CommandApduCase::CommandApduCase3Extended => {
            Some(CommandApduCase::Case3Extended)
        }
        jcim_api::v0_3::CommandApduCase::CommandApduCase4Extended => {
            Some(CommandApduCase::Case4Extended)
        }
        jcim_api::v0_3::CommandApduCase::Unspecified => None,
    }
}

/// Decode the protobuf command-domain enum into the ISO/GP command-domain model.
fn command_domain_from_proto(
    value: jcim_api::v0_3::CommandDomain,
) -> Option<iso7816::CommandDomain> {
    match value {
        jcim_api::v0_3::CommandDomain::Iso7816 => Some(iso7816::CommandDomain::Iso7816),
        jcim_api::v0_3::CommandDomain::GlobalPlatform => {
            Some(iso7816::CommandDomain::GlobalPlatform)
        }
        jcim_api::v0_3::CommandDomain::Opaque => Some(iso7816::CommandDomain::Opaque),
        jcim_api::v0_3::CommandDomain::Unspecified => None,
    }
}

/// Decode the protobuf command-kind enum into the ISO/GP command-kind model.
fn command_kind_from_proto(value: jcim_api::v0_3::CommandKind) -> Option<iso7816::CommandKind> {
    Some(match value {
        jcim_api::v0_3::CommandKind::Select => iso7816::CommandKind::Select,
        jcim_api::v0_3::CommandKind::ManageChannel => iso7816::CommandKind::ManageChannel,
        jcim_api::v0_3::CommandKind::GetResponse => iso7816::CommandKind::GetResponse,
        jcim_api::v0_3::CommandKind::ReadBinary => iso7816::CommandKind::ReadBinary,
        jcim_api::v0_3::CommandKind::WriteBinary => iso7816::CommandKind::WriteBinary,
        jcim_api::v0_3::CommandKind::UpdateBinary => iso7816::CommandKind::UpdateBinary,
        jcim_api::v0_3::CommandKind::EraseBinary => iso7816::CommandKind::EraseBinary,
        jcim_api::v0_3::CommandKind::ReadRecord => iso7816::CommandKind::ReadRecord,
        jcim_api::v0_3::CommandKind::UpdateRecord => iso7816::CommandKind::UpdateRecord,
        jcim_api::v0_3::CommandKind::AppendRecord => iso7816::CommandKind::AppendRecord,
        jcim_api::v0_3::CommandKind::SearchRecord => iso7816::CommandKind::SearchRecord,
        jcim_api::v0_3::CommandKind::GetData => iso7816::CommandKind::GetData,
        jcim_api::v0_3::CommandKind::PutData => iso7816::CommandKind::PutData,
        jcim_api::v0_3::CommandKind::Verify => iso7816::CommandKind::Verify,
        jcim_api::v0_3::CommandKind::ChangeReferenceData => {
            iso7816::CommandKind::ChangeReferenceData
        }
        jcim_api::v0_3::CommandKind::ResetRetryCounter => iso7816::CommandKind::ResetRetryCounter,
        jcim_api::v0_3::CommandKind::InternalAuthenticate => {
            iso7816::CommandKind::InternalAuthenticate
        }
        jcim_api::v0_3::CommandKind::ExternalAuthenticate => {
            iso7816::CommandKind::ExternalAuthenticate
        }
        jcim_api::v0_3::CommandKind::GetChallenge => iso7816::CommandKind::GetChallenge,
        jcim_api::v0_3::CommandKind::Envelope => iso7816::CommandKind::Envelope,
        jcim_api::v0_3::CommandKind::GpGetStatus => iso7816::CommandKind::GpGetStatus,
        jcim_api::v0_3::CommandKind::GpSetStatus => iso7816::CommandKind::GpSetStatus,
        jcim_api::v0_3::CommandKind::GpInitializeUpdate => iso7816::CommandKind::GpInitializeUpdate,
        jcim_api::v0_3::CommandKind::GpExternalAuthenticate => {
            iso7816::CommandKind::GpExternalAuthenticate
        }
        jcim_api::v0_3::CommandKind::Opaque => iso7816::CommandKind::Opaque,
        jcim_api::v0_3::CommandKind::Unspecified => return None,
    })
}

/// Encode service-status state into the RPC service-status response envelope.
pub(crate) fn service_status_response(status: ServiceStatusSummary) -> GetServiceStatusResponse {
    GetServiceStatusResponse {
        socket_path: status.socket_path.display().to_string(),
        running: status.running,
        known_project_count: status.known_project_count,
        active_simulation_count: status.active_simulation_count,
        service_binary_path: status.service_binary_path.display().to_string(),
        service_binary_fingerprint: status.service_binary_fingerprint,
    }
}

/// Map one application error into the tonic transport status used by the local daemon API.
pub(crate) fn to_status(error: JcimError) -> Status {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tonic::Code;

    use jcim_api::v0_3::{
        ApduEncoding, CommandApduCase, CommandApduFrame, CommandDomain, CommandKind,
        SecureMessagingProtocol as SecureMessagingProtocolProto, StatusWordClass,
    };
    use jcim_core::error::JcimError;
    use jcim_core::iso7816::{self, StatusWord};

    use super::{
        command_apdu_from_proto, secure_messaging_protocol_from_proto, status_word_info, to_status,
    };

    #[test]
    fn to_status_maps_public_error_classes_to_transport_codes() {
        let invalid = to_status(JcimError::InvalidApdu("bad apdu".to_string()));
        assert_eq!(invalid.code(), Code::InvalidArgument);
        assert_eq!(invalid.message(), "bad apdu");

        let unavailable = to_status(JcimError::BackendStartup("bundle missing".to_string()));
        assert_eq!(unavailable.code(), Code::Unavailable);
        assert_eq!(unavailable.message(), "bundle missing");

        let internal = to_status(JcimError::MissingStatePath(PathBuf::from(
            "/tmp/runtime.toml",
        )));
        assert_eq!(internal.code(), Code::Internal);
        assert!(internal.message().contains("/tmp/runtime.toml"));
    }

    #[test]
    fn status_word_info_preserves_retry_and_length_hints() {
        let remaining = status_word_info(StatusWord::new(0x6110));
        assert_eq!(remaining.class, StatusWordClass::NormalProcessing as i32);
        assert_eq!(remaining.label, "response_bytes_available");
        assert_eq!(remaining.remaining_response_bytes, Some(16));
        assert!(remaining.success);

        let retry = status_word_info(StatusWord::new(0x63C2));
        assert_eq!(retry.class, StatusWordClass::Warning as i32);
        assert_eq!(retry.label, "verify_failed_retries_remaining");
        assert_eq!(retry.retry_counter, Some(2));
        assert!(retry.warning);

        let exact_length = status_word_info(StatusWord::new(0x6C20));
        assert_eq!(exact_length.class, StatusWordClass::CheckingError as i32);
        assert_eq!(exact_length.label, "correct_length_hint");
        assert_eq!(exact_length.exact_length_hint, Some(32));
    }

    #[test]
    fn command_apdu_from_proto_accepts_valid_structured_frames() {
        let mut frame = select_command_frame();
        frame.raw.clear();

        let command = command_apdu_from_proto(Some(frame)).expect("valid structured frame");
        assert_eq!(
            command.to_bytes(),
            [
                0x00, 0xA4, 0x04, 0x00, 0x09, 0x53, 0x61, 0x74, 0x6F, 0x43, 0x68, 0x69, 0x70, 0x00,
            ]
        );
    }

    #[test]
    fn command_apdu_from_proto_rejects_mismatched_metadata() {
        let mut frame = select_command_frame();
        frame.kind = CommandKind::GetResponse as i32;
        let kind_error =
            command_apdu_from_proto(Some(frame)).expect_err("kind mismatch should fail");
        assert_eq!(kind_error.code(), Code::InvalidArgument);
        assert!(kind_error.message().contains("command kind"));

        let mut frame = select_command_frame();
        frame.logical_channel = 1;
        let channel_error =
            command_apdu_from_proto(Some(frame)).expect_err("channel mismatch should fail");
        assert_eq!(channel_error.code(), Code::InvalidArgument);
        assert!(channel_error.message().contains("logical channel metadata"));
    }

    #[test]
    fn secure_messaging_protocol_from_proto_preserves_custom_labels() {
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Iso7816 as i32,
                "ignored",
            ),
            Some(jcim_core::iso7816::SecureMessagingProtocol::Iso7816)
        );
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Other as i32,
                "scp-custom",
            ),
            Some(jcim_core::iso7816::SecureMessagingProtocol::Other(
                "scp-custom".to_string()
            ))
        );
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Unspecified as i32,
                "",
            ),
            None
        );
    }

    fn select_command_frame() -> CommandApduFrame {
        let command = jcim_core::apdu::CommandApdu::parse(&[
            0x00, 0xA4, 0x04, 0x00, 0x09, 0x53, 0x61, 0x74, 0x6F, 0x43, 0x68, 0x69, 0x70, 0x00,
        ])
        .expect("parse select");
        let descriptor = iso7816::describe_command(&command);
        CommandApduFrame {
            raw: command.to_bytes(),
            cla: u32::from(command.cla),
            ins: u32::from(command.ins),
            p1: u32::from(command.p1),
            p2: u32::from(command.p2),
            data: command.data.clone(),
            ne: command.ne.map(|value| value as u32),
            encoding: ApduEncoding::Short as i32,
            apdu_case: CommandApduCase::CommandApduCase3Short as i32,
            domain: match descriptor.domain {
                jcim_core::iso7816::CommandDomain::Iso7816 => CommandDomain::Iso7816 as i32,
                jcim_core::iso7816::CommandDomain::GlobalPlatform => {
                    CommandDomain::GlobalPlatform as i32
                }
                jcim_core::iso7816::CommandDomain::Opaque => CommandDomain::Opaque as i32,
            },
            kind: match descriptor.kind {
                jcim_core::iso7816::CommandKind::Select => CommandKind::Select as i32,
                jcim_core::iso7816::CommandKind::ManageChannel => CommandKind::ManageChannel as i32,
                jcim_core::iso7816::CommandKind::GetResponse => CommandKind::GetResponse as i32,
                jcim_core::iso7816::CommandKind::ReadBinary => CommandKind::ReadBinary as i32,
                jcim_core::iso7816::CommandKind::WriteBinary => CommandKind::WriteBinary as i32,
                jcim_core::iso7816::CommandKind::UpdateBinary => CommandKind::UpdateBinary as i32,
                jcim_core::iso7816::CommandKind::EraseBinary => CommandKind::EraseBinary as i32,
                jcim_core::iso7816::CommandKind::ReadRecord => CommandKind::ReadRecord as i32,
                jcim_core::iso7816::CommandKind::UpdateRecord => CommandKind::UpdateRecord as i32,
                jcim_core::iso7816::CommandKind::AppendRecord => CommandKind::AppendRecord as i32,
                jcim_core::iso7816::CommandKind::SearchRecord => CommandKind::SearchRecord as i32,
                jcim_core::iso7816::CommandKind::GetData => CommandKind::GetData as i32,
                jcim_core::iso7816::CommandKind::PutData => CommandKind::PutData as i32,
                jcim_core::iso7816::CommandKind::Verify => CommandKind::Verify as i32,
                jcim_core::iso7816::CommandKind::ChangeReferenceData => {
                    CommandKind::ChangeReferenceData as i32
                }
                jcim_core::iso7816::CommandKind::ResetRetryCounter => {
                    CommandKind::ResetRetryCounter as i32
                }
                jcim_core::iso7816::CommandKind::InternalAuthenticate => {
                    CommandKind::InternalAuthenticate as i32
                }
                jcim_core::iso7816::CommandKind::ExternalAuthenticate => {
                    CommandKind::ExternalAuthenticate as i32
                }
                jcim_core::iso7816::CommandKind::GetChallenge => CommandKind::GetChallenge as i32,
                jcim_core::iso7816::CommandKind::Envelope => CommandKind::Envelope as i32,
                jcim_core::iso7816::CommandKind::GpGetStatus => CommandKind::GpGetStatus as i32,
                jcim_core::iso7816::CommandKind::GpSetStatus => CommandKind::GpSetStatus as i32,
                jcim_core::iso7816::CommandKind::GpInitializeUpdate => {
                    CommandKind::GpInitializeUpdate as i32
                }
                jcim_core::iso7816::CommandKind::GpExternalAuthenticate => {
                    CommandKind::GpExternalAuthenticate as i32
                }
                jcim_core::iso7816::CommandKind::Opaque => CommandKind::Opaque as i32,
            },
            logical_channel: u32::from(descriptor.logical_channel),
        }
    }
}
