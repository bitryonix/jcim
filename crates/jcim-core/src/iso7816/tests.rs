#![allow(clippy::missing_docs_in_private_items)]

use crate::aid::Aid;
use crate::apdu::ResponseApdu;

use super::{
    Atr, CommandDomain, CommandKind, IsoCapabilities, IsoSessionState, ProtocolParameters,
    SecureMessagingProtocol, StatusWord, TransportProtocol, append_record,
    apply_response_to_session, describe_command, get_challenge, get_response, manage_channel_close,
    manage_channel_open, put_data, read_binary, read_record, reset_retry_counter, select_by_name,
    select_file, select_path, update_binary, update_record, verify,
};

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
    apply_response_to_session(&mut state, &select, &ResponseApdu::status(0x9000)).expect("apply");
    assert_eq!(state.selected_aid, Some(aid.clone()));

    let verify = verify(0x80, b"1234");
    apply_response_to_session(&mut state, &verify, &ResponseApdu::status(0x9000)).expect("apply");
    assert!(state.verified_references.contains(&0x80));

    let external = super::external_authenticate(0x01, 0x00, b"\xAA\xBB", None);
    apply_response_to_session(&mut state, &external, &ResponseApdu::status(0x9000)).expect("apply");
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
