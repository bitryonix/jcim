//! Backend-facing shared model types.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{JcimError, Result};
use crate::iso7816::IsoCapabilities;

use super::{CardProfileId, ProtocolVersion};

/// Supported backend implementations known to JCIM.
///
/// # Why this exists
/// Backend selection appears in configuration, manifests, CLI flags, and runtime snapshots. A
/// dedicated enum keeps those surfaces synchronized.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// The maintained CAP-first Java Card simulator adapter.
    Simulator,
}

impl BackendKind {
    /// Return the canonical lowercase name used by config files, manifests, and CLI flags.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Simulator => "simulator",
        }
    }

    /// Return the default bundle directory name for this backend kind.
    pub fn default_bundle_subdir(self) -> &'static str {
        self.display_name()
    }
}

impl Display for BackendKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

impl FromStr for BackendKind {
    type Err = JcimError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "simulator" => Ok(Self::Simulator),
            _ => Err(JcimError::Unsupported(format!(
                "unsupported backend kind: {value}"
            ))),
        }
    }
}

/// GlobalPlatform secure-channel mode requested by a runtime or configuration profile.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ScpMode {
    /// Accept any secure-channel mode the backend and profile can negotiate.
    #[default]
    Any,
    /// Require Secure Channel Protocol 02 compatibility.
    Scp02,
    /// Require Secure Channel Protocol 03 compatibility.
    Scp03,
}

/// Backend capability summary returned during local-service and backend handshakes.
///
/// # Why this exists
/// A single handshake reply has to describe both protocol compatibility and what higher-level
/// operations a backend can actually perform.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct BackendCapabilities {
    /// Protocol version implemented by the backend.
    pub protocol_version: ProtocolVersion,
    /// Explicit ISO/IEC 7816 capability summary reported by the backend.
    pub iso_capabilities: IsoCapabilities,
    /// Whether the backend accepts raw CAP bytes for installation.
    pub accepts_cap: bool,
    /// Whether the backend accepts typed command APDU requests.
    pub supports_typed_apdu: bool,
    /// Whether the backend accepts raw APDU passthrough requests.
    pub supports_raw_apdu: bool,
    /// Whether the backend can execute APDU traffic.
    pub supports_apdu: bool,
    /// Whether the backend supports a reset operation.
    pub supports_reset: bool,
    /// Whether the backend supports power-on and power-off control.
    pub supports_power_control: bool,
    /// Whether the backend can return explicit ISO session state.
    pub supports_get_session_state: bool,
    /// Whether the backend supports first-class logical-channel management.
    pub supports_manage_channel: bool,
    /// Whether the backend supports secure-messaging session management.
    pub supports_secure_messaging: bool,
    /// Whether the backend can produce runtime snapshots.
    pub supports_snapshot: bool,
    /// Whether the backend supports installation workflows.
    pub supports_install: bool,
    /// Whether the backend supports package deletion workflows.
    pub supports_delete: bool,
    /// Whether the backend can return explicit health probes.
    pub supports_backend_health: bool,
    /// Whether the backend executes real applet bytecode rather than stub metadata flows.
    pub executes_real_methods: bool,
    /// Whether SCP02 wire behavior is compatible with real card tooling expectations.
    pub wire_compatible_scp02: bool,
    /// Whether SCP03 wire behavior is compatible with real card tooling expectations.
    pub wire_compatible_scp03: bool,
    /// Profiles the backend explicitly supports, or empty for "not declared".
    pub supported_profiles: Vec<CardProfileId>,
}

/// Coarse health status for a backend instance.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum BackendHealthStatus {
    /// The backend is ready for normal traffic.
    #[default]
    Ready,
    /// The backend is available but not at full fidelity.
    Degraded,
    /// The backend cannot currently serve requests safely.
    Unavailable,
}

/// Health payload returned by backend and local-service health probes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct BackendHealth {
    /// Backend implementation that produced this health report.
    pub backend_kind: BackendKind,
    /// Coarse health state for operator-facing diagnostics.
    pub status: BackendHealthStatus,
    /// Human-readable detail about the current health state.
    pub message: String,
    /// Protocol version spoken by the reporting backend.
    pub protocol_version: ProtocolVersion,
}

/// Handshake payload returned before APDU traffic begins.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ProtocolHandshake {
    /// Protocol version negotiated for the session.
    pub protocol_version: ProtocolVersion,
    /// Backend kind servicing the session.
    pub backend_kind: BackendKind,
    /// Reader name the backend will report to clients.
    pub reader_name: String,
    /// Capability summary for the selected backend instance.
    pub backend_capabilities: BackendCapabilities,
}
