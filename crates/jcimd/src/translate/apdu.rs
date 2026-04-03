use tonic::Status;

use jcim_api::v0_3::ResponseApduFrame;
use jcim_core::apdu::{ApduEncoding, CommandApdu, CommandApduCase, ResponseApdu};
use jcim_core::iso7816::{self, SecureMessagingProtocol};

use super::iso::status_word_info;
use super::status::to_status;

/// Decode one structured or raw protobuf APDU frame into the core typed APDU model.
// `tonic::Status` is the maintained transport-edge error type for these conversion helpers.
#[allow(clippy::result_large_err)]
pub(crate) fn command_apdu_from_proto(
    frame: Option<jcim_api::v0_3::CommandApduFrame>,
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
