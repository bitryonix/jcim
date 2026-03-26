//! Convenience re-exports for the JCIM core domain model.
//!
//! # Why this exists
//! Downstream crates often need the same handful of AID, APDU, ISO/GP helper, error, and model
//! types. This prelude keeps examples and small adapters readable without hiding where those
//! concepts live.

pub use crate::aid::Aid;
pub use crate::apdu::{CommandApdu, ResponseApdu};
pub use crate::error::{JcimError, Result};
pub use crate::globalplatform;
pub use crate::iso7816;
pub use crate::model::{
    BackendCapabilities, BackendHealth, BackendHealthStatus, BackendKind, CardProfile,
    CardProfileId, HardwareProfile, InstallDisposition, InstallRequest, InstallResult,
    JavaCardClassicVersion, MemoryLimits, MemoryStatus, PackageSummary, PowerAction,
    ProtocolHandshake, ProtocolVersion, RuntimeSnapshot, ScpMode, VirtualAppletMetadata,
};
