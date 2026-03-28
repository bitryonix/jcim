//! GlobalPlatform command builders and response parsers.

#![allow(clippy::missing_docs_in_private_items)]

mod commands;
mod lifecycle;
mod parsers;
mod secure_channel;
mod status;

pub use commands::*;
pub use lifecycle::*;
pub use parsers::*;
pub use secure_channel::*;
pub use status::*;

#[cfg(test)]
mod tests {
    use crate::aid::Aid;
    use crate::apdu::ResponseApdu;
    use crate::iso7816::{INS_EXTERNAL_AUTHENTICATE, StatusWord};

    use super::{
        CardLifeCycle, DerivedSessionContext, GetStatusOccurrence, GpKeysetMetadata,
        ISSUER_SECURITY_DOMAIN_AID, LockTransition, RegistryKind, ScpMode, SecurityLevel,
        derive_session_context, establish_secure_channel, external_authenticate, get_status,
        initialize_update, parse_get_status, parse_initialize_update,
        select_issuer_security_domain, set_application_status, set_card_status,
        set_security_domain_status,
    };

    #[test]
    fn builds_select_get_status_and_set_status_commands() {
        assert_eq!(
            select_issuer_security_domain().to_bytes(),
            vec![
                0x00, 0xA4, 0x04, 0x00, 0x08, 0xA0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00,
            ]
        );
        assert_eq!(
            get_status(RegistryKind::Applications, GetStatusOccurrence::FirstOrAll).to_bytes(),
            vec![0x80, 0xF2, 0x40, 0x02, 0x02, 0x4F, 0x00, 0x00]
        );
        assert_eq!(
            set_card_status(CardLifeCycle::CardLocked).to_bytes(),
            vec![0x80, 0xF0, 0x80, 0x7F]
        );
        let applet = Aid::from_hex("A000000151000001").expect("applet aid");
        assert_eq!(
            set_application_status(&applet, LockTransition::Lock).to_bytes(),
            vec![
                0x80, 0xF0, 0x40, 0x80, 0x08, 0xA0, 0x00, 0x00, 0x01, 0x51, 0x00, 0x00, 0x01,
            ]
        );
        assert_eq!(
            set_security_domain_status(&applet, LockTransition::Unlock).to_bytes(),
            vec![
                0x80, 0xF0, 0x60, 0x00, 0x08, 0xA0, 0x00, 0x00, 0x01, 0x51, 0x00, 0x00, 0x01,
            ]
        );
        assert_eq!(
            initialize_update([0x01; 8]).to_bytes(),
            vec![
                0x80, 0x50, 0x00, 0x00, 0x08, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00,
            ]
        );
        assert_eq!(
            external_authenticate(super::SecurityLevel::CommandMac, [0xAA; 8]).to_bytes(),
            vec![
                0x80,
                INS_EXTERNAL_AUTHENTICATE,
                0x01,
                0x00,
                0x08,
                0xAA,
                0xAA,
                0xAA,
                0xAA,
                0xAA,
                0xAA,
                0xAA,
                0xAA,
                0x00,
            ]
        );
        assert_eq!(
            ISSUER_SECURITY_DOMAIN_AID,
            [0xA0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn parses_get_status_response_with_tlvs() {
        let response = ResponseApdu {
            data: vec![
                0xE3, 0x13, 0x4F, 0x08, 0xA0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x9F, 0x70,
                0x01, 0x0F, 0xC5, 0x03, 0x9E, 0x00, 0x00,
            ],
            sw: StatusWord::SUCCESS.as_u16(),
        };
        let parsed =
            parse_get_status(RegistryKind::Applications, &response).expect("parse get status");
        assert!(!parsed.more_data_available);
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].aid.to_hex(), "A000000003000000");
        assert_eq!(parsed.entries[0].life_cycle_state, 0x0F);
        assert_eq!(parsed.entries[0].privileges, Some([0x9E, 0x00, 0x00]));
    }

    #[test]
    fn parses_more_data_status() {
        let response = ResponseApdu {
            data: vec![0xE3, 0x07, 0x4F, 0x01, 0x01, 0x9F, 0x70, 0x01, 0x07],
            sw: StatusWord::MORE_DATA_AVAILABLE.as_u16(),
        };
        let parsed =
            parse_get_status(RegistryKind::IssuerSecurityDomain, &response).expect("parse");
        assert!(parsed.more_data_available);
    }

    #[test]
    fn parses_initialize_update_responses_for_scp02_and_scp03() {
        let scp02 = ResponseApdu::success(vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x11, 0x02, 0xAA, 0x55,
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
        ]);
        let parsed_scp02 = parse_initialize_update(ScpMode::Scp02, &scp02).expect("scp02");
        assert_eq!(parsed_scp02.key_version_number, 0x11);
        assert_eq!(parsed_scp02.scp_identifier, 0x02);
        assert_eq!(parsed_scp02.sequence_counter, Some(vec![0xAA, 0x55]));
        assert_eq!(
            parsed_scp02.card_challenge,
            vec![0x10, 0x11, 0x12, 0x13, 0x14, 0x15]
        );

        let scp03 = ResponseApdu::success(vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x12, 0x03, 0x70, 0x30,
            0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46,
            0x47,
        ]);
        let parsed_scp03 = parse_initialize_update(ScpMode::Scp03, &scp03).expect("scp03");
        assert_eq!(parsed_scp03.key_version_number, 0x12);
        assert_eq!(parsed_scp03.scp_identifier, 0x03);
        assert_eq!(parsed_scp03.scp_implementation, Some(0x70));
        assert_eq!(
            parsed_scp03.card_challenge,
            vec![0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37]
        );
    }

    #[test]
    fn derives_and_establishes_secure_channel_summaries() {
        let initialize_update = parse_initialize_update(
            ScpMode::Scp02,
            &ResponseApdu::success(vec![
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x11, 0x02, 0xAA, 0x55,
                0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
            ]),
        )
        .expect("init update");
        let keyset = GpKeysetMetadata {
            name: "admin".to_string(),
            mode: ScpMode::Scp02,
        };
        let derived: DerivedSessionContext = derive_session_context(
            keyset.clone(),
            SecurityLevel::CommandMac,
            [0xAB; 8],
            initialize_update,
        );
        assert_eq!(derived.keyset, keyset);
        assert_eq!(derived.security_level, SecurityLevel::CommandMac);

        let established = establish_secure_channel(&derived, "admin-scp02");
        assert_eq!(established.keyset.name, "admin");
        assert_eq!(established.session_id, "admin-scp02");
    }
}
