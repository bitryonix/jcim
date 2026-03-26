//! Install, runtime snapshot, and power-control model types.

use serde::{Deserialize, Serialize};

use crate::aid::Aid;
use crate::iso7816::{IsoCapabilities, IsoSessionState};

use super::{
    BackendCapabilities, BackendKind, CardProfileId, JavaCardClassicVersion, MemoryLimits,
};

/// Requested card power action.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum PowerAction {
    /// Power the card on and make ATR data available.
    On,
    /// Power the card off and clear card-selection state.
    Off,
}

impl From<bool> for PowerAction {
    fn from(value: bool) -> Self {
        if value { Self::On } else { Self::Off }
    }
}

/// Install-time selection policy used by typed CAP install requests.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum InstallDisposition {
    /// Make installed applets immediately selectable.
    MakeSelectable,
    /// Keep installed applets present but not selectable yet.
    KeepUnselectable,
}

impl From<bool> for InstallDisposition {
    fn from(value: bool) -> Self {
        if value {
            Self::MakeSelectable
        } else {
            Self::KeepUnselectable
        }
    }
}

/// Typed install request used by the service, client, and runtime surfaces.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct InstallRequest {
    /// Raw CAP archive bytes to install.
    pub cap_bytes: Vec<u8>,
    /// Selection policy for any applets created by the install.
    pub disposition: InstallDisposition,
}

impl InstallRequest {
    /// Build a typed install request from CAP bytes and an explicit disposition.
    pub fn new(cap_bytes: Vec<u8>, disposition: InstallDisposition) -> Self {
        Self {
            cap_bytes,
            disposition,
        }
    }

    /// Adapt a compact boolean-selection input into the typed request model.
    pub fn from_selectable_flag(cap_bytes: Vec<u8>, make_selectable: bool) -> Self {
        Self::new(cap_bytes, InstallDisposition::from(make_selectable))
    }

    /// Report whether the resulting install should make applets selectable immediately.
    pub fn make_selectable(&self) -> bool {
        self.disposition == InstallDisposition::MakeSelectable
    }
}

/// Summary of one installed package returned by inventories and install results.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct PackageSummary {
    /// Package AID.
    pub package_aid: Aid,
    /// Human-readable package name.
    pub package_name: String,
    /// Package version string reported by the backend.
    pub version: String,
    /// Number of applets associated with this package.
    pub applet_count: usize,
}

/// Runtime memory usage counters exposed to diagnostics.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct MemoryStatus {
    /// Persistent memory currently in use.
    pub persistent_used: usize,
    /// Peak CLEAR_ON_RESET transient memory used by recent traffic.
    pub transient_reset_used: usize,
    /// Peak CLEAR_ON_DESELECT transient memory used by recent traffic.
    pub transient_deselect_used: usize,
    /// Commit buffer bytes currently reserved.
    pub commit_buffer_used: usize,
    /// Peak install scratch usage observed during the last install flow.
    pub install_scratch_peak_bytes: usize,
    /// Number of memory pages touched by installed package data.
    pub pages_touched: usize,
    /// Number of erase blocks touched by installed package data.
    pub erase_blocks_touched: usize,
    /// Aggregate wear count derived from erase block usage.
    pub wear_count: u64,
}

/// Metadata for one virtual or backend-reported applet instance.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct VirtualAppletMetadata {
    /// Package AID that owns the applet.
    pub package_aid: Aid,
    /// Applet AID declared by the CAP package.
    pub applet_aid: Aid,
    /// Instance AID currently exposed for selection.
    pub instance_aid: Aid,
    /// Whether the instance is selectable.
    pub selectable: bool,
    /// Human-readable package name.
    pub package_name: String,
    /// Optional human-readable applet name.
    pub applet_name: Option<String>,
}

/// Result of a successful CAP install workflow.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct InstallResult {
    /// Package summary for the newly installed CAP.
    pub package: PackageSummary,
    /// Applets created or exposed by the install.
    pub applets: Vec<VirtualAppletMetadata>,
    /// Memory usage counters observed after the install completed.
    pub memory_status: MemoryStatus,
}

/// Point-in-time runtime state exposed to clients, services, and diagnostics.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    /// Backend kind serving the snapshot.
    pub backend_kind: BackendKind,
    /// Card profile currently active for the runtime.
    pub profile_id: CardProfileId,
    /// Java Card version implied by the selected profile.
    pub version: JavaCardClassicVersion,
    /// Backend capability summary for the selected runtime.
    pub backend_capabilities: BackendCapabilities,
    /// Current ATR.
    pub atr: Vec<u8>,
    /// Reader name reported by the runtime.
    pub reader_name: String,
    /// Explicit ISO/IEC 7816 capability summary reported by the backend.
    pub iso_capabilities: IsoCapabilities,
    /// Whether the card is currently powered on.
    pub power_on: bool,
    /// Currently selected AID, if any.
    pub selected_aid: Option<Aid>,
    /// Current tracked ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
    /// Static memory limits for the profile.
    pub memory_limits: MemoryLimits,
    /// Dynamic memory usage counters.
    pub memory_status: MemoryStatus,
}
