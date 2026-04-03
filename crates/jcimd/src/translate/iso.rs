use jcim_api::v0_3::file_selection_info::Selection as FileSelectionProto;
use jcim_api::v0_3::{
    AidInfo, AtrInfo, AtrInterfaceGroup, FileSelectionInfo, IsoCapabilitiesInfo,
    IsoSessionStateInfo, LogicalChannelStateInfo, ProtocolParametersInfo, RetryCounterInfo,
    SecureMessagingStateInfo, StatusWordInfo,
};
use jcim_core::aid::Aid;
use jcim_core::iso7816::{
    Atr, FileSelection, IsoCapabilities, IsoSessionState, LogicalChannelState, PowerState,
    ProtocolParameters, RetryCounterState, SecureMessagingProtocol, SecureMessagingState,
    StatusWord, StatusWordClass, TransmissionConvention, TransportProtocol,
};

/// Encode one status word plus its derived class and hints into the RPC model.
pub(crate) fn status_word_info(status: StatusWord) -> StatusWordInfo {
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

/// Encode one AID into the RPC helper shape used across multiple transport responses.
pub(super) fn aid_info(aid: &Aid) -> AidInfo {
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
