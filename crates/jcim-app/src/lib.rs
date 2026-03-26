//! Transport-neutral application core for JCIM 0.2.
//!
//! # Why this exists
//! JCIM 0.2 is centered on one local control plane instead of ad hoc CLI orchestration. This
//! crate owns the application-service boundary between transport shells such as the CLI or future
//! desktop UI and the lower-level build, simulator, and card adapters.
//!
//! # Role in the system
//! `jcim-app` resolves projects, manages the local project registry, supervises project-backed
//! managed simulations, coordinates build-on-demand flows, exposes physical-card operations, and
//! persists machine-local configuration.

#![forbid(unsafe_code)]

mod app;
mod card;
mod java_runtime;
mod model;
mod registry;

pub use app::JcimApp;
pub use card::{MockPhysicalCardAdapter, PhysicalCardAdapter};
pub use model::{
    AppletSummary, ArtifactSummary, CardAppletInventory, CardAppletSummary, CardDeleteSummary,
    CardInstallSummary, CardPackageInventory, CardPackageSummary, CardReaderSummary,
    CardStatusSummary, EventLine, GpSecureChannelSummary, OverviewSummary, ProjectDetails,
    ProjectSelectorInput, ProjectSummary, ServiceStatusSummary, SetupSummary, SimulationEngineMode,
    SimulationSelectorInput, SimulationSourceKind, SimulationStatusKind, SimulationSummary,
};
