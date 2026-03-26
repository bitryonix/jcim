//! Public typed Rust models for the JCIM SDK.

#![allow(clippy::missing_docs_in_private_items)]

use std::path::{Path, PathBuf};

use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::globalplatform;
use jcim_core::iso7816::{Atr, IsoCapabilities, IsoSessionState, ProtocolParameters};
use serde::Serialize;

/// Selector for one JCIM project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectRef {
    /// Absolute or relative project path.
    pub project_path: Option<PathBuf>,
    /// Stable local project id.
    pub project_id: Option<String>,
}

impl ProjectRef {
    /// Select a project by path.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            project_path: Some(path.into()),
            project_id: None,
        }
    }

    /// Select a project by id.
    pub fn from_id(project_id: impl Into<String>) -> Self {
        Self {
            project_path: None,
            project_id: Some(project_id.into()),
        }
    }

    /// Resolve the current working directory as a JCIM project.
    pub fn current() -> std::io::Result<Self> {
        Ok(Self::from_path(std::env::current_dir()?))
    }
}

/// Selector for one running simulation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SimulationRef {
    /// Stable simulation id.
    pub simulation_id: String,
}

impl SimulationRef {
    /// Build a selector from one simulation id.
    pub fn new(simulation_id: impl Into<String>) -> Self {
        Self {
            simulation_id: simulation_id.into(),
        }
    }
}

/// Input source used to start one simulation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum SimulationInput {
    /// Build or reuse one JCIM project and start the simulator from its managed runtime artifacts.
    Project(ProjectRef),
}

/// Input source used to install a CAP onto a physical card.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum CardInstallSource {
    /// Resolve the CAP from a JCIM project.
    Project(ProjectRef),
    /// Install one explicit CAP path.
    Cap(PathBuf),
}

/// Physical reader selector for card operations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ReaderRef {
    /// Use the configured JCIM default reader.
    Default,
    /// Use one explicit reader name.
    Named(String),
}

impl ReaderRef {
    /// Build a named reader selector.
    pub fn named(reader_name: impl Into<String>) -> Self {
        Self::Named(reader_name.into())
    }

    pub(crate) fn as_deref(&self) -> Option<&str> {
        match self {
            Self::Default => None,
            Self::Named(reader_name) => Some(reader_name),
        }
    }
}

/// Target selector used to open one unified APDU connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum CardConnectionTarget {
    /// Open one connection against a physical reader.
    Reader(ReaderRef),
    /// Attach one connection to an already-running simulation.
    ExistingSimulation(SimulationRef),
    /// Start and own one simulation-backed connection.
    StartSimulation(SimulationInput),
}

/// High-level kind of target behind one unified APDU connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CardConnectionKind {
    /// The connection targets a physical reader.
    Reader,
    /// The connection targets a virtual simulation.
    Simulation,
}

/// Resolved target locator behind one unified APDU connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum CardConnectionLocator {
    /// One resolved physical reader name.
    Reader {
        /// Resolved reader name used for all operations.
        reader_name: String,
    },
    /// One simulation target, optionally owned by the connection.
    Simulation {
        /// Stable running simulation selector.
        simulation: SimulationRef,
        /// Whether this connection started the simulation and must stop it on close.
        owned: bool,
    },
}

/// High-level overview of the managed JCIM service state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OverviewSummary {
    /// Number of known projects.
    pub known_project_count: u32,
    /// Number of active simulations.
    pub active_simulation_count: u32,
}

/// One applet summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AppletSummary {
    /// Fully-qualified applet class or label.
    pub class_name: String,
    /// Applet instance AID.
    pub aid: String,
}

/// One project summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectSummary {
    /// Stable local project id.
    pub project_id: String,
    /// Human-facing project name.
    pub name: String,
    /// Absolute project path.
    pub project_path: PathBuf,
    /// Java Card profile name.
    pub profile: String,
    /// Build strategy name.
    pub build_kind: String,
    /// Java package name.
    pub package_name: String,
    /// Package AID.
    pub package_aid: String,
    /// Declared applets.
    pub applets: Vec<AppletSummary>,
}

/// One project plus its current manifest TOML.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectDetails {
    /// High-level project summary.
    pub project: ProjectSummary,
    /// Pretty manifest text.
    pub manifest_toml: String,
}

/// One emitted build artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArtifactSummary {
    /// Artifact kind.
    pub kind: String,
    /// Absolute artifact path.
    pub path: PathBuf,
}

/// Result of one build request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BuildSummary {
    /// Built project.
    pub project: ProjectSummary,
    /// Emitted artifacts.
    pub artifacts: Vec<ArtifactSummary>,
    /// Whether the build rebuilt anything.
    pub rebuilt: bool,
}

/// Input source for one managed simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum SimulationSourceKind {
    /// Simulation came from a JCIM project.
    Project,
    /// Legacy source kind kept only for compatibility with older service values.
    Cap,
    /// Service returned an unknown value.
    Unknown,
}

/// Host mode for one simulation engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum SimulationEngineMode {
    /// Legacy engine mode returned by older services.
    Native,
    /// Legacy engine mode returned by older services.
    Container,
    /// Simulator is running in a bundled managed JVM.
    ManagedJava,
    /// Service returned an unknown value.
    Unknown,
}

/// Lifecycle state for one simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum SimulationStatus {
    /// Simulation is still starting.
    Starting,
    /// Simulation is ready for traffic.
    Running,
    /// Simulation was stopped cleanly.
    Stopped,
    /// Simulation failed.
    Failed,
    /// Service returned an unknown value.
    Unknown,
}

/// One simulation summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SimulationSummary {
    /// Stable simulation id.
    pub simulation_id: String,
    /// Input source used to create the simulation.
    pub source_kind: SimulationSourceKind,
    /// Owning project id when present.
    pub project_id: Option<String>,
    /// Owning project path when present.
    pub project_path: Option<PathBuf>,
    /// Installed CAP path.
    pub cap_path: PathBuf,
    /// Host engine mode.
    pub engine_mode: SimulationEngineMode,
    /// Current simulation state.
    pub status: SimulationStatus,
    /// Reader name reported by the backend.
    pub reader_name: String,
    /// Human-facing health detail.
    pub health: String,
    /// Parsed ATR when known.
    pub atr: Option<Atr>,
    /// Active transport protocol summary when known.
    pub active_protocol: Option<ProtocolParameters>,
    /// Explicit ISO/IEC 7816 capability summary.
    pub iso_capabilities: IsoCapabilities,
    /// Current tracked ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
    /// Number of packages visible in the simulation.
    pub package_count: u32,
    /// Number of applets visible in the simulation.
    pub applet_count: u32,
    /// Installed package name.
    pub package_name: String,
    /// Installed package AID.
    pub package_aid: String,
    /// Recent retained events.
    pub recent_events: Vec<String>,
}

impl SimulationSummary {
    /// Build a simulation selector for this summary.
    pub fn simulation_ref(&self) -> SimulationRef {
        SimulationRef::new(self.simulation_id.clone())
    }
}

/// One structured event line.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EventLine {
    /// Event severity.
    pub level: String,
    /// Event message.
    pub message: String,
}

/// One physical reader summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CardReaderSummary {
    /// Reader name.
    pub name: String,
    /// Whether a card is present.
    pub card_present: bool,
}

/// Structured card status.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CardStatusSummary {
    /// Effective reader name.
    pub reader_name: String,
    /// Whether a card is present.
    pub card_present: bool,
    /// Parsed ATR when known.
    pub atr: Option<Atr>,
    /// Active transport protocol summary when known.
    pub active_protocol: Option<ProtocolParameters>,
    /// Explicit ISO/IEC 7816 capability summary.
    pub iso_capabilities: IsoCapabilities,
    /// Current tracked ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
    /// Raw diagnostic lines.
    pub lines: Vec<String>,
}

/// Result of one typed or raw APDU exchange.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApduExchangeSummary {
    /// Command APDU transmitted by the SDK.
    pub command: CommandApdu,
    /// Parsed response APDU returned by the target.
    pub response: ResponseApdu,
    /// Updated ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
}

/// Result of one logical-channel management workflow.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManageChannelSummary {
    /// Opened or affected channel number when known.
    pub channel_number: Option<u8>,
    /// Underlying response APDU.
    pub response: ResponseApdu,
    /// Updated ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
}

/// Result of one secure-messaging management workflow.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SecureMessagingSummary {
    /// Updated ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
}

/// Result of one established GlobalPlatform secure-channel workflow.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GpSecureChannelSummary {
    /// Established secure-channel metadata.
    pub secure_channel: globalplatform::EstablishedSecureChannel,
    /// Security domain AID selected for the authenticated session.
    pub selected_aid: Aid,
    /// Updated ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
}

/// Result of one reset workflow.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResetSummary {
    /// Parsed ATR when available after reset.
    pub atr: Option<Atr>,
    /// Updated ISO/IEC 7816 session state after reset.
    pub session_state: IsoSessionState,
}

/// Result of installing a CAP onto a physical card.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CardInstallSummary {
    /// Effective reader name.
    pub reader_name: String,
    /// Installed CAP path.
    pub cap_path: PathBuf,
    /// Installed package name.
    pub package_name: String,
    /// Installed package AID.
    pub package_aid: String,
    /// Applets carried by the CAP.
    pub applets: Vec<AppletSummary>,
    /// Raw diagnostic lines.
    pub output_lines: Vec<String>,
}

/// Result of deleting one item from a physical card.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CardDeleteSummary {
    /// Effective reader name.
    pub reader_name: String,
    /// Deleted AID.
    pub aid: String,
    /// Whether the delete completed successfully.
    pub deleted: bool,
    /// Raw diagnostic lines.
    pub output_lines: Vec<String>,
}

/// One physical-card package summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CardPackageSummary {
    /// Package AID.
    pub aid: String,
    /// Adapter-parsed description.
    pub description: String,
}

/// One physical-card applet summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CardAppletSummary {
    /// Applet AID.
    pub aid: String,
    /// Adapter-parsed description.
    pub description: String,
}

/// Inventory of physical-card packages.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CardPackageInventory {
    /// Effective reader name.
    pub reader_name: String,
    /// Parsed package items.
    pub packages: Vec<CardPackageSummary>,
    /// Raw diagnostic lines.
    pub output_lines: Vec<String>,
}

/// Inventory of physical-card applets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CardAppletInventory {
    /// Effective reader name.
    pub reader_name: String,
    /// Parsed applet items.
    pub applets: Vec<CardAppletSummary>,
    /// Raw diagnostic lines.
    pub output_lines: Vec<String>,
}

/// Result of `jcim system setup`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SetupSummary {
    /// Machine-local config path.
    pub config_path: PathBuf,
    /// Human-facing setup message.
    pub message: String,
}

/// Summary of local service status.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ServiceStatusSummary {
    /// Managed local socket path.
    pub socket_path: PathBuf,
    /// Whether the service is running.
    pub running: bool,
    /// Number of known projects.
    pub known_project_count: u32,
    /// Number of active simulations.
    pub active_simulation_count: u32,
    /// Path to the `jcimd` binary that booted the current service instance.
    pub service_binary_path: PathBuf,
    /// Startup-captured fingerprint of the daemon binary behind the current service instance.
    pub service_binary_fingerprint: String,
}

/// Build a `PathBuf` from one borrowed path.
pub(crate) fn owned_path(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref().to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{ProjectRef, ReaderRef, SimulationRef};

    #[test]
    fn project_ref_constructors_preserve_selector_shape() {
        let by_path = ProjectRef::from_path("examples/satochip/workdir");
        assert!(by_path.project_path.is_some());
        assert!(by_path.project_id.is_none());

        let by_id = ProjectRef::from_id("proj-123");
        assert!(by_id.project_path.is_none());
        assert_eq!(by_id.project_id.as_deref(), Some("proj-123"));
    }

    #[test]
    fn reader_ref_named_and_default_behave_as_expected() {
        assert_eq!(ReaderRef::Default.as_deref(), None);
        assert_eq!(ReaderRef::named("Reader 0").as_deref(), Some("Reader 0"));
    }

    #[test]
    fn simulation_ref_constructor_preserves_id() {
        let reference = SimulationRef::new("sim-123");
        assert_eq!(reference.simulation_id, "sim-123");
    }
}
