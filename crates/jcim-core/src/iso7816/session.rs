use serde::{Deserialize, Serialize};

use crate::aid::Aid;
use crate::apdu::{CommandApdu, ResponseApdu};
use crate::error::{JcimError, Result};

use super::atr::{Atr, TransportProtocol};
use super::commands::{IsoCommand, decode_command, describe_command};
use super::secure_messaging::{SecureMessagingProtocol, SecureMessagingState};
use super::selection::FileSelection;
use super::status_word::StatusWord;

/// Transport-parameter summary derived from ATR or runtime state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct ProtocolParameters {
    /// Active transport protocol.
    pub protocol: Option<TransportProtocol>,
    /// FI code extracted from TA1 when known.
    pub fi: Option<u8>,
    /// DI code extracted from TA1 when known.
    pub di: Option<u8>,
    /// Waiting integer when known.
    pub waiting_integer: Option<u8>,
    /// IFSC when known.
    pub ifsc: Option<u8>,
}

impl ProtocolParameters {
    /// Derive one protocol summary from a parsed ATR.
    pub fn from_atr(atr: &Atr) -> Self {
        let first = atr.interface_groups.first();
        let fi = first.and_then(|group| group.ta.map(|value| value >> 4));
        let di = first.and_then(|group| group.ta.map(|value| value & 0x0F));
        let waiting_integer = first.and_then(|group| group.tc);
        let ifsc = atr
            .interface_groups
            .iter()
            .find(|group| group.protocol == Some(TransportProtocol::T1))
            .and_then(|group| group.ta);
        Self {
            protocol: atr.default_protocol(),
            fi,
            di,
            waiting_integer,
            ifsc,
        }
    }
}

/// Current card power state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerState {
    /// Card is powered off or absent.
    #[default]
    Off,
    /// Card is powered and can answer requests.
    On,
}

/// Retry counter state for one reference data object such as a PIN.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RetryCounterState {
    /// Reference identifier used in P2.
    pub reference: u8,
    /// Remaining retries when known.
    pub remaining: u8,
}

/// One open logical channel summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogicalChannelState {
    /// Channel number.
    pub channel_number: u8,
    /// Current selected AID or DF name on the channel.
    pub selected_aid: Option<Aid>,
    /// Current file selection on the channel when tracked.
    pub current_file: Option<FileSelection>,
}

/// Explicit capability summary for ISO/IEC 7816 session features.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IsoCapabilities {
    /// Supported transport protocols.
    pub protocols: Vec<TransportProtocol>,
    /// Whether extended-length APDUs are supported.
    pub extended_length: bool,
    /// Whether logical channels are supported.
    pub logical_channels: bool,
    /// Maximum logical channels, including the basic channel.
    pub max_logical_channels: u8,
    /// Whether secure messaging is supported.
    pub secure_messaging: bool,
    /// Whether JCIM can expose file-model state.
    pub file_model_visibility: bool,
    /// Whether raw APDU passthrough is supported.
    pub raw_apdu: bool,
}

impl Default for IsoCapabilities {
    fn default() -> Self {
        Self {
            protocols: vec![TransportProtocol::T1],
            extended_length: false,
            logical_channels: false,
            max_logical_channels: 1,
            secure_messaging: false,
            file_model_visibility: false,
            raw_apdu: true,
        }
    }
}

/// Current tracked ISO/IEC 7816 session state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct IsoSessionState {
    /// Current power state.
    pub power_state: PowerState,
    /// Parsed ATR when available.
    pub atr: Option<Atr>,
    /// Active protocol parameters when available.
    pub active_protocol: Option<ProtocolParameters>,
    /// Selected AID on the basic channel when available.
    pub selected_aid: Option<Aid>,
    /// Current selected file on the basic channel when tracked.
    pub current_file: Option<FileSelection>,
    /// Open logical channels and their selection state.
    pub open_channels: Vec<LogicalChannelState>,
    /// Secure-messaging session summary.
    pub secure_messaging: SecureMessagingState,
    /// References currently verified in the session.
    pub verified_references: Vec<u8>,
    /// Retry counters known from recent responses.
    pub retry_counters: Vec<RetryCounterState>,
    /// Last observed status word.
    pub last_status: Option<StatusWord>,
}

impl IsoSessionState {
    /// Build one reset session state from ATR and protocol metadata.
    pub fn reset(atr: Option<Atr>, active_protocol: Option<ProtocolParameters>) -> Self {
        Self {
            power_state: PowerState::On,
            atr,
            active_protocol,
            selected_aid: None,
            current_file: None,
            open_channels: vec![LogicalChannelState {
                channel_number: 0,
                selected_aid: None,
                current_file: None,
            }],
            secure_messaging: SecureMessagingState::default(),
            verified_references: Vec::new(),
            retry_counters: Vec::new(),
            last_status: None,
        }
    }
}

/// Apply one response to the tracked ISO session state.
pub fn apply_response_to_session(
    state: &mut IsoSessionState,
    command: &CommandApdu,
    response: &ResponseApdu,
) -> Result<()> {
    state.last_status = Some(response.status_word());
    let descriptor = describe_command(command);
    let channel = descriptor.logical_channel;
    ensure_channel_entry(state, channel);

    match decode_command(command)? {
        IsoCommand::Select(select) if response.is_success() => match select.target {
            FileSelection::ByName(name) => {
                if let Ok(aid) = Aid::from_slice(&name) {
                    state.selected_aid = Some(aid.clone());
                    if let Some(entry) = state
                        .open_channels
                        .iter_mut()
                        .find(|entry| entry.channel_number == channel)
                    {
                        entry.selected_aid = Some(aid);
                        entry.current_file = None;
                    }
                    state.current_file = None;
                } else {
                    let selection = FileSelection::ByName(name);
                    state.current_file = Some(selection.clone());
                    if let Some(entry) = state
                        .open_channels
                        .iter_mut()
                        .find(|entry| entry.channel_number == channel)
                    {
                        entry.current_file = Some(selection);
                    }
                }
            }
            other => {
                state.current_file = Some(other.clone());
                if let Some(entry) = state
                    .open_channels
                    .iter_mut()
                    .find(|entry| entry.channel_number == channel)
                {
                    entry.current_file = Some(other);
                }
            }
        },
        IsoCommand::ManageChannel(command) if response.is_success() => {
            if command.open {
                let opened = response
                    .data
                    .first()
                    .copied()
                    .or(command.channel_number)
                    .unwrap_or(1);
                ensure_channel_entry(state, opened);
            } else if let Some(channel_number) = command.channel_number {
                state
                    .open_channels
                    .retain(|entry| entry.channel_number != channel_number);
            }
        }
        IsoCommand::Verify(command) => match response.status_word() {
            status if status.is_success() => {
                push_unique_reference(&mut state.verified_references, command.reference);
            }
            status => {
                state
                    .verified_references
                    .retain(|value| *value != command.reference);
                if let Some(remaining) = status.retry_counter() {
                    upsert_retry_counter(&mut state.retry_counters, command.reference, remaining);
                }
            }
        },
        IsoCommand::ChangeReferenceData(command) | IsoCommand::ResetRetryCounter(command) => {
            if response.is_success() {
                push_unique_reference(&mut state.verified_references, command.reference);
                upsert_retry_counter(&mut state.retry_counters, command.reference, 3);
            }
        }
        IsoCommand::ExternalAuthenticate(command) if response.is_success() => {
            state.secure_messaging.active = command.p1 != 0 || command.p2 != 0;
            state.secure_messaging.protocol = Some(SecureMessagingProtocol::Iso7816);
            state.secure_messaging.security_level = Some(command.p1);
            state.secure_messaging.command_counter =
                state.secure_messaging.command_counter.saturating_add(1);
        }
        _ => {}
    }

    Ok(())
}

/// Return the logical channel encoded by the CLA byte.
pub const fn logical_channel_from_cla(cla: u8) -> u8 {
    if cla & 0x40 != 0 {
        4 + (cla & 0x0F)
    } else {
        cla & 0x03
    }
}

/// Apply one logical channel to the CLA byte while preserving the surrounding class flags.
pub fn set_logical_channel(cla: u8, channel: u8) -> Result<u8> {
    if channel <= 3 {
        Ok((cla & 0xBC) | channel)
    } else if channel <= 19 {
        Ok((cla & 0xB0) | 0x40 | (channel - 4))
    } else {
        Err(JcimError::InvalidApdu(format!(
            "logical channel {} exceeds ISO/IEC 7816 interindustry support",
            channel
        )))
    }
}

/// Ensure the tracked session contains one logical-channel entry for the given channel number.
fn ensure_channel_entry(state: &mut IsoSessionState, channel_number: u8) {
    if state
        .open_channels
        .iter()
        .all(|entry| entry.channel_number != channel_number)
    {
        state.open_channels.push(LogicalChannelState {
            channel_number,
            selected_aid: None,
            current_file: None,
        });
        state
            .open_channels
            .sort_by_key(|entry| entry.channel_number);
    }
}

/// Insert one verified-reference identifier once and keep the tracked set sorted.
fn push_unique_reference(references: &mut Vec<u8>, reference: u8) {
    if !references.contains(&reference) {
        references.push(reference);
        references.sort_unstable();
    }
}

/// Insert or update one retry-counter observation while preserving stable reference ordering.
fn upsert_retry_counter(counters: &mut Vec<RetryCounterState>, reference: u8, remaining: u8) {
    if let Some(counter) = counters
        .iter_mut()
        .find(|counter| counter.reference == reference)
    {
        counter.remaining = remaining;
    } else {
        counters.push(RetryCounterState {
            reference,
            remaining,
        });
        counters.sort_by_key(|counter| counter.reference);
    }
}
