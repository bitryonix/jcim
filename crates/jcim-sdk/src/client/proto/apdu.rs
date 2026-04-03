use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::iso7816::{self, CommandDomain, CommandKind, SecureMessagingProtocol};

use crate::error::{JcimSdkError, Result};

/// Encode an optional secure-messaging protocol into protobuf enum and custom-label fields.
pub(in crate::client) fn secure_messaging_protocol_fields(
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
pub(in crate::client) fn command_apdu_frame(
    apdu: &CommandApdu,
) -> jcim_api::v0_3::CommandApduFrame {
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
pub(in crate::client) fn response_apdu_from_proto(
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
