use serde::{Deserialize, Serialize};

use crate::apdu::CommandApdu;
use crate::error::Result;

use super::super::selection::{FileSelection, SelectCommand, decode_select};
use super::builders::{get_challenge, get_response};
use super::classification::CommandKind;
use super::constants::*;

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
