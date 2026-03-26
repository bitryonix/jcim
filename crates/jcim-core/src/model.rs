//! Shared value types and maintained public domain models for JCIM.
//!
//! # Why this exists
//! Most JCIM crates need to talk about the same backend kinds, card profiles, install requests,
//! and runtime snapshots. Keeping those concepts here gives the workspace one maintained language
//! for control-plane and runtime state.
//!
//! # Role in the system
//! Reach for this module when code needs typed protocol negotiation data, backend capability
//! reporting, or card-profile metadata that must stay stable across service, runtime, and CLI
//! code.
//!
//! # Examples
//! A CLI-style workflow can resolve a maintained profile and prepare a typed install request:
//!
//! ```rust
//! use jcim_core::model::{CardProfile, CardProfileId, InstallDisposition, InstallRequest};
//!
//! let profile = CardProfile::builtin(CardProfileId::Classic305);
//! let request = InstallRequest::new(vec![0xCA, 0xFE], InstallDisposition::KeepUnselectable);
//!
//! assert_eq!(profile.profile_id(), CardProfileId::Classic305);
//! assert!(!request.make_selectable());
//! ```

mod backend;
mod install;
mod profile;
mod protocol;

pub use backend::{
    BackendCapabilities, BackendHealth, BackendHealthStatus, BackendKind, ProtocolHandshake,
    ScpMode,
};
pub use install::{
    InstallDisposition, InstallRequest, InstallResult, MemoryStatus, PackageSummary, PowerAction,
    RuntimeSnapshot, VirtualAppletMetadata,
};
pub use profile::{
    CardProfile, CardProfileId, HardwareProfile, JavaCardClassicVersion, MemoryLimits,
};
pub use protocol::ProtocolVersion;

#[cfg(test)]
mod tests;
