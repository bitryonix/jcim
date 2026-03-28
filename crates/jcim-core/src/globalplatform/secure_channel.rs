use serde::{Deserialize, Serialize};

use crate::apdu::{CommandApdu, ResponseApdu};
use crate::error::{JcimError, Result};
use crate::iso7816::{INS_EXTERNAL_AUTHENTICATE, StatusWord};

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

/// Build one `EXTERNAL AUTHENTICATE` request using a precomputed host cryptogram.
pub fn external_authenticate(
    security_level: SecurityLevel,
    host_cryptogram: [u8; 8],
) -> CommandApdu {
    CommandApdu::new(
        0x80,
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
