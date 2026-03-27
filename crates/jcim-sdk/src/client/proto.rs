use jcim_api::v0_3::{
    GetServiceStatusResponse, ProjectSelector, ResetCardResponse, ResetSimulationResponse,
    SimulationSelector,
};
use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::globalplatform;
use jcim_core::iso7816::{
    self, Atr, CommandDomain, CommandKind, FileSelection, IsoCapabilities, IsoSessionState,
    LogicalChannelState, PowerState, ProtocolParameters, RetryCounterState,
    SecureMessagingProtocol, SecureMessagingState, StatusWord, TransportProtocol,
};

use crate::error::{JcimSdkError, Result};
use crate::types::{
    AppletSummary, ArtifactSummary, GpSecureChannelSummary, ProjectRef, ProjectSummary,
    ResetSummary, ServiceStatusSummary, SetupSummary, SimulationStatus, SimulationSummary,
    owned_path,
};

pub(super) fn project_selector(project: &ProjectRef) -> ProjectSelector {
    ProjectSelector {
        project_path: project
            .project_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        project_id: project.project_id.clone().unwrap_or_default(),
    }
}

pub(super) fn simulation_selector(simulation_id: String) -> SimulationSelector {
    SimulationSelector { simulation_id }
}

pub(super) fn project_summary(project: jcim_api::v0_3::ProjectInfo) -> Result<ProjectSummary> {
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

pub(super) fn artifact_summary(artifact: jcim_api::v0_3::Artifact) -> Result<ArtifactSummary> {
    Ok(ArtifactSummary {
        kind: artifact.kind,
        path: owned_path(artifact.path),
    })
}

pub(super) fn simulation_summary(
    simulation: jcim_api::v0_3::SimulationInfo,
) -> Result<SimulationSummary> {
    Ok(SimulationSummary {
        simulation_id: simulation.simulation_id,
        project_id: simulation.project_id,
        project_path: owned_path(simulation.project_path),
        status: match jcim_api::v0_3::SimulationStatus::try_from(simulation.status) {
            Ok(jcim_api::v0_3::SimulationStatus::Starting) => SimulationStatus::Starting,
            Ok(jcim_api::v0_3::SimulationStatus::Running) => SimulationStatus::Running,
            Ok(jcim_api::v0_3::SimulationStatus::Stopped) => SimulationStatus::Stopped,
            Ok(jcim_api::v0_3::SimulationStatus::Failed) => SimulationStatus::Failed,
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

pub(super) fn secure_messaging_protocol_fields(
    protocol: Option<&SecureMessagingProtocol>,
) -> (i32, String) {
    match protocol {
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
    }
}

pub(super) fn command_apdu_frame(apdu: &CommandApdu) -> jcim_api::v0_3::CommandApduFrame {
    let descriptor = iso7816::describe_command(apdu);
    jcim_api::v0_3::CommandApduFrame {
        raw: apdu.to_bytes(),
        cla: u32::from(apdu.cla),
        ins: u32::from(apdu.ins),
        p1: u32::from(apdu.p1),
        p2: u32::from(apdu.p2),
        data: apdu.data.clone(),
        ne: apdu.ne.map(|value| value as u32),
        encoding: match apdu.encoding {
            jcim_core::apdu::ApduEncoding::Short => jcim_api::v0_3::ApduEncoding::Short as i32,
            jcim_core::apdu::ApduEncoding::Extended => {
                jcim_api::v0_3::ApduEncoding::Extended as i32
            }
        },
        apdu_case: match apdu.apdu_case() {
            jcim_core::apdu::CommandApduCase::Case1 => {
                jcim_api::v0_3::CommandApduCase::CommandApduCase1 as i32
            }
            jcim_core::apdu::CommandApduCase::Case2Short => {
                jcim_api::v0_3::CommandApduCase::CommandApduCase2Short as i32
            }
            jcim_core::apdu::CommandApduCase::Case3Short => {
                jcim_api::v0_3::CommandApduCase::CommandApduCase3Short as i32
            }
            jcim_core::apdu::CommandApduCase::Case4Short => {
                jcim_api::v0_3::CommandApduCase::CommandApduCase4Short as i32
            }
            jcim_core::apdu::CommandApduCase::Case2Extended => {
                jcim_api::v0_3::CommandApduCase::CommandApduCase2Extended as i32
            }
            jcim_core::apdu::CommandApduCase::Case3Extended => {
                jcim_api::v0_3::CommandApduCase::CommandApduCase3Extended as i32
            }
            jcim_core::apdu::CommandApduCase::Case4Extended => {
                jcim_api::v0_3::CommandApduCase::CommandApduCase4Extended as i32
            }
        },
        domain: match descriptor.domain {
            CommandDomain::Iso7816 => jcim_api::v0_3::CommandDomain::Iso7816 as i32,
            CommandDomain::GlobalPlatform => jcim_api::v0_3::CommandDomain::GlobalPlatform as i32,
            CommandDomain::Opaque => jcim_api::v0_3::CommandDomain::Opaque as i32,
        },
        kind: match descriptor.kind {
            CommandKind::Select => jcim_api::v0_3::CommandKind::Select as i32,
            CommandKind::ManageChannel => jcim_api::v0_3::CommandKind::ManageChannel as i32,
            CommandKind::GetResponse => jcim_api::v0_3::CommandKind::GetResponse as i32,
            CommandKind::ReadBinary => jcim_api::v0_3::CommandKind::ReadBinary as i32,
            CommandKind::WriteBinary => jcim_api::v0_3::CommandKind::WriteBinary as i32,
            CommandKind::UpdateBinary => jcim_api::v0_3::CommandKind::UpdateBinary as i32,
            CommandKind::EraseBinary => jcim_api::v0_3::CommandKind::EraseBinary as i32,
            CommandKind::ReadRecord => jcim_api::v0_3::CommandKind::ReadRecord as i32,
            CommandKind::UpdateRecord => jcim_api::v0_3::CommandKind::UpdateRecord as i32,
            CommandKind::AppendRecord => jcim_api::v0_3::CommandKind::AppendRecord as i32,
            CommandKind::SearchRecord => jcim_api::v0_3::CommandKind::SearchRecord as i32,
            CommandKind::GetData => jcim_api::v0_3::CommandKind::GetData as i32,
            CommandKind::PutData => jcim_api::v0_3::CommandKind::PutData as i32,
            CommandKind::Verify => jcim_api::v0_3::CommandKind::Verify as i32,
            CommandKind::ChangeReferenceData => {
                jcim_api::v0_3::CommandKind::ChangeReferenceData as i32
            }
            CommandKind::ResetRetryCounter => jcim_api::v0_3::CommandKind::ResetRetryCounter as i32,
            CommandKind::InternalAuthenticate => {
                jcim_api::v0_3::CommandKind::InternalAuthenticate as i32
            }
            CommandKind::ExternalAuthenticate => {
                jcim_api::v0_3::CommandKind::ExternalAuthenticate as i32
            }
            CommandKind::GetChallenge => jcim_api::v0_3::CommandKind::GetChallenge as i32,
            CommandKind::Envelope => jcim_api::v0_3::CommandKind::Envelope as i32,
            CommandKind::GpGetStatus => jcim_api::v0_3::CommandKind::GpGetStatus as i32,
            CommandKind::GpSetStatus => jcim_api::v0_3::CommandKind::GpSetStatus as i32,
            CommandKind::GpInitializeUpdate => {
                jcim_api::v0_3::CommandKind::GpInitializeUpdate as i32
            }
            CommandKind::GpExternalAuthenticate => {
                jcim_api::v0_3::CommandKind::GpExternalAuthenticate as i32
            }
            CommandKind::Opaque => jcim_api::v0_3::CommandKind::Opaque as i32,
        },
        logical_channel: u32::from(descriptor.logical_channel),
    }
}

pub(super) fn response_apdu_from_proto(
    frame: Option<jcim_api::v0_3::ResponseApduFrame>,
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

pub(super) fn reset_summary_from_simulation_proto(
    response: ResetSimulationResponse,
) -> Result<ResetSummary> {
    let atr = atr_from_proto(response.atr)?;
    let session_state = iso_session_state_from_proto(response.session_state)?;
    Ok(ResetSummary {
        atr: atr.or_else(|| session_state.atr.clone()),
        session_state,
    })
}

pub(super) fn reset_summary_from_card_proto(response: ResetCardResponse) -> Result<ResetSummary> {
    let atr = atr_from_proto(response.atr)?;
    let session_state = iso_session_state_from_proto(response.session_state)?;
    Ok(ResetSummary {
        atr: atr.or_else(|| session_state.atr.clone()),
        session_state,
    })
}

pub(super) fn gp_secure_channel_from_proto(
    info: Option<jcim_api::v0_3::GpSecureChannelInfo>,
) -> Result<GpSecureChannelSummary> {
    let info = info.ok_or_else(|| {
        JcimSdkError::InvalidResponse(
            "missing GP secure-channel summary in service response".to_string(),
        )
    })?;
    let protocol = match jcim_api::v0_3::SecureMessagingProtocol::try_from(info.protocol).ok() {
        Some(jcim_api::v0_3::SecureMessagingProtocol::Scp02) => globalplatform::ScpMode::Scp02,
        Some(jcim_api::v0_3::SecureMessagingProtocol::Scp03) => globalplatform::ScpMode::Scp03,
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

pub(super) fn service_status_summary(
    response: GetServiceStatusResponse,
) -> Result<ServiceStatusSummary> {
    let GetServiceStatusResponse {
        socket_path,
        running,
        known_project_count,
        active_simulation_count,
        service_binary_path,
        service_binary_fingerprint,
    } = response;
    Ok(ServiceStatusSummary {
        socket_path: owned_path(socket_path),
        running,
        known_project_count,
        active_simulation_count,
        service_binary_path: owned_path(service_binary_path),
        service_binary_fingerprint,
    })
}

pub(super) fn setup_summary(response: jcim_api::v0_3::SetupToolchainsResponse) -> SetupSummary {
    SetupSummary {
        config_path: owned_path(response.config_path),
        message: response.message,
    }
}

pub(super) fn atr_from_proto(info: Option<jcim_api::v0_3::AtrInfo>) -> Result<Option<Atr>> {
    info.map(|value| Atr::parse(&value.raw).map_err(JcimSdkError::from))
        .transpose()
}

pub(super) fn protocol_parameters_from_proto(
    info: Option<jcim_api::v0_3::ProtocolParametersInfo>,
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

pub(super) fn iso_capabilities_from_proto(
    info: Option<jcim_api::v0_3::IsoCapabilitiesInfo>,
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

pub(super) fn iso_session_state_from_proto(
    info: Option<jcim_api::v0_3::IsoSessionStateInfo>,
) -> Result<IsoSessionState> {
    let Some(info) = info else {
        return Ok(IsoSessionState::default());
    };
    Ok(IsoSessionState {
        power_state: match jcim_api::v0_3::PowerState::try_from(info.power_state) {
            Ok(jcim_api::v0_3::PowerState::On) => PowerState::On,
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

fn aid_from_proto(info: Option<jcim_api::v0_3::AidInfo>) -> Result<Option<Aid>> {
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
    info: Option<jcim_api::v0_3::FileSelectionInfo>,
) -> Option<FileSelection> {
    use jcim_api::v0_3::file_selection_info::Selection;

    match info.and_then(|info| info.selection) {
        Some(Selection::ByName(bytes)) => Some(FileSelection::ByName(bytes)),
        Some(Selection::FileId(file_id)) => Some(FileSelection::FileId(file_id as u16)),
        Some(Selection::Path(bytes)) => Some(FileSelection::Path(bytes)),
        None => None,
    }
}

fn transport_protocol_from_proto(value: i32) -> Option<TransportProtocol> {
    match jcim_api::v0_3::TransportProtocol::try_from(value).ok()? {
        jcim_api::v0_3::TransportProtocol::T0 => Some(TransportProtocol::T0),
        jcim_api::v0_3::TransportProtocol::T1 => Some(TransportProtocol::T1),
        jcim_api::v0_3::TransportProtocol::T2 => Some(TransportProtocol::T2),
        jcim_api::v0_3::TransportProtocol::T3 => Some(TransportProtocol::T3),
        jcim_api::v0_3::TransportProtocol::T14 => Some(TransportProtocol::T14),
        jcim_api::v0_3::TransportProtocol::Other => Some(TransportProtocol::Other(0xFF)),
        jcim_api::v0_3::TransportProtocol::Unspecified => None,
    }
}

fn secure_messaging_protocol_from_proto(
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
