//! ISO/IEC 7816 card, session, command, and status models.
#![allow(missing_docs)]
// This module intentionally centralizes a large standards-shaped public surface.
// We keep top-level docs high-signal here and avoid repeating line-by-line field docs on every
// typed command container introduced by the ISO-first redesign.

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::aid::Aid;
use crate::apdu::{ApduEncoding, CommandApdu, CommandApduCase, ResponseApdu};
use crate::error::{JcimError, Result};

/// Interindustry class byte for ordinary ISO/IEC 7816 commands on the basic channel.
pub const CLA_ISO7816: u8 = 0x00;

/// `SELECT` instruction.
pub const INS_SELECT: u8 = 0xA4;
/// `MANAGE CHANNEL` instruction.
pub const INS_MANAGE_CHANNEL: u8 = 0x70;
/// `GET RESPONSE` instruction.
pub const INS_GET_RESPONSE: u8 = 0xC0;
/// `READ BINARY` instruction.
pub const INS_READ_BINARY: u8 = 0xB0;
/// `WRITE BINARY` instruction.
pub const INS_WRITE_BINARY: u8 = 0xD0;
/// `UPDATE BINARY` instruction.
pub const INS_UPDATE_BINARY: u8 = 0xD6;
/// `ERASE BINARY` instruction.
pub const INS_ERASE_BINARY: u8 = 0x0E;
/// `READ RECORD` instruction.
pub const INS_READ_RECORD: u8 = 0xB2;
/// `UPDATE RECORD` instruction.
pub const INS_UPDATE_RECORD: u8 = 0xDC;
/// `APPEND RECORD` instruction.
pub const INS_APPEND_RECORD: u8 = 0xE2;
/// `SEARCH RECORD` instruction.
pub const INS_SEARCH_RECORD: u8 = 0xA2;
/// `GET DATA` instruction.
pub const INS_GET_DATA: u8 = 0xCA;
/// `PUT DATA` instruction.
pub const INS_PUT_DATA: u8 = 0xDA;
/// `VERIFY` instruction.
pub const INS_VERIFY: u8 = 0x20;
/// `CHANGE REFERENCE DATA` instruction.
pub const INS_CHANGE_REFERENCE_DATA: u8 = 0x24;
/// `RESET RETRY COUNTER` instruction.
pub const INS_RESET_RETRY_COUNTER: u8 = 0x2C;
/// `INTERNAL AUTHENTICATE` instruction.
pub const INS_INTERNAL_AUTHENTICATE: u8 = 0x88;
/// `EXTERNAL AUTHENTICATE` instruction.
pub const INS_EXTERNAL_AUTHENTICATE: u8 = 0x82;
/// `GET CHALLENGE` instruction.
pub const INS_GET_CHALLENGE: u8 = 0x84;
/// `ENVELOPE` instruction.
pub const INS_ENVELOPE: u8 = 0xC2;

/// Transmission convention declared by the ATR TS byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransmissionConvention {
    /// Direct convention.
    Direct,
    /// Inverse convention.
    Inverse,
}

impl Display for TransmissionConvention {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => f.write_str("direct"),
            Self::Inverse => f.write_str("inverse"),
        }
    }
}

/// Transport protocol advertised by an ATR or active session.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum TransportProtocol {
    /// T=0 byte-oriented half duplex.
    T0,
    /// T=1 block-oriented half duplex.
    T1,
    /// T=2 reserved historical protocol identifier.
    T2,
    /// T=3 reserved historical protocol identifier.
    T3,
    /// T=14 proprietary transport.
    T14,
    /// One explicit protocol number not covered by the named variants.
    Other(u8),
}

impl TransportProtocol {
    /// Parse one protocol code.
    pub const fn from_code(code: u8) -> Self {
        match code {
            0x00 => Self::T0,
            0x01 => Self::T1,
            0x02 => Self::T2,
            0x03 => Self::T3,
            0x0E => Self::T14,
            value => Self::Other(value),
        }
    }

    /// Return the wire protocol code.
    pub const fn code(self) -> u8 {
        match self {
            Self::T0 => 0x00,
            Self::T1 => 0x01,
            Self::T2 => 0x02,
            Self::T3 => 0x03,
            Self::T14 => 0x0E,
            Self::Other(value) => value,
        }
    }

    /// Parse one `T=...` string reported by a card stack.
    pub fn from_status_text(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        let number = trimmed.strip_prefix("T=")?;
        number.parse::<u8>().ok().map(Self::from_code)
    }
}

impl Display for TransportProtocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "T={}", self.code())
    }
}

/// Parsed interface-byte group from an ATR.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AtrInterfaceGroup {
    /// One-based group index.
    pub index: u8,
    /// TAi when present.
    pub ta: Option<u8>,
    /// TBi when present.
    pub tb: Option<u8>,
    /// TCi when present.
    pub tc: Option<u8>,
    /// TDi when present.
    pub td: Option<u8>,
    /// Protocol announced by TDi when present.
    pub protocol: Option<TransportProtocol>,
}

/// Parsed ATR plus retained raw bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Atr {
    /// Raw ATR bytes.
    pub raw: Vec<u8>,
    /// Direct or inverse convention.
    pub convention: TransmissionConvention,
    /// Parsed interface-byte groups.
    pub interface_groups: Vec<AtrInterfaceGroup>,
    /// Historical bytes.
    pub historical_bytes: Vec<u8>,
    /// TCK checksum byte when present.
    pub checksum_tck: Option<u8>,
    /// Protocols declared by the ATR.
    pub protocols: Vec<TransportProtocol>,
}

impl Atr {
    /// Parse one ATR.
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 2 {
            return Err(JcimError::InvalidApdu(
                "ATR must be at least 2 bytes".to_string(),
            ));
        }

        let convention = match raw[0] {
            0x3B => TransmissionConvention::Direct,
            0x3F => TransmissionConvention::Inverse,
            other => {
                return Err(JcimError::InvalidApdu(format!(
                    "unsupported ATR convention byte {:02X}",
                    other
                )));
            }
        };

        let t0 = raw[1];
        let mut y = t0 >> 4;
        let historical_len = usize::from(t0 & 0x0F);
        let mut index = 2usize;
        let mut group_number = 1u8;
        let mut groups = Vec::new();
        let mut protocols = Vec::new();

        loop {
            let mut group = AtrInterfaceGroup {
                index: group_number,
                ta: None,
                tb: None,
                tc: None,
                td: None,
                protocol: None,
            };
            if y & 0x01 != 0 {
                group.ta = Some(required_atr_byte(raw, &mut index, "TAi")?);
            }
            if y & 0x02 != 0 {
                group.tb = Some(required_atr_byte(raw, &mut index, "TBi")?);
            }
            if y & 0x04 != 0 {
                group.tc = Some(required_atr_byte(raw, &mut index, "TCi")?);
            }
            if y & 0x08 != 0 {
                let td = required_atr_byte(raw, &mut index, "TDi")?;
                let protocol = TransportProtocol::from_code(td & 0x0F);
                group.td = Some(td);
                group.protocol = Some(protocol);
                protocols.push(protocol);
                y = td >> 4;
                groups.push(group);
                group_number += 1;
                continue;
            }
            groups.push(group);
            break;
        }

        if protocols.is_empty() {
            protocols.push(TransportProtocol::T0);
        }

        let historical_end = index + historical_len;
        if historical_end > raw.len() {
            return Err(JcimError::InvalidApdu(
                "ATR historical bytes exceeded available input".to_string(),
            ));
        }
        let historical_bytes = raw[index..historical_end].to_vec();
        index = historical_end;

        let checksum_tck = if protocols
            .iter()
            .any(|protocol| *protocol != TransportProtocol::T0)
        {
            Some(required_atr_byte(raw, &mut index, "TCK")?)
        } else {
            None
        };

        if index != raw.len() {
            return Err(JcimError::InvalidApdu(format!(
                "ATR had {} trailing bytes after parsing",
                raw.len() - index
            )));
        }

        Ok(Self {
            raw: raw.to_vec(),
            convention,
            interface_groups: groups,
            historical_bytes,
            checksum_tck,
            protocols,
        })
    }

    /// Return the first protocol declared by the ATR.
    pub fn default_protocol(&self) -> Option<TransportProtocol> {
        self.protocols.first().copied()
    }

    /// Convert the ATR to uppercase hexadecimal.
    pub fn to_hex(&self) -> String {
        hex::encode_upper(&self.raw)
    }
}

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

/// High-level status-word class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StatusWordClass {
    /// Successful processing.
    NormalProcessing,
    /// Warning processing state.
    Warning,
    /// Execution error.
    ExecutionError,
    /// Checking error or malformed request.
    CheckingError,
    /// One unmapped status-word family.
    Unknown,
}

/// Parsed ISO/IEC 7816 status word.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct StatusWord(u16);

impl StatusWord {
    /// `90 00`.
    pub const SUCCESS: Self = Self(0x9000);
    /// `61 00` class: response bytes remain available.
    pub const RESPONSE_BYTES_AVAILABLE: Self = Self(0x6100);
    /// `63 10`: more data available for `GET STATUS` continuation flows.
    pub const MORE_DATA_AVAILABLE: Self = Self(0x6310);
    /// `62 83`.
    pub const WARNING_SELECTED_FILE_INVALIDATED: Self = Self(0x6283);
    /// `63 C0` class: verification failed with retries remaining.
    pub const VERIFY_FAIL_RETRY_COUNTER_BASE: Self = Self(0x63C0);
    /// `69 82`.
    pub const SECURITY_STATUS_NOT_SATISFIED: Self = Self(0x6982);
    /// `69 83`.
    pub const AUTH_METHOD_BLOCKED: Self = Self(0x6983);
    /// `69 85`.
    pub const CONDITIONS_NOT_SATISFIED: Self = Self(0x6985);
    /// `69 86`.
    pub const COMMAND_NOT_ALLOWED: Self = Self(0x6986);
    /// `6A 82`.
    pub const FILE_OR_APPLICATION_NOT_FOUND: Self = Self(0x6A82);
    /// `6A 86`.
    pub const INCORRECT_P1_P2: Self = Self(0x6A86);
    /// `6A 88`.
    pub const DATA_NOT_FOUND: Self = Self(0x6A88);
    /// `67 00`.
    pub const WRONG_LENGTH: Self = Self(0x6700);
    /// `6C 00` class: exact length hint available.
    pub const CORRECT_LENGTH_HINT: Self = Self(0x6C00);
    /// `6D 00`.
    pub const INSTRUCTION_NOT_SUPPORTED: Self = Self(0x6D00);
    /// `6E 00`.
    pub const CLASS_NOT_SUPPORTED: Self = Self(0x6E00);

    /// Build one status-word helper from a raw value.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Borrow the raw value.
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Return the coarse status-word class.
    pub const fn class(self) -> StatusWordClass {
        match (self.0 >> 8) as u8 {
            0x90 | 0x61 => StatusWordClass::NormalProcessing,
            0x62 | 0x63 => StatusWordClass::Warning,
            0x64..=0x66 => StatusWordClass::ExecutionError,
            0x67..=0x6F => StatusWordClass::CheckingError,
            _ => StatusWordClass::Unknown,
        }
    }

    /// Report whether the status word represents successful completion.
    pub const fn is_success(self) -> bool {
        matches!(self.class(), StatusWordClass::NormalProcessing) && (self.0 >> 8) as u8 != 0x6F
    }

    /// Report whether the status word is warning-class.
    pub const fn is_warning(self) -> bool {
        matches!((self.0 >> 8) as u8, 0x62 | 0x63)
    }

    /// Return remaining response bytes hinted by `61 XX`, when present.
    pub const fn remaining_response_bytes(self) -> Option<usize> {
        if (self.0 >> 8) as u8 == 0x61 {
            let low = (self.0 & 0x00FF) as usize;
            Some(if low == 0 { 256 } else { low })
        } else {
            None
        }
    }

    /// Return retries remaining when encoded as `63 CX`.
    pub const fn retry_counter(self) -> Option<u8> {
        if (self.0 & 0xFFF0) == 0x63C0 {
            Some((self.0 & 0x000F) as u8)
        } else {
            None
        }
    }

    /// Return a corrected length hint when encoded as `6C XX`.
    pub const fn exact_length_hint(self) -> Option<usize> {
        if (self.0 >> 8) as u8 == 0x6C {
            let low = (self.0 & 0x00FF) as usize;
            Some(if low == 0 { 256 } else { low })
        } else {
            None
        }
    }

    /// Return one stable label for common status words and status-word classes.
    pub fn label(self) -> &'static str {
        match self.0 {
            0x9000 => "success",
            0x6310 => "more_data_available",
            0x6283 => "selected_file_invalidated",
            0x6982 => "security_status_not_satisfied",
            0x6983 => "authentication_method_blocked",
            0x6985 => "conditions_not_satisfied",
            0x6986 => "command_not_allowed",
            0x6A82 => "file_or_application_not_found",
            0x6A86 => "incorrect_p1_p2",
            0x6A88 => "data_not_found",
            0x6700 => "wrong_length",
            0x6D00 => "instruction_not_supported",
            0x6E00 => "class_not_supported",
            _ if (self.0 >> 8) as u8 == 0x61 => "response_bytes_available",
            _ if (self.0 & 0xFFF0) == 0x63C0 => "verify_failed_retries_remaining",
            _ if (self.0 >> 8) as u8 == 0x6C => "correct_length_hint",
            _ => "unknown_status",
        }
    }
}

impl From<u16> for StatusWord {
    fn from(value: u16) -> Self {
        Self::new(value)
    }
}

impl From<StatusWord> for u16 {
    fn from(value: StatusWord) -> Self {
        value.as_u16()
    }
}

impl Display for StatusWord {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04X}", self.0)
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

/// Secure-messaging protocol family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecureMessagingProtocol {
    /// ISO interindustry secure messaging.
    Iso7816,
    /// GlobalPlatform SCP02.
    Scp02,
    /// GlobalPlatform SCP03.
    Scp03,
    /// One opaque protocol label.
    Other(String),
}

/// Current secure-messaging session summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct SecureMessagingState {
    /// Whether secure messaging is currently active.
    pub active: bool,
    /// Negotiated secure-messaging protocol.
    pub protocol: Option<SecureMessagingProtocol>,
    /// Raw security-level byte when known.
    pub security_level: Option<u8>,
    /// Session label or identifier when one exists.
    pub session_id: Option<String>,
    /// Monotonic command counter when tracked.
    pub command_counter: u32,
}

/// Retry counter state for one reference data object such as a PIN.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RetryCounterState {
    /// Reference identifier used in P2.
    pub reference: u8,
    /// Remaining retries when known.
    pub remaining: u8,
}

/// Selected file or application reference.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileSelection {
    /// Selection by DF name or application identifier.
    ByName(Vec<u8>),
    /// Selection by file identifier.
    FileId(u16),
    /// Selection by path bytes.
    Path(Vec<u8>),
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

/// Command domain used for operator-facing classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandDomain {
    /// ISO/IEC 7816 interindustry commands.
    Iso7816,
    /// GlobalPlatform card-management commands.
    GlobalPlatform,
    /// Opaque or unknown command family.
    Opaque,
}

/// High-level command kind used across JCIM APIs and logs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Select,
    ManageChannel,
    GetResponse,
    ReadBinary,
    WriteBinary,
    UpdateBinary,
    EraseBinary,
    ReadRecord,
    UpdateRecord,
    AppendRecord,
    SearchRecord,
    GetData,
    PutData,
    Verify,
    ChangeReferenceData,
    ResetRetryCounter,
    InternalAuthenticate,
    ExternalAuthenticate,
    GetChallenge,
    Envelope,
    GpGetStatus,
    GpSetStatus,
    GpInitializeUpdate,
    GpExternalAuthenticate,
    Opaque,
}

/// Generic command descriptor derived from one APDU.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandDescriptor {
    /// ISO/GP/opaque domain.
    pub domain: CommandDomain,
    /// High-level command kind.
    pub kind: CommandKind,
    /// Parsed APDU case.
    pub apdu_case: CommandApduCase,
    /// Encoding mode.
    pub encoding: ApduEncoding,
    /// Logical channel carried in the CLA byte.
    pub logical_channel: u8,
}

/// Structured `SELECT` command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SelectCommand {
    pub p1: u8,
    pub p2: u8,
    pub target: FileSelection,
    pub ne: Option<usize>,
}

/// Structured `MANAGE CHANNEL` command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManageChannelCommand {
    pub open: bool,
    pub channel_number: Option<u8>,
    pub ne: Option<usize>,
}

/// Structured `GET RESPONSE` command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GetResponseCommand {
    pub expected_length: usize,
}

/// Structured command operating on one binary offset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BinaryReadCommand {
    pub p1: u8,
    pub p2: u8,
    pub ne: Option<usize>,
}

/// Structured binary write/update command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BinaryWriteCommand {
    pub p1: u8,
    pub p2: u8,
    pub data: Vec<u8>,
}

/// Structured erase binary command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EraseBinaryCommand {
    pub p1: u8,
    pub p2: u8,
    pub data: Vec<u8>,
}

/// Structured record read command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecordReadCommand {
    pub record_number: u8,
    pub reference_control: u8,
    pub ne: Option<usize>,
}

/// Structured record update or append command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecordWriteCommand {
    pub record_number: u8,
    pub reference_control: u8,
    pub data: Vec<u8>,
}

/// Structured search-record command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchRecordCommand {
    pub record_number: u8,
    pub reference_control: u8,
    pub data: Vec<u8>,
    pub ne: Option<usize>,
}

/// Structured GET/PUT DATA command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DataCommand {
    pub p1: u8,
    pub p2: u8,
    pub data: Vec<u8>,
    pub ne: Option<usize>,
}

/// Structured security-reference-data command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReferenceDataCommand {
    pub p1: u8,
    pub reference: u8,
    pub data: Vec<u8>,
}

/// Structured authenticate command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthenticateCommand {
    pub p1: u8,
    pub p2: u8,
    pub data: Vec<u8>,
    pub ne: Option<usize>,
}

/// Structured `GET CHALLENGE` command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GetChallengeCommand {
    pub expected_length: usize,
}

/// Structured `ENVELOPE` command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeCommand {
    pub p1: u8,
    pub p2: u8,
    pub data: Vec<u8>,
    pub ne: Option<usize>,
}

/// One decoded ISO/IEC 7816 command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum IsoCommand {
    Select(SelectCommand),
    ManageChannel(ManageChannelCommand),
    GetResponse(GetResponseCommand),
    ReadBinary(BinaryReadCommand),
    WriteBinary(BinaryWriteCommand),
    UpdateBinary(BinaryWriteCommand),
    EraseBinary(EraseBinaryCommand),
    ReadRecord(RecordReadCommand),
    UpdateRecord(RecordWriteCommand),
    AppendRecord(RecordWriteCommand),
    SearchRecord(SearchRecordCommand),
    GetData(DataCommand),
    PutData(DataCommand),
    Verify(ReferenceDataCommand),
    ChangeReferenceData(ReferenceDataCommand),
    ResetRetryCounter(ReferenceDataCommand),
    InternalAuthenticate(AuthenticateCommand),
    ExternalAuthenticate(AuthenticateCommand),
    GetChallenge(GetChallengeCommand),
    Envelope(EnvelopeCommand),
    Opaque(CommandApdu),
}

impl IsoCommand {
    /// Return the high-level command kind.
    pub const fn kind(&self) -> CommandKind {
        match self {
            Self::Select(_) => CommandKind::Select,
            Self::ManageChannel(_) => CommandKind::ManageChannel,
            Self::GetResponse(_) => CommandKind::GetResponse,
            Self::ReadBinary(_) => CommandKind::ReadBinary,
            Self::WriteBinary(_) => CommandKind::WriteBinary,
            Self::UpdateBinary(_) => CommandKind::UpdateBinary,
            Self::EraseBinary(_) => CommandKind::EraseBinary,
            Self::ReadRecord(_) => CommandKind::ReadRecord,
            Self::UpdateRecord(_) => CommandKind::UpdateRecord,
            Self::AppendRecord(_) => CommandKind::AppendRecord,
            Self::SearchRecord(_) => CommandKind::SearchRecord,
            Self::GetData(_) => CommandKind::GetData,
            Self::PutData(_) => CommandKind::PutData,
            Self::Verify(_) => CommandKind::Verify,
            Self::ChangeReferenceData(_) => CommandKind::ChangeReferenceData,
            Self::ResetRetryCounter(_) => CommandKind::ResetRetryCounter,
            Self::InternalAuthenticate(_) => CommandKind::InternalAuthenticate,
            Self::ExternalAuthenticate(_) => CommandKind::ExternalAuthenticate,
            Self::GetChallenge(_) => CommandKind::GetChallenge,
            Self::Envelope(_) => CommandKind::Envelope,
            Self::Opaque(_) => CommandKind::Opaque,
        }
    }

    /// Encode the typed command back to one APDU.
    pub fn to_apdu(&self) -> CommandApdu {
        match self {
            Self::Select(command) => match &command.target {
                FileSelection::ByName(name) => CommandApdu::new(
                    CLA_ISO7816,
                    INS_SELECT,
                    command.p1,
                    command.p2,
                    name.clone(),
                    command.ne,
                ),
                FileSelection::FileId(file_id) => CommandApdu::new(
                    CLA_ISO7816,
                    INS_SELECT,
                    command.p1,
                    command.p2,
                    file_id.to_be_bytes().to_vec(),
                    command.ne,
                ),
                FileSelection::Path(path) => CommandApdu::new(
                    CLA_ISO7816,
                    INS_SELECT,
                    command.p1,
                    command.p2,
                    path.clone(),
                    command.ne,
                ),
            },
            Self::ManageChannel(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_MANAGE_CHANNEL,
                if command.open { 0x00 } else { 0x80 },
                command.channel_number.unwrap_or_default(),
                Vec::new(),
                command.ne,
            ),
            Self::GetResponse(command) => get_response(command.expected_length),
            Self::ReadBinary(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_READ_BINARY,
                command.p1,
                command.p2,
                Vec::new(),
                command.ne,
            ),
            Self::WriteBinary(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_WRITE_BINARY,
                command.p1,
                command.p2,
                command.data.clone(),
                None,
            ),
            Self::UpdateBinary(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_UPDATE_BINARY,
                command.p1,
                command.p2,
                command.data.clone(),
                None,
            ),
            Self::EraseBinary(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_ERASE_BINARY,
                command.p1,
                command.p2,
                command.data.clone(),
                None,
            ),
            Self::ReadRecord(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_READ_RECORD,
                command.record_number,
                command.reference_control,
                Vec::new(),
                command.ne,
            ),
            Self::UpdateRecord(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_UPDATE_RECORD,
                command.record_number,
                command.reference_control,
                command.data.clone(),
                None,
            ),
            Self::AppendRecord(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_APPEND_RECORD,
                command.record_number,
                command.reference_control,
                command.data.clone(),
                None,
            ),
            Self::SearchRecord(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_SEARCH_RECORD,
                command.record_number,
                command.reference_control,
                command.data.clone(),
                command.ne,
            ),
            Self::GetData(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_GET_DATA,
                command.p1,
                command.p2,
                Vec::new(),
                command.ne,
            ),
            Self::PutData(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_PUT_DATA,
                command.p1,
                command.p2,
                command.data.clone(),
                None,
            ),
            Self::Verify(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_VERIFY,
                command.p1,
                command.reference,
                command.data.clone(),
                None,
            ),
            Self::ChangeReferenceData(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_CHANGE_REFERENCE_DATA,
                command.p1,
                command.reference,
                command.data.clone(),
                None,
            ),
            Self::ResetRetryCounter(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_RESET_RETRY_COUNTER,
                command.p1,
                command.reference,
                command.data.clone(),
                None,
            ),
            Self::InternalAuthenticate(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_INTERNAL_AUTHENTICATE,
                command.p1,
                command.p2,
                command.data.clone(),
                command.ne,
            ),
            Self::ExternalAuthenticate(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_EXTERNAL_AUTHENTICATE,
                command.p1,
                command.p2,
                command.data.clone(),
                command.ne,
            ),
            Self::GetChallenge(command) => get_challenge(command.expected_length),
            Self::Envelope(command) => CommandApdu::new(
                CLA_ISO7816,
                INS_ENVELOPE,
                command.p1,
                command.p2,
                command.data.clone(),
                command.ne,
            ),
            Self::Opaque(apdu) => apdu.clone(),
        }
    }
}

/// Return one command descriptor for a raw APDU.
pub fn describe_command(apdu: &CommandApdu) -> CommandDescriptor {
    let (domain, kind) = match (apdu.cla, apdu.ins) {
        (_, INS_SELECT) => (CommandDomain::Iso7816, CommandKind::Select),
        (_, INS_MANAGE_CHANNEL) => (CommandDomain::Iso7816, CommandKind::ManageChannel),
        (_, INS_GET_RESPONSE) => (CommandDomain::Iso7816, CommandKind::GetResponse),
        (_, INS_READ_BINARY) => (CommandDomain::Iso7816, CommandKind::ReadBinary),
        (_, INS_WRITE_BINARY) => (CommandDomain::Iso7816, CommandKind::WriteBinary),
        (_, INS_UPDATE_BINARY) => (CommandDomain::Iso7816, CommandKind::UpdateBinary),
        (_, INS_ERASE_BINARY) => (CommandDomain::Iso7816, CommandKind::EraseBinary),
        (_, INS_READ_RECORD) => (CommandDomain::Iso7816, CommandKind::ReadRecord),
        (_, INS_UPDATE_RECORD) => (CommandDomain::Iso7816, CommandKind::UpdateRecord),
        (_, INS_APPEND_RECORD) => (CommandDomain::Iso7816, CommandKind::AppendRecord),
        (_, INS_SEARCH_RECORD) => (CommandDomain::Iso7816, CommandKind::SearchRecord),
        (_, INS_GET_DATA) => (CommandDomain::Iso7816, CommandKind::GetData),
        (_, INS_PUT_DATA) => (CommandDomain::Iso7816, CommandKind::PutData),
        (_, INS_VERIFY) => (CommandDomain::Iso7816, CommandKind::Verify),
        (_, INS_CHANGE_REFERENCE_DATA) => {
            (CommandDomain::Iso7816, CommandKind::ChangeReferenceData)
        }
        (_, INS_RESET_RETRY_COUNTER) => (CommandDomain::Iso7816, CommandKind::ResetRetryCounter),
        (_, INS_INTERNAL_AUTHENTICATE) => {
            (CommandDomain::Iso7816, CommandKind::InternalAuthenticate)
        }
        (0x80, 0xF2) => (CommandDomain::GlobalPlatform, CommandKind::GpGetStatus),
        (0x80, 0xF0) => (CommandDomain::GlobalPlatform, CommandKind::GpSetStatus),
        (0x80, 0x50) => (
            CommandDomain::GlobalPlatform,
            CommandKind::GpInitializeUpdate,
        ),
        (0x80, 0x82) => (
            CommandDomain::GlobalPlatform,
            CommandKind::GpExternalAuthenticate,
        ),
        (_, INS_EXTERNAL_AUTHENTICATE) => {
            (CommandDomain::Iso7816, CommandKind::ExternalAuthenticate)
        }
        (_, INS_GET_CHALLENGE) => (CommandDomain::Iso7816, CommandKind::GetChallenge),
        (_, INS_ENVELOPE) => (CommandDomain::Iso7816, CommandKind::Envelope),
        _ => (CommandDomain::Opaque, CommandKind::Opaque),
    };
    CommandDescriptor {
        domain,
        kind,
        apdu_case: apdu.apdu_case(),
        encoding: apdu.encoding,
        logical_channel: logical_channel_from_cla(apdu.cla),
    }
}

/// Decode one APDU into a typed ISO command when JCIM recognizes it.
pub fn decode_command(apdu: &CommandApdu) -> Result<IsoCommand> {
    Ok(match apdu.ins {
        INS_SELECT => IsoCommand::Select(decode_select(apdu)?),
        INS_MANAGE_CHANNEL => IsoCommand::ManageChannel(ManageChannelCommand {
            open: apdu.p1 == 0x00,
            channel_number: (apdu.p2 != 0).then_some(apdu.p2),
            ne: apdu.ne,
        }),
        INS_GET_RESPONSE => IsoCommand::GetResponse(GetResponseCommand {
            expected_length: apdu.ne.unwrap_or(256),
        }),
        INS_READ_BINARY => IsoCommand::ReadBinary(BinaryReadCommand {
            p1: apdu.p1,
            p2: apdu.p2,
            ne: apdu.ne,
        }),
        INS_WRITE_BINARY => IsoCommand::WriteBinary(BinaryWriteCommand {
            p1: apdu.p1,
            p2: apdu.p2,
            data: apdu.data.clone(),
        }),
        INS_UPDATE_BINARY => IsoCommand::UpdateBinary(BinaryWriteCommand {
            p1: apdu.p1,
            p2: apdu.p2,
            data: apdu.data.clone(),
        }),
        INS_ERASE_BINARY => IsoCommand::EraseBinary(EraseBinaryCommand {
            p1: apdu.p1,
            p2: apdu.p2,
            data: apdu.data.clone(),
        }),
        INS_READ_RECORD => IsoCommand::ReadRecord(RecordReadCommand {
            record_number: apdu.p1,
            reference_control: apdu.p2,
            ne: apdu.ne,
        }),
        INS_UPDATE_RECORD => IsoCommand::UpdateRecord(RecordWriteCommand {
            record_number: apdu.p1,
            reference_control: apdu.p2,
            data: apdu.data.clone(),
        }),
        INS_APPEND_RECORD => IsoCommand::AppendRecord(RecordWriteCommand {
            record_number: apdu.p1,
            reference_control: apdu.p2,
            data: apdu.data.clone(),
        }),
        INS_SEARCH_RECORD => IsoCommand::SearchRecord(SearchRecordCommand {
            record_number: apdu.p1,
            reference_control: apdu.p2,
            data: apdu.data.clone(),
            ne: apdu.ne,
        }),
        INS_GET_DATA => IsoCommand::GetData(DataCommand {
            p1: apdu.p1,
            p2: apdu.p2,
            data: Vec::new(),
            ne: apdu.ne,
        }),
        INS_PUT_DATA => IsoCommand::PutData(DataCommand {
            p1: apdu.p1,
            p2: apdu.p2,
            data: apdu.data.clone(),
            ne: None,
        }),
        INS_VERIFY => IsoCommand::Verify(ReferenceDataCommand {
            p1: apdu.p1,
            reference: apdu.p2,
            data: apdu.data.clone(),
        }),
        INS_CHANGE_REFERENCE_DATA => IsoCommand::ChangeReferenceData(ReferenceDataCommand {
            p1: apdu.p1,
            reference: apdu.p2,
            data: apdu.data.clone(),
        }),
        INS_RESET_RETRY_COUNTER => IsoCommand::ResetRetryCounter(ReferenceDataCommand {
            p1: apdu.p1,
            reference: apdu.p2,
            data: apdu.data.clone(),
        }),
        INS_INTERNAL_AUTHENTICATE => IsoCommand::InternalAuthenticate(AuthenticateCommand {
            p1: apdu.p1,
            p2: apdu.p2,
            data: apdu.data.clone(),
            ne: apdu.ne,
        }),
        INS_EXTERNAL_AUTHENTICATE => IsoCommand::ExternalAuthenticate(AuthenticateCommand {
            p1: apdu.p1,
            p2: apdu.p2,
            data: apdu.data.clone(),
            ne: apdu.ne,
        }),
        INS_GET_CHALLENGE => IsoCommand::GetChallenge(GetChallengeCommand {
            expected_length: apdu.ne.unwrap_or(256),
        }),
        INS_ENVELOPE => IsoCommand::Envelope(EnvelopeCommand {
            p1: apdu.p1,
            p2: apdu.p2,
            data: apdu.data.clone(),
            ne: apdu.ne,
        }),
        _ => IsoCommand::Opaque(apdu.clone()),
    })
}

/// Decode one `SELECT` APDU into the maintained typed selection model.
fn decode_select(apdu: &CommandApdu) -> Result<SelectCommand> {
    let target = match (apdu.p1, apdu.data.len()) {
        (0x04, _) => FileSelection::ByName(apdu.data.clone()),
        (0x08, _) => FileSelection::Path(apdu.data.clone()),
        (_, 2) => FileSelection::FileId(u16::from_be_bytes([apdu.data[0], apdu.data[1]])),
        _ => FileSelection::ByName(apdu.data.clone()),
    };
    Ok(SelectCommand {
        p1: apdu.p1,
        p2: apdu.p2,
        target,
        ne: apdu.ne,
    })
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

/// Build one `SELECT` by DF name or application identifier command.
pub fn select_by_name(aid: &Aid) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_SELECT,
        0x04,
        0x00,
        aid.as_bytes().to_vec(),
        Some(256),
    )
}

/// Build one `SELECT FILE` by file identifier.
pub fn select_file(file_id: u16) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_SELECT,
        0x00,
        0x00,
        file_id.to_be_bytes().to_vec(),
        Some(256),
    )
}

/// Build one `SELECT FILE` by path.
pub fn select_path(path: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_SELECT,
        0x08,
        0x00,
        path.to_vec(),
        Some(256),
    )
}

/// Build one `GET RESPONSE` command.
pub fn get_response(expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_GET_RESPONSE,
        0x00,
        0x00,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `MANAGE CHANNEL` open command.
pub fn manage_channel_open() -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_MANAGE_CHANNEL,
        0x00,
        0x00,
        Vec::new(),
        Some(1),
    )
}

/// Build one `MANAGE CHANNEL` close command.
pub fn manage_channel_close(channel_number: u8) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_MANAGE_CHANNEL,
        0x80,
        channel_number,
        Vec::new(),
        None,
    )
}

/// Build one `READ BINARY` command using one short file offset.
pub fn read_binary(offset: u16, expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_READ_BINARY,
        (offset >> 8) as u8,
        offset as u8,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `WRITE BINARY` command.
pub fn write_binary(offset: u16, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_WRITE_BINARY,
        (offset >> 8) as u8,
        offset as u8,
        data.to_vec(),
        None,
    )
}

/// Build one `UPDATE BINARY` command.
pub fn update_binary(offset: u16, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_UPDATE_BINARY,
        (offset >> 8) as u8,
        offset as u8,
        data.to_vec(),
        None,
    )
}

/// Build one `ERASE BINARY` command.
pub fn erase_binary(offset: u16, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_ERASE_BINARY,
        (offset >> 8) as u8,
        offset as u8,
        data.to_vec(),
        None,
    )
}

/// Build one `READ RECORD` command.
pub fn read_record(
    record_number: u8,
    reference_control: u8,
    expected_length: usize,
) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_READ_RECORD,
        record_number,
        reference_control,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `UPDATE RECORD` command.
pub fn update_record(record_number: u8, reference_control: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_UPDATE_RECORD,
        record_number,
        reference_control,
        data.to_vec(),
        None,
    )
}

/// Build one `APPEND RECORD` command.
pub fn append_record(record_number: u8, reference_control: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_APPEND_RECORD,
        record_number,
        reference_control,
        data.to_vec(),
        None,
    )
}

/// Build one `SEARCH RECORD` command.
pub fn search_record(
    record_number: u8,
    reference_control: u8,
    data: &[u8],
    expected_length: usize,
) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_SEARCH_RECORD,
        record_number,
        reference_control,
        data.to_vec(),
        Some(expected_length),
    )
}

/// Build one `GET DATA` command.
pub fn get_data(p1: u8, p2: u8, expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_GET_DATA,
        p1,
        p2,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `PUT DATA` command.
pub fn put_data(p1: u8, p2: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(CLA_ISO7816, INS_PUT_DATA, p1, p2, data.to_vec(), None)
}

/// Build one `VERIFY` command.
pub fn verify(reference: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_VERIFY,
        0x00,
        reference,
        data.to_vec(),
        None,
    )
}

/// Build one `CHANGE REFERENCE DATA` command.
pub fn change_reference_data(p1: u8, reference: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_CHANGE_REFERENCE_DATA,
        p1,
        reference,
        data.to_vec(),
        None,
    )
}

/// Build one `RESET RETRY COUNTER` command.
pub fn reset_retry_counter(p1: u8, reference: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_RESET_RETRY_COUNTER,
        p1,
        reference,
        data.to_vec(),
        None,
    )
}

/// Build one `INTERNAL AUTHENTICATE` command.
pub fn internal_authenticate(p1: u8, p2: u8, data: &[u8], expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_INTERNAL_AUTHENTICATE,
        p1,
        p2,
        data.to_vec(),
        Some(expected_length),
    )
}

/// Build one `EXTERNAL AUTHENTICATE` command.
pub fn external_authenticate(
    p1: u8,
    p2: u8,
    data: &[u8],
    expected_length: Option<usize>,
) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_EXTERNAL_AUTHENTICATE,
        p1,
        p2,
        data.to_vec(),
        expected_length,
    )
}

/// Build one `GET CHALLENGE` command.
pub fn get_challenge(expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_GET_CHALLENGE,
        0x00,
        0x00,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `ENVELOPE` command.
pub fn envelope(p1: u8, p2: u8, data: &[u8], expected_length: Option<usize>) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_ENVELOPE,
        p1,
        p2,
        data.to_vec(),
        expected_length,
    )
}

/// Read the next ATR byte or report which logical ATR field ran out of input.
fn required_atr_byte(raw: &[u8], index: &mut usize, label: &str) -> Result<u8> {
    let value = raw
        .get(*index)
        .copied()
        .ok_or_else(|| JcimError::InvalidApdu(format!("ATR ended before {}", label)))?;
    *index += 1;
    Ok(value)
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

#[cfg(test)]
mod tests {
    use crate::aid::Aid;

    use super::{
        Atr, CommandDomain, CommandKind, IsoCapabilities, IsoSessionState, ProtocolParameters,
        SecureMessagingProtocol, StatusWord, TransportProtocol, append_record,
        apply_response_to_session, describe_command, get_challenge, get_response,
        manage_channel_close, manage_channel_open, put_data, read_binary, read_record,
        reset_retry_counter, select_by_name, select_file, select_path, update_binary,
        update_record, verify,
    };
    use crate::apdu::ResponseApdu;

    #[test]
    fn parses_common_atr_shape() {
        let atr = Atr::parse(&hex::decode("3B800100").expect("hex")).expect("atr");
        assert_eq!(atr.convention.to_string(), "direct");
        assert_eq!(atr.protocols, vec![TransportProtocol::T1]);
        assert_eq!(
            ProtocolParameters::from_atr(&atr).protocol,
            Some(TransportProtocol::T1)
        );
    }

    #[test]
    fn command_builders_cover_interindustry_surface() {
        let aid = Aid::from_hex("A000000151000000").expect("aid");
        assert_eq!(select_by_name(&aid).to_bytes()[1], super::INS_SELECT);
        assert_eq!(select_file(0x3F00).to_bytes()[1], super::INS_SELECT);
        assert_eq!(
            select_path(&[0x3F, 0x00, 0x7F, 0x10]).to_bytes()[1],
            super::INS_SELECT
        );
        assert_eq!(
            get_response(256).to_bytes(),
            vec![0x00, 0xC0, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            manage_channel_open().to_bytes(),
            vec![0x00, 0x70, 0x00, 0x00, 0x01]
        );
        assert_eq!(
            manage_channel_close(2).to_bytes(),
            vec![0x00, 0x70, 0x80, 0x02]
        );
        assert_eq!(
            read_binary(0x0020, 8).to_bytes(),
            vec![0x00, 0xB0, 0x00, 0x20, 0x08]
        );
        assert_eq!(
            update_binary(0x0001, &[0xAA]).to_bytes(),
            vec![0x00, 0xD6, 0x00, 0x01, 0x01, 0xAA]
        );
        assert_eq!(
            read_record(1, 0x04, 8).to_bytes(),
            vec![0x00, 0xB2, 0x01, 0x04, 0x08]
        );
        assert_eq!(
            update_record(1, 0x04, &[0xBB]).to_bytes(),
            vec![0x00, 0xDC, 0x01, 0x04, 0x01, 0xBB]
        );
        assert_eq!(
            append_record(0, 0x04, &[0xCC]).to_bytes(),
            vec![0x00, 0xE2, 0x00, 0x04, 0x01, 0xCC]
        );
        assert_eq!(
            put_data(0x00, 0xFF, &[0xDD]).to_bytes(),
            vec![0x00, 0xDA, 0x00, 0xFF, 0x01, 0xDD]
        );
        assert_eq!(
            verify(0x80, &[0x12, 0x34]).to_bytes(),
            vec![0x00, 0x20, 0x00, 0x80, 0x02, 0x12, 0x34]
        );
        assert_eq!(
            reset_retry_counter(0x00, 0x80, &[0x01]).to_bytes(),
            vec![0x00, 0x2C, 0x00, 0x80, 0x01, 0x01]
        );
        assert_eq!(
            get_challenge(8).to_bytes(),
            vec![0x00, 0x84, 0x00, 0x00, 0x08]
        );
    }

    #[test]
    fn command_descriptor_classifies_commands() {
        let aid = Aid::from_hex("A000000151000000").expect("aid");
        let descriptor = describe_command(&select_by_name(&aid));
        assert_eq!(descriptor.domain, CommandDomain::Iso7816);
        assert_eq!(descriptor.kind, CommandKind::Select);
    }

    #[test]
    fn status_words_report_classes_and_hints() {
        assert!(StatusWord::SUCCESS.is_success());
        assert_eq!(
            StatusWord::new(0x6100).remaining_response_bytes(),
            Some(256)
        );
        assert_eq!(StatusWord::new(0x63C2).retry_counter(), Some(2));
        assert_eq!(StatusWord::new(0x6C10).exact_length_hint(), Some(16));
    }

    #[test]
    fn session_state_tracks_select_verify_and_secure_messaging() {
        let atr = Atr::parse(&hex::decode("3B800100").expect("hex")).expect("atr");
        let mut state =
            IsoSessionState::reset(Some(atr.clone()), Some(ProtocolParameters::from_atr(&atr)));
        let aid = Aid::from_hex("A000000151000001").expect("aid");
        let select = select_by_name(&aid);
        apply_response_to_session(&mut state, &select, &ResponseApdu::status(0x9000))
            .expect("apply");
        assert_eq!(state.selected_aid, Some(aid.clone()));

        let verify = verify(0x80, b"1234");
        apply_response_to_session(&mut state, &verify, &ResponseApdu::status(0x9000))
            .expect("apply");
        assert!(state.verified_references.contains(&0x80));

        let external = super::external_authenticate(0x01, 0x00, b"\xAA\xBB", None);
        apply_response_to_session(&mut state, &external, &ResponseApdu::status(0x9000))
            .expect("apply");
        assert!(state.secure_messaging.active);
        assert_eq!(
            state.secure_messaging.protocol,
            Some(SecureMessagingProtocol::Iso7816)
        );
    }

    #[test]
    fn default_capabilities_are_explicit() {
        let capabilities = IsoCapabilities::default();
        assert_eq!(capabilities.protocols, vec![TransportProtocol::T1]);
        assert!(capabilities.raw_apdu);
    }
}
