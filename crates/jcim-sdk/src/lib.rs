//! Canonical Rust lifecycle API for JCIM 0.2.
//!
//! # Why this exists
//! JCIM is service-first, but Rust callers still need a supported API that covers project build,
//! simulator control, CAP installation, and physical-card APDU workflows without hand-writing
//! gRPC bootstrap and protobuf glue in every consumer.

#![forbid(unsafe_code)]

mod client;
mod connection;
mod error;
mod types;

pub use jcim_core::aid::Aid;
pub use jcim_core::apdu::{CommandApdu, ResponseApdu};
pub use jcim_core::{globalplatform, iso7816};

pub use client::JcimClient;
pub use connection::CardConnection;
pub use error::{JcimSdkError, Result};
pub use types::{
    ApduExchangeSummary, AppletSummary, ArtifactSummary, BuildSummary, CardAppletInventory,
    CardAppletSummary, CardConnectionKind, CardConnectionLocator, CardConnectionTarget,
    CardDeleteSummary, CardInstallSource, CardInstallSummary, CardPackageInventory,
    CardPackageSummary, CardReaderSummary, CardStatusSummary, EventLine, GpSecureChannelSummary,
    ManageChannelSummary, OverviewSummary, ProjectDetails, ProjectRef, ProjectSummary, ReaderRef,
    ResetSummary, SecureMessagingSummary, ServiceStatusSummary, SetupSummary, SimulationEngineMode,
    SimulationInput, SimulationRef, SimulationSourceKind, SimulationStatus, SimulationSummary,
};
