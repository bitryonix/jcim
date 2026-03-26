//! External backend JSON-line framing and wire-model helpers.

#![allow(clippy::large_enum_variant, clippy::missing_docs_in_private_items)]
// These serde wire mirrors are private to the backend adapter and intentionally follow the JSON
// contract shape one-for-one. Field-by-field rustdoc would add noise without clarifying the
// maintained public surface, so this module keeps the documentation focus on the surrounding
// adapter behavior instead.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use jcim_core::aid::Aid;
use jcim_core::apdu::CommandApdu;
use jcim_core::error::{JcimError, Result};
use jcim_core::iso7816::{
    Atr, FileSelection, IsoCapabilities, IsoSessionState, LogicalChannelState, PowerState,
    ProtocolParameters, RetryCounterState, SecureMessagingProtocol, SecureMessagingState,
    StatusWord,
};
use jcim_core::model::{
    BackendCapabilities, BackendHealth, BackendKind, CardProfileId, InstallDisposition,
    InstallRequest, InstallResult, JavaCardClassicVersion, MemoryLimits, MemoryStatus,
    PackageSummary, PowerAction, ProtocolHandshake, ProtocolVersion, RuntimeSnapshot,
    VirtualAppletMetadata,
};
use serde::{Deserialize, Serialize};

/// Supported JSON-line operation names for the maintained external backend contract.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum BackendOperation {
    Handshake,
    Health,
    GetSessionState,
    TransmitTyped,
    TransmitRaw,
    Reset,
    Power,
    ManageChannel,
    OpenSecureMessaging,
    AdvanceSecureMessaging,
    CloseSecureMessaging,
    Install,
    DeletePackage,
    ListApplets,
    ListPackages,
    Snapshot,
    Shutdown,
}

impl BackendOperation {
    /// Stable snake-case label used in wire diagnostics.
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Handshake => "handshake",
            Self::Health => "health",
            Self::GetSessionState => "get_session_state",
            Self::TransmitTyped => "transmit_typed",
            Self::TransmitRaw => "transmit_raw",
            Self::Reset => "reset",
            Self::Power => "power",
            Self::ManageChannel => "manage_channel",
            Self::OpenSecureMessaging => "open_secure_messaging",
            Self::AdvanceSecureMessaging => "advance_secure_messaging",
            Self::CloseSecureMessaging => "close_secure_messaging",
            Self::Install => "install",
            Self::DeletePackage => "delete_package",
            Self::ListApplets => "list_applets",
            Self::ListPackages => "list_packages",
            Self::Snapshot => "snapshot",
            Self::Shutdown => "shutdown",
        }
    }
}

/// JSON-line request variants sent to the maintained external backend process.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(super) enum BackendRequest {
    Handshake {
        client_protocol: ProtocolVersion,
    },
    Health,
    GetSessionState,
    TransmitTyped {
        raw_hex: String,
        command: CommandApdu,
    },
    TransmitRaw {
        apdu_hex: String,
    },
    Reset,
    Power {
        action: PowerAction,
    },
    ManageChannel {
        open: bool,
        channel_number: Option<u8>,
    },
    OpenSecureMessaging {
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    },
    AdvanceSecureMessaging {
        increment_by: u32,
    },
    CloseSecureMessaging,
    Install {
        request: InstallRequestWire,
    },
    DeletePackage {
        aid: Aid,
    },
    ListApplets,
    ListPackages,
    Snapshot,
    Shutdown,
}

/// Install request payload adapted for the external JSON wire.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(super) struct InstallRequestWire {
    /// CAP archive bytes rendered as uppercase hexadecimal for Java-side parsing.
    pub cap_hex: String,
    /// Selection policy requested by the caller.
    pub disposition: InstallDisposition,
}

impl From<&InstallRequest> for InstallRequestWire {
    fn from(value: &InstallRequest) -> Self {
        Self {
            cap_hex: hex::encode_upper(&value.cap_bytes),
            disposition: value.disposition,
        }
    }
}

/// One selected-file reference on the backend JSON wire.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum FileSelectionWire {
    ByName { hex: String },
    FileId { file_id: u16 },
    Path { hex: String },
}

impl TryFrom<FileSelectionWire> for FileSelection {
    type Error = JcimError;

    fn try_from(value: FileSelectionWire) -> Result<Self> {
        Ok(match value {
            FileSelectionWire::ByName { hex } => FileSelection::ByName(hex::decode(&hex)?),
            FileSelectionWire::FileId { file_id } => FileSelection::FileId(file_id),
            FileSelectionWire::Path { hex } => FileSelection::Path(hex::decode(&hex)?),
        })
    }
}

/// One logical-channel entry on the backend JSON wire.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(super) struct LogicalChannelStateWire {
    pub channel_number: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_aid: Option<Aid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_file: Option<FileSelectionWire>,
}

impl TryFrom<LogicalChannelStateWire> for LogicalChannelState {
    type Error = JcimError;

    fn try_from(value: LogicalChannelStateWire) -> Result<Self> {
        Ok(Self {
            channel_number: value.channel_number,
            selected_aid: value.selected_aid,
            current_file: value.current_file.map(TryInto::try_into).transpose()?,
        })
    }
}

/// Backend-owned session-state payload carried on the external JSON wire.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
pub(super) struct BackendSessionStateWire {
    #[serde(default)]
    pub power_state: PowerState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atr_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_protocol: Option<ProtocolParameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_aid: Option<Aid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_file: Option<FileSelectionWire>,
    #[serde(default)]
    pub open_channels: Vec<LogicalChannelStateWire>,
    #[serde(default)]
    pub secure_messaging: SecureMessagingState,
    #[serde(default)]
    pub verified_references: Vec<u8>,
    #[serde(default)]
    pub retry_counters: Vec<RetryCounterState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_status: Option<u16>,
}

impl TryFrom<BackendSessionStateWire> for IsoSessionState {
    type Error = JcimError;

    fn try_from(value: BackendSessionStateWire) -> Result<Self> {
        let atr = parse_optional_atr_hex(value.atr_hex)?;
        let active_protocol = value
            .active_protocol
            .or_else(|| atr.as_ref().map(ProtocolParameters::from_atr));
        let mut open_channels = value
            .open_channels
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<LogicalChannelState>>>()?;
        if value.power_state == PowerState::On
            && open_channels.iter().all(|entry| entry.channel_number != 0)
        {
            open_channels.push(LogicalChannelState {
                channel_number: 0,
                selected_aid: value.selected_aid.clone(),
                current_file: value
                    .current_file
                    .clone()
                    .map(TryInto::try_into)
                    .transpose()?,
            });
            open_channels.sort_by_key(|entry| entry.channel_number);
        }

        Ok(IsoSessionState {
            power_state: value.power_state,
            atr,
            active_protocol,
            selected_aid: value.selected_aid,
            current_file: value.current_file.map(TryInto::try_into).transpose()?,
            open_channels,
            secure_messaging: value.secure_messaging,
            verified_references: value.verified_references,
            retry_counters: value.retry_counters,
            last_status: value.last_status.map(StatusWord::from),
        })
    }
}

/// Response payload for one APDU exchange.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(super) struct BackendApduExchangeWire {
    pub response_hex: String,
    pub session_state: BackendSessionStateWire,
}

/// Response payload for one reset operation.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(super) struct BackendResetResultWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atr_hex: Option<String>,
    pub session_state: BackendSessionStateWire,
}

/// Response payload for one power-control operation.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(super) struct BackendPowerResultWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atr_hex: Option<String>,
    pub session_state: BackendSessionStateWire,
}

/// Response payload for one secure-messaging state transition.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(super) struct BackendSecureMessagingSummaryWire {
    pub session_state: BackendSessionStateWire,
}

/// Snapshot payload carried by the external simulator backend.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(super) struct RuntimeSnapshotWire {
    pub backend_kind: BackendKind,
    pub profile_id: CardProfileId,
    pub version: JavaCardClassicVersion,
    pub backend_capabilities: BackendCapabilities,
    pub atr_hex: String,
    pub reader_name: String,
    pub iso_capabilities: IsoCapabilities,
    pub power_on: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_aid: Option<Aid>,
    pub session_state: BackendSessionStateWire,
    pub memory_limits: MemoryLimits,
    pub memory_status: MemoryStatus,
}

impl TryFrom<RuntimeSnapshotWire> for RuntimeSnapshot {
    type Error = JcimError;

    fn try_from(value: RuntimeSnapshotWire) -> Result<Self> {
        Ok(Self {
            backend_kind: value.backend_kind,
            profile_id: value.profile_id,
            version: value.version,
            backend_capabilities: value.backend_capabilities,
            atr: hex::decode(&value.atr_hex)?,
            reader_name: value.reader_name,
            iso_capabilities: value.iso_capabilities,
            power_on: value.power_on,
            selected_aid: value
                .selected_aid
                .clone()
                .or_else(|| value.session_state.selected_aid.clone()),
            session_state: value.session_state.try_into()?,
            memory_limits: value.memory_limits,
            memory_status: value.memory_status,
        })
    }
}

/// JSON-line response variants returned by the maintained external backend process.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(super) enum BackendReply {
    Handshake {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handshake: Option<ProtocolHandshake>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Health {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        health: Option<BackendHealth>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    GetSessionState {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_state: Option<BackendSessionStateWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    TransmitTyped {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exchange: Option<BackendApduExchangeWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    TransmitRaw {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exchange: Option<BackendApduExchangeWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Reset {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reset: Option<BackendResetResultWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Power {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        power: Option<BackendPowerResultWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    ManageChannel {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exchange: Option<BackendApduExchangeWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    OpenSecureMessaging {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secure_messaging: Option<BackendSecureMessagingSummaryWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    AdvanceSecureMessaging {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secure_messaging: Option<BackendSecureMessagingSummaryWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    CloseSecureMessaging {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secure_messaging: Option<BackendSecureMessagingSummaryWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Install {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        install: Option<InstallResult>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    DeletePackage {
        ok: bool,
        #[serde(default)]
        deleted: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    ListApplets {
        ok: bool,
        #[serde(default)]
        applets: Vec<VirtualAppletMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    ListPackages {
        ok: bool,
        #[serde(default)]
        packages: Vec<PackageSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Snapshot {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        snapshot: Option<RuntimeSnapshotWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Shutdown {
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

impl BackendReply {
    /// Operation reported by this reply.
    pub(super) const fn operation(&self) -> BackendOperation {
        match self {
            Self::Handshake { .. } => BackendOperation::Handshake,
            Self::Health { .. } => BackendOperation::Health,
            Self::GetSessionState { .. } => BackendOperation::GetSessionState,
            Self::TransmitTyped { .. } => BackendOperation::TransmitTyped,
            Self::TransmitRaw { .. } => BackendOperation::TransmitRaw,
            Self::Reset { .. } => BackendOperation::Reset,
            Self::Power { .. } => BackendOperation::Power,
            Self::ManageChannel { .. } => BackendOperation::ManageChannel,
            Self::OpenSecureMessaging { .. } => BackendOperation::OpenSecureMessaging,
            Self::AdvanceSecureMessaging { .. } => BackendOperation::AdvanceSecureMessaging,
            Self::CloseSecureMessaging { .. } => BackendOperation::CloseSecureMessaging,
            Self::Install { .. } => BackendOperation::Install,
            Self::DeletePackage { .. } => BackendOperation::DeletePackage,
            Self::ListApplets { .. } => BackendOperation::ListApplets,
            Self::ListPackages { .. } => BackendOperation::ListPackages,
            Self::Snapshot { .. } => BackendOperation::Snapshot,
            Self::Shutdown { .. } => BackendOperation::Shutdown,
        }
    }

    /// Whether the backend reported success for this operation.
    pub(super) const fn is_ok(&self) -> bool {
        match self {
            Self::Handshake { ok, .. }
            | Self::Health { ok, .. }
            | Self::GetSessionState { ok, .. }
            | Self::TransmitTyped { ok, .. }
            | Self::TransmitRaw { ok, .. }
            | Self::Reset { ok, .. }
            | Self::Power { ok, .. }
            | Self::ManageChannel { ok, .. }
            | Self::OpenSecureMessaging { ok, .. }
            | Self::AdvanceSecureMessaging { ok, .. }
            | Self::CloseSecureMessaging { ok, .. }
            | Self::Install { ok, .. }
            | Self::DeletePackage { ok, .. }
            | Self::ListApplets { ok, .. }
            | Self::ListPackages { ok, .. }
            | Self::Snapshot { ok, .. }
            | Self::Shutdown { ok, .. } => *ok,
        }
    }

    /// Optional backend-supplied error detail.
    pub(super) fn error_message(&self) -> Option<&str> {
        match self {
            Self::Handshake { error, .. }
            | Self::Health { error, .. }
            | Self::GetSessionState { error, .. }
            | Self::TransmitTyped { error, .. }
            | Self::TransmitRaw { error, .. }
            | Self::Reset { error, .. }
            | Self::Power { error, .. }
            | Self::ManageChannel { error, .. }
            | Self::OpenSecureMessaging { error, .. }
            | Self::AdvanceSecureMessaging { error, .. }
            | Self::CloseSecureMessaging { error, .. }
            | Self::Install { error, .. }
            | Self::DeletePackage { error, .. }
            | Self::ListApplets { error, .. }
            | Self::ListPackages { error, .. }
            | Self::Snapshot { error, .. }
            | Self::Shutdown { error, .. } => error.as_deref(),
        }
    }
}

/// Write one JSON-line request to an external backend process.
pub(super) fn write_request_line(stdin: &mut ChildStdin, request: &BackendRequest) -> Result<()> {
    serde_json::to_writer(&mut *stdin, request)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

/// Read one startup reply, but stop waiting if the backend never becomes responsive.
pub(super) fn read_reply_with_timeout(
    reader: BufReader<ChildStdout>,
    timeout: Duration,
) -> Result<(BufReader<ChildStdout>, BackendReply)> {
    let (tx, rx) = std_mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = reader;
        let result = read_startup_reply(&mut reader).map(|reply| (reader, reply));
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => Err(JcimError::BackendStartup(format!(
            "backend startup probe timed out after {} ms",
            timeout.as_millis()
        ))),
    }
}

/// Read exactly one startup reply line from an external backend process.
pub(super) fn read_startup_reply(reader: &mut BufReader<ChildStdout>) -> Result<BackendReply> {
    let mut line = String::new();
    let bytes = reader.read_line(&mut line)?;
    if bytes == 0 {
        return Err(JcimError::BackendStartup(
            "backend process closed the control stream during startup".to_string(),
        ));
    }
    parse_reply_line(&line)
}

/// Convert child-process exit state into a stable JCIM backend error.
pub(super) fn child_exit_error(child: &mut Child, fallback: &str) -> JcimError {
    match child.try_wait() {
        Ok(Some(status)) => {
            JcimError::BackendExited(format!("backend process exited with {status}"))
        }
        Ok(None) => JcimError::BackendUnavailable(fallback.to_string()),
        Err(error) => JcimError::BackendUnavailable(format!("{fallback}: {error}")),
    }
}

/// Parse one JSON-line reply from an external backend.
pub(super) fn parse_reply_line(line: &str) -> Result<BackendReply> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
        return Err(JcimError::MalformedBackendReply(
            "backend returned an empty reply line".to_string(),
        ));
    }
    serde_json::from_str(trimmed).map_err(|error| {
        JcimError::MalformedBackendReply(format!("backend returned invalid JSON: {error}"))
    })
}

/// Ensure one reply matches the expected operation.
pub(super) fn ensure_reply_operation(
    reply: &BackendReply,
    expected: BackendOperation,
) -> Result<()> {
    let actual = reply.operation();
    if actual == expected {
        Ok(())
    } else {
        Err(JcimError::MalformedBackendReply(format!(
            "backend returned {} where {} was expected",
            actual.as_str(),
            expected.as_str()
        )))
    }
}

/// Convert one backend-declared failure into the stable JCIM error model.
pub(super) fn ensure_reply_ok(reply: &BackendReply) -> Result<()> {
    if reply.is_ok() {
        Ok(())
    } else {
        Err(JcimError::Unsupported(
            reply
                .error_message()
                .unwrap_or("external backend reported an unspecified error")
                .to_string(),
        ))
    }
}

/// Validate protocol compatibility using JCIM's major-version rule.
pub(super) fn validate_protocol(expected: ProtocolVersion, actual: ProtocolVersion) -> Result<()> {
    if expected.is_compatible_with(actual) {
        Ok(())
    } else {
        Err(JcimError::ProtocolMismatch {
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn parse_optional_atr_hex(value: Option<String>) -> Result<Option<Atr>> {
    value
        .map(|hex_value| {
            let raw = hex::decode(&hex_value)?;
            Atr::parse(&raw)
        })
        .transpose()
}
