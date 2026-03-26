//! Shared error type for JCIM crates.

use std::path::PathBuf;

use thiserror::Error;

/// Standard result alias used across JCIM crates.
pub type Result<T> = std::result::Result<T, JcimError>;

#[derive(Debug, Error)]
/// Error type returned by parsing, runtime, transport, and backend management code.
pub enum JcimError {
    /// An operating-system I/O boundary failed. Callers should usually retry only if the
    /// underlying resource is expected to become available again.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON encoding or decoding failed. Callers should treat this as malformed input or an
    /// internal protocol bug rather than retrying blindly.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// TOML decoding failed while loading configuration or manifests. Callers should correct the
    /// offending file before retrying.
    #[error("TOML decode error: {0}")]
    Toml(#[from] toml::de::Error),
    /// Hex decoding failed for an input that should have been hexadecimal text. Callers should
    /// surface the validation error to the operator.
    #[error("hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),
    /// A supplied or decoded AID was syntactically invalid.
    #[error("invalid AID: {0}")]
    InvalidAid(String),
    /// An APDU was too short or otherwise malformed for the expected parser.
    #[error("invalid APDU: {0}")]
    InvalidApdu(String),
    /// A CAP archive had invalid structure or invalid component contents.
    #[error("CAP format error: {0}")]
    CapFormat(String),
    /// A required CAP component was missing from the archive.
    #[error("missing CAP component: {0}")]
    MissingCapComponent(&'static str),
    /// The CAP file version was not supported by the selected profile or parser.
    #[error("unsupported CAP file version {0}")]
    UnsupportedCapVersion(String),
    /// The requested feature or mode is not supported by the current implementation.
    #[error("unsupported feature: {0}")]
    Unsupported(String),
    /// The two protocol participants could not negotiate a compatible version.
    #[error("protocol mismatch: expected {expected}, got {actual}")]
    ProtocolMismatch {
        /// Protocol version the local side expected.
        expected: String,
        /// Protocol version the remote side actually reported.
        actual: String,
    },
    /// The selected backend actor or process was unavailable before the request could complete.
    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),
    /// Backend startup failed before the backend became usable.
    #[error("backend startup failed: {0}")]
    BackendStartup(String),
    /// A backend process exited unexpectedly during use.
    #[error("backend exited unexpectedly: {0}")]
    BackendExited(String),
    /// An external backend replied with malformed control-plane data.
    #[error("malformed backend reply: {0}")]
    MalformedBackendReply(String),
    /// A GlobalPlatform workflow failed validation, authorization, or state checks.
    #[error("GlobalPlatform error: {0}")]
    Gp(String),
    /// A workflow required a state file path that does not exist or was not configured.
    #[error("state path does not exist: {0}")]
    MissingStatePath(PathBuf),
}
