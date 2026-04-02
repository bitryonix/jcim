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

/// Encode one SDK project selector into the maintained protobuf selector shape.
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

/// Encode one simulation id into the maintained protobuf selector shape.
pub(super) fn simulation_selector(simulation_id: String) -> SimulationSelector {
    SimulationSelector { simulation_id }
}

/// Decode one protobuf project payload into the stable SDK summary type.
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

/// Decode one protobuf artifact payload into the stable SDK summary type.
pub(super) fn artifact_summary(artifact: jcim_api::v0_3::Artifact) -> Result<ArtifactSummary> {
    Ok(ArtifactSummary {
        kind: artifact.kind,
        path: owned_path(artifact.path),
    })
}

/// Decode one protobuf simulation payload into the stable SDK summary type.
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

/// Encode an optional secure-messaging protocol into protobuf enum and custom-label fields.
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

/// Encode one typed command APDU into the maintained protobuf frame with descriptor metadata.
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

/// Decode one protobuf response APDU frame from raw bytes or structured fields.
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

/// Decode one simulation reset response into the unified SDK reset summary.
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

/// Decode one physical-card reset response into the unified SDK reset summary.
pub(super) fn reset_summary_from_card_proto(response: ResetCardResponse) -> Result<ResetSummary> {
    let atr = atr_from_proto(response.atr)?;
    let session_state = iso_session_state_from_proto(response.session_state)?;
    Ok(ResetSummary {
        atr: atr.or_else(|| session_state.atr.clone()),
        session_state,
    })
}

/// Decode one GP secure-channel protobuf payload into the stable SDK summary type.
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

/// Decode one service-status response into the stable SDK summary type.
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

/// Decode one toolchain-setup response into the stable SDK summary type.
pub(super) fn setup_summary(response: jcim_api::v0_3::SetupToolchainsResponse) -> SetupSummary {
    SetupSummary {
        config_path: owned_path(response.config_path),
        message: response.message,
    }
}

/// Decode an optional protobuf ATR payload into the typed SDK ATR model.
pub(super) fn atr_from_proto(info: Option<jcim_api::v0_3::AtrInfo>) -> Result<Option<Atr>> {
    info.map(|value| Atr::parse(&value.raw).map_err(JcimSdkError::from))
        .transpose()
}

/// Decode optional active-protocol parameters from the protobuf transport shape.
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

/// Decode protobuf ISO capability flags into the typed SDK capability model.
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

/// Decode one protobuf ISO session-state payload into the typed SDK session model.
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

/// Decode an optional protobuf AID payload, treating empty raw bytes as absent.
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

/// Decode one protobuf file-selection payload into the typed ISO selection model.
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

/// Decode one protobuf transport-protocol enum into the typed ISO transport model.
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

/// Decode protobuf secure-messaging protocol fields into the typed ISO protocol model.
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jcim_api::v0_3::file_selection_info::Selection as FileSelectionSelection;
    use jcim_api::v0_3::{
        AidInfo, ApduEncoding, AtrInfo, CommandApduCase, CommandDomain, CommandKind,
        FileSelectionInfo, GetServiceStatusResponse, IsoSessionStateInfo, LogicalChannelStateInfo,
        PowerState as PowerStateProto, ProjectSelector, ProtocolParametersInfo, ResponseApduFrame,
        RetryCounterInfo, SecureMessagingProtocol as SecureMessagingProtocolProto,
        SecureMessagingStateInfo, SimulationInfo, StatusWordClass, StatusWordInfo,
        TransportProtocol,
    };
    use jcim_core::aid::Aid;
    use jcim_core::apdu::ResponseApdu;
    use jcim_core::iso7816::{
        self, FileSelection, IsoSessionState, PowerState, SecureMessagingProtocol, StatusWord,
    };

    use super::*;

    #[test]
    fn selector_helpers_preserve_path_and_id_shapes() {
        let project = project_selector(&crate::types::ProjectRef::from_path(
            "examples/satochip/workdir",
        ));
        assert_eq!(
            project,
            ProjectSelector {
                project_path: PathBuf::from("examples/satochip/workdir")
                    .display()
                    .to_string(),
                project_id: String::new(),
            }
        );

        let simulation = simulation_selector("sim-42".to_string());
        assert_eq!(simulation.simulation_id, "sim-42");
    }

    #[test]
    fn simulation_summary_maps_unknown_status_to_unknown() {
        let summary = simulation_summary(SimulationInfo {
            simulation_id: "sim-1".to_string(),
            project_id: "proj-1".to_string(),
            project_path: "/tmp/project".to_string(),
            status: 999,
            reader_name: "Reader".to_string(),
            health: "healthy".to_string(),
            atr: None,
            active_protocol: None,
            iso_capabilities: None,
            session_state: None,
            package_count: 1,
            applet_count: 2,
            package_name: "pkg".to_string(),
            package_aid: "A0000001510001".to_string(),
            recent_events: vec!["info: started".to_string()],
        })
        .expect("summary");

        assert_eq!(summary.status, crate::types::SimulationStatus::Unknown);
        assert_eq!(
            summary.iso_capabilities,
            jcim_core::iso7816::IsoCapabilities::default()
        );
        assert_eq!(summary.session_state, IsoSessionState::default());
    }

    #[test]
    fn response_apdu_from_proto_accepts_raw_and_structured_forms() {
        let raw = response_apdu_from_proto(Some(ResponseApduFrame {
            raw: vec![0x01, 0x02, 0x90, 0x00],
            data: vec![0xFF],
            sw: 0x6A82,
            status: None,
        }))
        .expect("raw response");
        assert_eq!(raw, ResponseApdu::success(vec![0x01, 0x02]));

        let structured = response_apdu_from_proto(Some(ResponseApduFrame {
            raw: Vec::new(),
            data: vec![0xCA, 0xFE],
            sw: 0x9000,
            status: None,
        }))
        .expect("structured response");
        assert_eq!(
            structured,
            ResponseApdu {
                data: vec![0xCA, 0xFE],
                sw: 0x9000,
            }
        );
    }

    #[test]
    fn secure_messaging_protocol_helpers_preserve_custom_labels() {
        assert_eq!(
            secure_messaging_protocol_fields(Some(&SecureMessagingProtocol::Other(
                "scp-custom".to_string()
            ))),
            (
                SecureMessagingProtocolProto::Other as i32,
                "scp-custom".to_string()
            )
        );
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Other as i32,
                "scp-custom",
            ),
            Some(SecureMessagingProtocol::Other("scp-custom".to_string()))
        );
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Unspecified as i32,
                "",
            ),
            None
        );
    }

    #[test]
    fn service_status_summary_preserves_paths_and_fingerprint() {
        let summary = service_status_summary(GetServiceStatusResponse {
            socket_path: "/tmp/jcim.sock".to_string(),
            running: true,
            known_project_count: 3,
            active_simulation_count: 2,
            service_binary_path: "/tmp/jcimd".to_string(),
            service_binary_fingerprint: "123:456:789".to_string(),
        })
        .expect("service summary");

        assert_eq!(summary.socket_path, PathBuf::from("/tmp/jcim.sock"));
        assert!(summary.running);
        assert_eq!(summary.known_project_count, 3);
        assert_eq!(summary.active_simulation_count, 2);
        assert_eq!(summary.service_binary_path, PathBuf::from("/tmp/jcimd"));
        assert_eq!(summary.service_binary_fingerprint, "123:456:789");
    }

    #[test]
    fn iso_session_state_from_proto_reconstructs_nested_state() {
        let atr_raw = vec![0x3B, 0x80, 0x01, 0x00];
        let state = iso_session_state_from_proto(Some(IsoSessionStateInfo {
            power_state: PowerStateProto::On as i32,
            atr: Some(AtrInfo {
                raw: atr_raw.clone(),
                hex: String::new(),
                convention: jcim_api::v0_3::TransmissionConvention::Direct as i32,
                interface_groups: Vec::new(),
                historical_bytes: Vec::new(),
                checksum_tck: None,
                protocols: vec![TransportProtocol::T1 as i32],
            }),
            active_protocol: Some(ProtocolParametersInfo {
                protocol: TransportProtocol::T1 as i32,
                fi: Some(9),
                di: Some(4),
                waiting_integer: Some(5),
                ifsc: Some(32),
            }),
            selected_aid: Some(AidInfo {
                raw: Aid::from_hex("A000000151000001")
                    .expect("aid")
                    .as_bytes()
                    .to_vec(),
                hex: "A000000151000001".to_string(),
            }),
            current_file: Some(FileSelectionInfo {
                selection: Some(FileSelectionSelection::FileId(0x3F00)),
            }),
            open_channels: vec![LogicalChannelStateInfo {
                channel_number: 1,
                selected_aid: Some(AidInfo {
                    raw: Aid::from_hex("A000000151000002")
                        .expect("aid")
                        .as_bytes()
                        .to_vec(),
                    hex: "A000000151000002".to_string(),
                }),
                current_file: Some(FileSelectionInfo {
                    selection: Some(FileSelectionSelection::ByName(vec![0xA0, 0x00])),
                }),
            }],
            secure_messaging: Some(SecureMessagingStateInfo {
                active: true,
                protocol: SecureMessagingProtocolProto::Other as i32,
                security_level: Some(0x13),
                session_id: "secure-1".to_string(),
                command_counter: 7,
                protocol_label: "scp-custom".to_string(),
            }),
            verified_references: vec![0x81, 0x82],
            retry_counters: vec![RetryCounterInfo {
                reference: 0x81,
                remaining: 3,
            }],
            last_status: Some(StatusWordInfo {
                value: 0x63C2,
                class: StatusWordClass::Warning as i32,
                label: "verify_failed_retries_remaining".to_string(),
                success: false,
                warning: true,
                remaining_response_bytes: None,
                retry_counter: Some(2),
                exact_length_hint: None,
            }),
        }))
        .expect("session state");

        assert_eq!(state.power_state, PowerState::On);
        assert_eq!(state.atr.as_ref().expect("atr").raw, atr_raw);
        assert_eq!(
            state.active_protocol.as_ref().expect("protocol").protocol,
            Some(jcim_core::iso7816::TransportProtocol::T1)
        );
        assert_eq!(state.current_file, Some(FileSelection::FileId(0x3F00)));
        assert_eq!(state.open_channels.len(), 1);
        assert_eq!(
            state.open_channels[0].current_file,
            Some(FileSelection::ByName(vec![0xA0, 0x00]))
        );
        assert_eq!(
            state.secure_messaging.protocol,
            Some(SecureMessagingProtocol::Other("scp-custom".to_string()))
        );
        assert_eq!(state.secure_messaging.command_counter, 7);
        assert_eq!(state.verified_references, vec![0x81, 0x82]);
        assert_eq!(state.retry_counters[0].remaining, 3);
        assert_eq!(state.last_status, Some(StatusWord::new(0x63C2)));
    }

    #[test]
    fn command_apdu_frame_preserves_descriptor_metadata() {
        let aid = Aid::from_hex("A000000151000001").expect("aid");
        let command = iso7816::select_by_name(&aid);
        let frame = command_apdu_frame(&command);

        assert_eq!(frame.encoding, ApduEncoding::Short as i32);
        assert_eq!(
            frame.apdu_case,
            CommandApduCase::CommandApduCase4Short as i32
        );
        assert_eq!(frame.domain, CommandDomain::Iso7816 as i32);
        assert_eq!(frame.kind, CommandKind::Select as i32);
        assert_eq!(frame.logical_channel, 0);
    }
}
