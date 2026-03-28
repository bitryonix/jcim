use serde::{Deserialize, Serialize};

/// Secure-messaging protocol family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecureMessagingProtocol {
    /// ISO interindustry secure messaging.
    Iso7816,
    /// GlobalPlatform SCP02.
    Scp02,
    /// GlobalPlatform SCP03.
    Scp03,
    /// One opaque protocol label.
    Other(String),
}

/// Current secure-messaging session summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct SecureMessagingState {
    /// Whether secure messaging is currently active.
    pub active: bool,
    /// Negotiated secure-messaging protocol.
    pub protocol: Option<SecureMessagingProtocol>,
    /// Raw security-level byte when known.
    pub security_level: Option<u8>,
    /// Session label or identifier when one exists.
    pub session_id: Option<String>,
    /// Monotonic command counter when tracked.
    pub command_counter: u32,
}
