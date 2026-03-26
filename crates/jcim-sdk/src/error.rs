//! Error types for the JCIM Rust SDK.

use thiserror::Error;

/// Standard result alias used by the SDK.
pub type Result<T> = std::result::Result<T, JcimSdkError>;

/// Error returned by the JCIM Rust SDK.
#[derive(Debug, Error)]
pub enum JcimSdkError {
    /// Transport setup or connection failed.
    #[error("transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
    /// The local service returned an RPC status error.
    #[error("service error: {0}")]
    Status(Box<tonic::Status>),
    /// One local process or filesystem operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// One shared JCIM parsing or validation boundary failed.
    #[error(transparent)]
    Core(#[from] jcim_core::error::JcimError),
    /// Hex decoding failed while converting APDUs.
    #[error("hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),
    /// The service returned a structurally incomplete payload.
    #[error("invalid service response: {0}")]
    InvalidResponse(String),
    /// The local SDK could not launch or locate `jcimd`.
    #[error("service bootstrap error: {0}")]
    Bootstrap(String),
}

impl From<tonic::Status> for JcimSdkError {
    fn from(value: tonic::Status) -> Self {
        Self::Status(Box::new(value))
    }
}
