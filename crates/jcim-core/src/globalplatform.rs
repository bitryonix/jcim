//! GlobalPlatform command builders and response parsers.

#![allow(clippy::missing_docs_in_private_items)]

use serde::{Deserialize, Serialize};

use crate::aid::Aid;
use crate::apdu::{CommandApdu, ResponseApdu};
use crate::error::{JcimError, Result};
use crate::iso7816::{INS_EXTERNAL_AUTHENTICATE, StatusWord, select_by_name};

/// GlobalPlatform issuer security domain AID as defined by the public card specification.
pub const ISSUER_SECURITY_DOMAIN_AID: [u8; 8] = [0xA0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00];

const CLA_GLOBAL_PLATFORM: u8 = 0x80;
const INS_GET_STATUS: u8 = 0xF2;
const INS_SET_STATUS: u8 = 0xF0;

/// Registry subset selected by `GET STATUS`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RegistryKind {
    /// Issuer Security Domain only.
    IssuerSecurityDomain,
    /// Applications, including Security Domains.
    Applications,
    /// Executable Load Files only.
    ExecutableLoadFiles,
    /// Executable Load Files and their Executable Modules.
    ExecutableLoadFilesAndModules,
}

impl RegistryKind {
    fn p1(self) -> u8 {
        match self {
            Self::IssuerSecurityDomain => 0x80,
            Self::Applications => 0x40,
            Self::ExecutableLoadFiles => 0x20,
            Self::ExecutableLoadFilesAndModules => 0x10,
        }
    }
}

/// Page selector used with `GET STATUS`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GetStatusOccurrence {
    /// Retrieve the first or all matching entries.
    FirstOrAll,
    /// Continue after a prior `FirstOrAll` call that returned `63 10`.
    Next,
}

impl GetStatusOccurrence {
    fn p2(self) -> u8 {
        match self {
            Self::FirstOrAll => 0x02,
            Self::Next => 0x03,
        }
    }
}

/// Card life cycle state coding used with `SET STATUS` for the issuer security domain.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CardLifeCycle {
    /// OP_READY (`01`).
    OpReady,
    /// INITIALIZED (`07`).
    Initialized,
    /// SECURED (`0F`).
    #[default]
    Secured,
    /// CARD_LOCKED (`7F`).
    CardLocked,
    /// TERMINATED (`FF`).
    Terminated,
}

impl CardLifeCycle {
    fn state_control(self) -> u8 {
        match self {
            Self::OpReady => 0x01,
            Self::Initialized => 0x07,
            Self::Secured => 0x0F,
            Self::CardLocked => 0x7F,
            Self::Terminated => 0xFF,
        }
    }
}

/// Lock transition used for Applications or Security Domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LockTransition {
    /// Transition to the locked state.
    Lock,
    /// Transition from the locked state back to the previous state.
    Unlock,
}

impl LockTransition {
    fn state_control(self) -> u8 {
        match self {
            Self::Lock => 0x80,
            Self::Unlock => 0x00,
        }
    }
}

/// Security level requested by `EXTERNAL AUTHENTICATE`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// No secure messaging.
    None,
    /// Command MAC only.
    CommandMac,
    /// Command MAC and response MAC.
    CommandAndResponseMac,
    /// Command MAC and command encryption.
    CommandMacAndEncryption,
    /// Command MAC, command encryption, and response MAC.
    CommandAndResponseMacWithEncryption,
    /// One explicit raw security level byte.
    Raw(u8),
}

impl SecurityLevel {
    /// Return the wire-encoded security-level byte.
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::None => 0x00,
            Self::CommandMac => 0x01,
            Self::CommandAndResponseMac => 0x11,
            Self::CommandMacAndEncryption => 0x03,
            Self::CommandAndResponseMacWithEncryption => 0x13,
            Self::Raw(value) => value,
        }
    }
}

/// Secure Channel Protocol mode supported by maintained JCIM GP flows.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScpMode {
    /// GlobalPlatform SCP02.
    Scp02,
    /// GlobalPlatform SCP03.
    Scp03,
}

/// Resolved GP keyset metadata used to open one secure channel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GpKeysetMetadata {
    /// Stable keyset name resolved by JCIM.
    pub name: String,
    /// Secure-channel mode requested for the keyset.
    pub mode: ScpMode,
}

/// Parsed result of one `INITIALIZE UPDATE` response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InitializeUpdateResponse {
    /// SCP mode used to interpret the response.
    pub mode: ScpMode,
    /// Raw response payload bytes before the status word.
    pub raw: Vec<u8>,
    /// Key diversification data when present.
    pub key_diversification_data: Vec<u8>,
    /// Key version number reported by the card.
    pub key_version_number: u8,
    /// SCP identifier byte reported by the card.
    pub scp_identifier: u8,
    /// SCP implementation byte when present.
    pub scp_implementation: Option<u8>,
    /// Sequence counter bytes when present.
    pub sequence_counter: Option<Vec<u8>>,
    /// Card challenge bytes.
    pub card_challenge: Vec<u8>,
    /// Card cryptogram bytes.
    pub card_cryptogram: Vec<u8>,
}

/// Derived secure-channel context retained by JCIM after `INITIALIZE UPDATE`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DerivedSessionContext {
    /// Resolved keyset metadata used for the channel.
    pub keyset: GpKeysetMetadata,
    /// Security level that JCIM will request for `EXTERNAL AUTHENTICATE`.
    pub security_level: SecurityLevel,
    /// Host challenge sent in the `INITIALIZE UPDATE` request.
    pub host_challenge: [u8; 8],
    /// Parsed initialize-update response.
    pub initialize_update: InitializeUpdateResponse,
}

/// Summary of one established GP secure channel retained by JCIM.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EstablishedSecureChannel {
    /// Resolved keyset metadata used for the channel.
    pub keyset: GpKeysetMetadata,
    /// Requested security level.
    pub security_level: SecurityLevel,
    /// Session identifier retained for logs, APIs, and state displays.
    pub session_id: String,
}

/// One parsed GlobalPlatform registry entry from `GET STATUS`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Registry subset that produced this entry.
    pub kind: RegistryKind,
    /// Entry AID.
    pub aid: Aid,
    /// Raw life cycle state byte.
    pub life_cycle_state: u8,
    /// Privilege bytes when present.
    pub privileges: Option<[u8; 3]>,
    /// Executable Load File AID when present.
    pub executable_load_file_aid: Option<Aid>,
    /// Associated Security Domain AID when present.
    pub associated_security_domain_aid: Option<Aid>,
    /// Executable Module AIDs when present.
    pub executable_module_aids: Vec<Aid>,
    /// Load File version bytes when present.
    pub load_file_version: Option<Vec<u8>>,
    /// Implicit selection parameters when present.
    pub implicit_selection_parameters: Vec<u8>,
}

/// Parsed result of one `GET STATUS` APDU.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GetStatusResponse {
    /// Requested registry subset.
    pub kind: RegistryKind,
    /// Parsed registry entries.
    pub entries: Vec<RegistryEntry>,
    /// Whether the card indicated `63 10` and expects a follow-up `Next` call.
    pub more_data_available: bool,
}

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

/// Parse the response to `GET STATUS`.
pub fn parse_get_status(kind: RegistryKind, response: &ResponseApdu) -> Result<GetStatusResponse> {
    let status = response.status_word();
    if status != StatusWord::SUCCESS && status != StatusWord::MORE_DATA_AVAILABLE {
        return Err(JcimError::Gp(format!(
            "GET STATUS returned status word {}",
            status
        )));
    }

    let entries = parse_registry_entries(kind, &response.data)?;
    Ok(GetStatusResponse {
        kind,
        entries,
        more_data_available: status == StatusWord::MORE_DATA_AVAILABLE,
    })
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

/// Build one `EXTERNAL AUTHENTICATE` request using a precomputed host cryptogram.
pub fn external_authenticate(
    security_level: SecurityLevel,
    host_cryptogram: [u8; 8],
) -> CommandApdu {
    CommandApdu::new(
        CLA_GLOBAL_PLATFORM,
        INS_EXTERNAL_AUTHENTICATE,
        security_level.as_byte(),
        0x00,
        host_cryptogram.to_vec(),
        Some(256),
    )
}

/// Parse the response to `INITIALIZE UPDATE`.
pub fn parse_initialize_update(
    mode: ScpMode,
    response: &ResponseApdu,
) -> Result<InitializeUpdateResponse> {
    if response.status_word() != StatusWord::SUCCESS {
        return Err(JcimError::Gp(format!(
            "INITIALIZE UPDATE returned status word {}",
            response.status_word()
        )));
    }

    match mode {
        ScpMode::Scp02 => parse_initialize_update_scp02(&response.data),
        ScpMode::Scp03 => parse_initialize_update_scp03(&response.data),
    }
}

/// Build one derived secure-channel context after `INITIALIZE UPDATE`.
pub fn derive_session_context(
    keyset: GpKeysetMetadata,
    security_level: SecurityLevel,
    host_challenge: [u8; 8],
    initialize_update: InitializeUpdateResponse,
) -> DerivedSessionContext {
    DerivedSessionContext {
        keyset,
        security_level,
        host_challenge,
        initialize_update,
    }
}

/// Build one established secure-channel summary for service and SDK consumers.
pub fn establish_secure_channel(
    session_context: &DerivedSessionContext,
    session_id: impl Into<String>,
) -> EstablishedSecureChannel {
    EstablishedSecureChannel {
        keyset: session_context.keyset.clone(),
        security_level: session_context.security_level,
        session_id: session_id.into(),
    }
}

fn parse_registry_entries(kind: RegistryKind, input: &[u8]) -> Result<Vec<RegistryEntry>> {
    let top_level = parse_tlvs(input)?;
    let mut entries = Vec::new();
    for tlv in top_level {
        if tlv.tag != 0xE3 {
            return Err(JcimError::Gp(format!(
                "GET STATUS returned unexpected top-level tag {:X}",
                tlv.tag
            )));
        }
        let nested = parse_tlvs(&tlv.value)?;
        let mut aid = None;
        let mut life_cycle_state = None;
        let mut privileges = None;
        let mut executable_load_file_aid = None;
        let mut associated_security_domain_aid = None;
        let mut executable_module_aids = Vec::new();
        let mut load_file_version = None;
        let mut implicit_selection_parameters = Vec::new();
        for child in nested {
            match child.tag {
                0x4F => aid = Some(Aid::from_slice(&child.value)?),
                0x9F70 => {
                    let Some(value) = child.value.first().copied() else {
                        return Err(JcimError::Gp(
                            "GET STATUS entry returned an empty life cycle state".to_string(),
                        ));
                    };
                    life_cycle_state = Some(value);
                }
                0xC5 => {
                    if child.value.len() == 3 {
                        privileges = Some([child.value[0], child.value[1], child.value[2]]);
                    }
                }
                0xC4 => executable_load_file_aid = Some(Aid::from_slice(&child.value)?),
                0xCC => associated_security_domain_aid = Some(Aid::from_slice(&child.value)?),
                0x84 => executable_module_aids.push(Aid::from_slice(&child.value)?),
                0xCE => load_file_version = Some(child.value),
                0xCF => {
                    let Some(value) = child.value.first().copied() else {
                        continue;
                    };
                    implicit_selection_parameters.push(value);
                }
                _ => {}
            }
        }
        entries.push(RegistryEntry {
            kind,
            aid: aid.ok_or_else(|| {
                JcimError::Gp("GET STATUS entry omitted the mandatory AID".to_string())
            })?,
            life_cycle_state: life_cycle_state.ok_or_else(|| {
                JcimError::Gp("GET STATUS entry omitted the mandatory life cycle state".to_string())
            })?,
            privileges,
            executable_load_file_aid,
            associated_security_domain_aid,
            executable_module_aids,
            load_file_version,
            implicit_selection_parameters,
        });
    }
    Ok(entries)
}

fn parse_initialize_update_scp02(input: &[u8]) -> Result<InitializeUpdateResponse> {
    if input.len() != 28 {
        return Err(JcimError::Gp(format!(
            "SCP02 INITIALIZE UPDATE payload must be 28 bytes, got {}",
            input.len()
        )));
    }

    Ok(InitializeUpdateResponse {
        mode: ScpMode::Scp02,
        raw: input.to_vec(),
        key_diversification_data: input[..10].to_vec(),
        key_version_number: input[10],
        scp_identifier: input[11],
        scp_implementation: None,
        sequence_counter: Some(input[12..14].to_vec()),
        card_challenge: input[14..20].to_vec(),
        card_cryptogram: input[20..28].to_vec(),
    })
}

fn parse_initialize_update_scp03(input: &[u8]) -> Result<InitializeUpdateResponse> {
    if input.len() != 29 {
        return Err(JcimError::Gp(format!(
            "SCP03 INITIALIZE UPDATE payload must be 29 bytes, got {}",
            input.len()
        )));
    }

    Ok(InitializeUpdateResponse {
        mode: ScpMode::Scp03,
        raw: input.to_vec(),
        key_diversification_data: input[..10].to_vec(),
        key_version_number: input[10],
        scp_identifier: input[11],
        scp_implementation: Some(input[12]),
        sequence_counter: None,
        card_challenge: input[13..21].to_vec(),
        card_cryptogram: input[21..29].to_vec(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BerTlv {
    tag: u32,
    value: Vec<u8>,
}

fn parse_tlvs(input: &[u8]) -> Result<Vec<BerTlv>> {
    let mut offset = 0;
    let mut tlvs = Vec::new();
    while offset < input.len() {
        let (tlv, consumed) = parse_tlv(&input[offset..])?;
        tlvs.push(tlv);
        offset += consumed;
    }
    Ok(tlvs)
}

fn parse_tlv(input: &[u8]) -> Result<(BerTlv, usize)> {
    if input.len() < 2 {
        return Err(JcimError::Gp(
            "BER-TLV input is too short to contain a tag and length".to_string(),
        ));
    }
    let mut offset = 0;
    let (tag, tag_length) = parse_tag(&input[offset..])?;
    offset += tag_length;
    let (length, length_length) = parse_length(&input[offset..])?;
    offset += length_length;
    if input.len() < offset + length {
        return Err(JcimError::Gp(
            "BER-TLV input ended before the declared value length".to_string(),
        ));
    }
    let value = input[offset..offset + length].to_vec();
    offset += length;
    Ok((BerTlv { tag, value }, offset))
}

fn parse_tag(input: &[u8]) -> Result<(u32, usize)> {
    let mut tag = u32::from(
        *input
            .first()
            .ok_or_else(|| JcimError::Gp("BER-TLV input is missing a tag byte".to_string()))?,
    );
    let mut consumed = 1;
    if tag & 0x1F == 0x1F {
        loop {
            let byte = *input
                .get(consumed)
                .ok_or_else(|| JcimError::Gp("BER-TLV tag was truncated".to_string()))?;
            tag = (tag << 8) | u32::from(byte);
            consumed += 1;
            if byte & 0x80 == 0 {
                break;
            }
        }
    }
    Ok((tag, consumed))
}

fn parse_length(input: &[u8]) -> Result<(usize, usize)> {
    let first = *input
        .first()
        .ok_or_else(|| JcimError::Gp("BER-TLV input is missing a length byte".to_string()))?;
    if first & 0x80 == 0 {
        return Ok((usize::from(first), 1));
    }

    let byte_count = usize::from(first & 0x7F);
    if byte_count == 0 || byte_count > 2 || input.len() < 1 + byte_count {
        return Err(JcimError::Gp(
            "BER-TLV long-form length is unsupported or truncated".to_string(),
        ));
    }

    let mut length = 0usize;
    for byte in &input[1..=byte_count] {
        length = (length << 8) | usize::from(*byte);
    }
    Ok((length, 1 + byte_count))
}

#[cfg(test)]
mod tests {
    use super::{
        CardLifeCycle, DerivedSessionContext, GetStatusOccurrence, GpKeysetMetadata,
        ISSUER_SECURITY_DOMAIN_AID, LockTransition, RegistryKind, ScpMode, SecurityLevel,
        StatusWord, derive_session_context, establish_secure_channel, external_authenticate,
        get_status, initialize_update, parse_get_status, parse_initialize_update,
        select_issuer_security_domain, set_application_status, set_card_status,
        set_security_domain_status,
    };
    use crate::aid::Aid;
    use crate::apdu::ResponseApdu;
    use crate::iso7816::INS_EXTERNAL_AUTHENTICATE;

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
