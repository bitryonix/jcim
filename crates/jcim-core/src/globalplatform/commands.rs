use crate::aid::Aid;
use crate::apdu::CommandApdu;
use crate::iso7816::select_by_name;

use super::lifecycle::{CardLifeCycle, LockTransition};
use super::status::{GetStatusOccurrence, RegistryKind};

/// GlobalPlatform issuer security domain AID as defined by the public card specification.
pub const ISSUER_SECURITY_DOMAIN_AID: [u8; 8] = [0xA0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00];

const CLA_GLOBAL_PLATFORM: u8 = 0x80;
const INS_GET_STATUS: u8 = 0xF2;
const INS_SET_STATUS: u8 = 0xF0;

/// Build one ISO `SELECT` for the issuer security domain.
pub fn select_issuer_security_domain() -> CommandApdu {
    CommandApdu::new(
        0x00,
        0xA4,
        0x04,
        0x00,
        ISSUER_SECURITY_DOMAIN_AID.to_vec(),
        Some(256),
    )
}

/// Build one ISO `SELECT` for an explicit security domain AID.
pub fn select_security_domain(aid: &Aid) -> CommandApdu {
    select_by_name(aid)
}

/// Build one GlobalPlatform `GET STATUS` APDU.
pub fn get_status(kind: RegistryKind, occurrence: GetStatusOccurrence) -> CommandApdu {
    CommandApdu::new(
        CLA_GLOBAL_PLATFORM,
        INS_GET_STATUS,
        kind.p1(),
        occurrence.p2(),
        vec![0x4F, 0x00],
        Some(256),
    )
}

/// Build one `SET STATUS` request that changes the card life cycle state.
pub fn set_card_status(state: CardLifeCycle) -> CommandApdu {
    CommandApdu::new(
        CLA_GLOBAL_PLATFORM,
        INS_SET_STATUS,
        0x80,
        state.state_control(),
        Vec::new(),
        None,
    )
}

/// Build one `SET STATUS` request for an Application or Supplementary Security Domain.
pub fn set_application_status(aid: &Aid, transition: LockTransition) -> CommandApdu {
    CommandApdu::new(
        CLA_GLOBAL_PLATFORM,
        INS_SET_STATUS,
        0x40,
        transition.state_control(),
        aid.as_bytes().to_vec(),
        None,
    )
}

/// Build one `SET STATUS` request for a Security Domain and all of its associated Applications.
pub fn set_security_domain_status(aid: &Aid, transition: LockTransition) -> CommandApdu {
    CommandApdu::new(
        CLA_GLOBAL_PLATFORM,
        INS_SET_STATUS,
        0x60,
        transition.state_control(),
        aid.as_bytes().to_vec(),
        None,
    )
}

/// Build one `INITIALIZE UPDATE` request with an 8-byte host challenge.
pub fn initialize_update(host_challenge: [u8; 8]) -> CommandApdu {
    CommandApdu::new(
        CLA_GLOBAL_PLATFORM,
        0x50,
        0x00,
        0x00,
        host_challenge.to_vec(),
        Some(256),
    )
}
