use serde::{Deserialize, Serialize};

use crate::apdu::{ApduEncoding, CommandApdu, CommandApduCase};

use super::super::session::logical_channel_from_cla;
use super::constants::*;

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
