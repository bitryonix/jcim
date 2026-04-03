/// APDU translation helpers between protobuf transport shapes and core models.
mod apdu;
/// Card and GlobalPlatform summary translation helpers for service responses.
mod card;
/// ISO 7816 translation helpers for service responses.
mod iso;
/// Project, artifact, and simulation summary translation helpers.
mod project;
/// Selector conversion helpers for incoming project and simulation requests.
mod selectors;
/// Error-to-status translation helpers for gRPC boundaries.
mod status;

pub(crate) use apdu::{
    command_apdu_from_proto, response_apdu_frame, secure_messaging_protocol_from_proto,
};
pub(crate) use card::{
    applet_inventory_response, delete_item_response, gp_secure_channel_info, install_cap_response,
    package_inventory_response,
};
pub(crate) use iso::{
    atr_info, iso_capabilities_info, iso_session_state_info, protocol_parameters_info,
};
pub(crate) use project::{artifact_info, project_details_response, project_info, simulation_info};
pub(crate) use selectors::{into_project_selector, into_simulation_selector};
pub(crate) use status::{service_status_response, to_status};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tonic::Code;

    use jcim_api::v0_3::{
        ApduEncoding, CommandApduCase, CommandApduFrame, CommandDomain, CommandKind,
        SecureMessagingProtocol as SecureMessagingProtocolProto, StatusWordClass,
    };
    use jcim_core::error::JcimError;
    use jcim_core::iso7816::{self, StatusWord};

    use super::iso::status_word_info;
    use super::{command_apdu_from_proto, secure_messaging_protocol_from_proto, to_status};

    #[test]
    fn to_status_maps_public_error_classes_to_transport_codes() {
        let invalid = to_status(JcimError::InvalidApdu("bad apdu".to_string()));
        assert_eq!(invalid.code(), Code::InvalidArgument);
        assert_eq!(invalid.message(), "bad apdu");

        let unavailable = to_status(JcimError::BackendStartup("bundle missing".to_string()));
        assert_eq!(unavailable.code(), Code::Unavailable);
        assert_eq!(unavailable.message(), "bundle missing");

        let internal = to_status(JcimError::MissingStatePath(PathBuf::from(
            "/tmp/runtime.toml",
        )));
        assert_eq!(internal.code(), Code::Internal);
        assert!(internal.message().contains("/tmp/runtime.toml"));
    }

    #[test]
    fn status_word_info_preserves_retry_and_length_hints() {
        let remaining = status_word_info(StatusWord::new(0x6110));
        assert_eq!(remaining.class, StatusWordClass::NormalProcessing as i32);
        assert_eq!(remaining.label, "response_bytes_available");
        assert_eq!(remaining.remaining_response_bytes, Some(16));
        assert!(remaining.success);

        let retry = status_word_info(StatusWord::new(0x63C2));
        assert_eq!(retry.class, StatusWordClass::Warning as i32);
        assert_eq!(retry.label, "verify_failed_retries_remaining");
        assert_eq!(retry.retry_counter, Some(2));
        assert!(retry.warning);

        let exact_length = status_word_info(StatusWord::new(0x6C20));
        assert_eq!(exact_length.class, StatusWordClass::CheckingError as i32);
        assert_eq!(exact_length.label, "correct_length_hint");
        assert_eq!(exact_length.exact_length_hint, Some(32));
    }

    #[test]
    fn command_apdu_from_proto_accepts_valid_structured_frames() {
        let mut frame = select_command_frame();
        frame.raw.clear();

        let command = command_apdu_from_proto(Some(frame)).expect("valid structured frame");
        assert_eq!(
            command.to_bytes(),
            [
                0x00, 0xA4, 0x04, 0x00, 0x09, 0x53, 0x61, 0x74, 0x6F, 0x43, 0x68, 0x69, 0x70, 0x00,
            ]
        );
    }

    #[test]
    fn command_apdu_from_proto_rejects_mismatched_metadata() {
        let mut frame = select_command_frame();
        frame.kind = CommandKind::GetResponse as i32;
        let kind_error =
            command_apdu_from_proto(Some(frame)).expect_err("kind mismatch should fail");
        assert_eq!(kind_error.code(), Code::InvalidArgument);
        assert!(kind_error.message().contains("command kind"));

        let mut frame = select_command_frame();
        frame.logical_channel = 1;
        let channel_error =
            command_apdu_from_proto(Some(frame)).expect_err("channel mismatch should fail");
        assert_eq!(channel_error.code(), Code::InvalidArgument);
        assert!(channel_error.message().contains("logical channel metadata"));
    }

    #[test]
    fn secure_messaging_protocol_from_proto_preserves_custom_labels() {
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Iso7816 as i32,
                "ignored",
            ),
            Some(jcim_core::iso7816::SecureMessagingProtocol::Iso7816)
        );
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Other as i32,
                "scp-custom",
            ),
            Some(jcim_core::iso7816::SecureMessagingProtocol::Other(
                "scp-custom".to_string()
            ))
        );
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Unspecified as i32,
                "",
            ),
            None
        );
    }

    fn select_command_frame() -> CommandApduFrame {
        let command = jcim_core::apdu::CommandApdu::parse(&[
            0x00, 0xA4, 0x04, 0x00, 0x09, 0x53, 0x61, 0x74, 0x6F, 0x43, 0x68, 0x69, 0x70, 0x00,
        ])
        .expect("parse select");
        let descriptor = iso7816::describe_command(&command);
        CommandApduFrame {
            raw: command.to_bytes(),
            cla: u32::from(command.cla),
            ins: u32::from(command.ins),
            p1: u32::from(command.p1),
            p2: u32::from(command.p2),
            data: command.data.clone(),
            ne: command.ne.map(|value| value as u32),
            encoding: ApduEncoding::Short as i32,
            apdu_case: CommandApduCase::CommandApduCase3Short as i32,
            domain: match descriptor.domain {
                jcim_core::iso7816::CommandDomain::Iso7816 => CommandDomain::Iso7816 as i32,
                jcim_core::iso7816::CommandDomain::GlobalPlatform => {
                    CommandDomain::GlobalPlatform as i32
                }
                jcim_core::iso7816::CommandDomain::Opaque => CommandDomain::Opaque as i32,
            },
            kind: match descriptor.kind {
                jcim_core::iso7816::CommandKind::Select => CommandKind::Select as i32,
                jcim_core::iso7816::CommandKind::ManageChannel => CommandKind::ManageChannel as i32,
                jcim_core::iso7816::CommandKind::GetResponse => CommandKind::GetResponse as i32,
                jcim_core::iso7816::CommandKind::ReadBinary => CommandKind::ReadBinary as i32,
                jcim_core::iso7816::CommandKind::WriteBinary => CommandKind::WriteBinary as i32,
                jcim_core::iso7816::CommandKind::UpdateBinary => CommandKind::UpdateBinary as i32,
                jcim_core::iso7816::CommandKind::EraseBinary => CommandKind::EraseBinary as i32,
                jcim_core::iso7816::CommandKind::ReadRecord => CommandKind::ReadRecord as i32,
                jcim_core::iso7816::CommandKind::UpdateRecord => CommandKind::UpdateRecord as i32,
                jcim_core::iso7816::CommandKind::AppendRecord => CommandKind::AppendRecord as i32,
                jcim_core::iso7816::CommandKind::SearchRecord => CommandKind::SearchRecord as i32,
                jcim_core::iso7816::CommandKind::GetData => CommandKind::GetData as i32,
                jcim_core::iso7816::CommandKind::PutData => CommandKind::PutData as i32,
                jcim_core::iso7816::CommandKind::Verify => CommandKind::Verify as i32,
                jcim_core::iso7816::CommandKind::ChangeReferenceData => {
                    CommandKind::ChangeReferenceData as i32
                }
                jcim_core::iso7816::CommandKind::ResetRetryCounter => {
                    CommandKind::ResetRetryCounter as i32
                }
                jcim_core::iso7816::CommandKind::InternalAuthenticate => {
                    CommandKind::InternalAuthenticate as i32
                }
                jcim_core::iso7816::CommandKind::ExternalAuthenticate => {
                    CommandKind::ExternalAuthenticate as i32
                }
                jcim_core::iso7816::CommandKind::GetChallenge => CommandKind::GetChallenge as i32,
                jcim_core::iso7816::CommandKind::Envelope => CommandKind::Envelope as i32,
                jcim_core::iso7816::CommandKind::GpGetStatus => CommandKind::GpGetStatus as i32,
                jcim_core::iso7816::CommandKind::GpSetStatus => CommandKind::GpSetStatus as i32,
                jcim_core::iso7816::CommandKind::GpInitializeUpdate => {
                    CommandKind::GpInitializeUpdate as i32
                }
                jcim_core::iso7816::CommandKind::GpExternalAuthenticate => {
                    CommandKind::GpExternalAuthenticate as i32
                }
                jcim_core::iso7816::CommandKind::Opaque => CommandKind::Opaque as i32,
            },
            logical_channel: u32::from(descriptor.logical_channel),
        }
    }
}
