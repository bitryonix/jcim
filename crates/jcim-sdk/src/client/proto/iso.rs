use jcim_core::aid::Aid;
use jcim_core::iso7816::{
    Atr, FileSelection, IsoCapabilities, IsoSessionState, LogicalChannelState, PowerState,
    ProtocolParameters, RetryCounterState, SecureMessagingProtocol, SecureMessagingState,
    StatusWord, TransportProtocol,
};

use crate::error::{JcimSdkError, Result};

/// Decode an optional protobuf ATR payload into the typed SDK ATR model.
pub(in crate::client) fn atr_from_proto(
    info: Option<jcim_api::v0_3::AtrInfo>,
) -> Result<Option<Atr>> {
    info.map(|value| Atr::parse(&value.raw).map_err(JcimSdkError::from))
        .transpose()
}

/// Decode optional active-protocol parameters from the protobuf transport shape.
pub(in crate::client) fn protocol_parameters_from_proto(
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
pub(in crate::client) fn iso_capabilities_from_proto(
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
pub(in crate::client) fn iso_session_state_from_proto(
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

/// Decode protobuf secure-messaging protocol fields into the typed ISO protocol model.
pub(in crate::client) fn secure_messaging_protocol_from_proto(
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

/// Decode an optional protobuf AID payload, treating empty raw bytes as absent.
pub(in crate::client) fn aid_from_proto(
    info: Option<jcim_api::v0_3::AidInfo>,
) -> Result<Option<Aid>> {
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
