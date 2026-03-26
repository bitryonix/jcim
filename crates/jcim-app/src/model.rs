//! Public application-service types exposed by `jcim-app`.

use std::path::PathBuf;

use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::globalplatform;
use jcim_core::iso7816::{Atr, IsoCapabilities, IsoSessionState, ProtocolParameters};

/// Selector used to resolve one JCIM project.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectSelectorInput {
    /// Absolute or relative project path. Directory and `jcim.toml` file paths are both accepted.
    pub project_path: Option<PathBuf>,
    /// Stable local project id assigned by the registry.
    pub project_id: Option<String>,
}

/// Selector used to resolve one running simulation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationSelectorInput {
    /// Simulation id previously returned by `start_simulation`.
    pub simulation_id: String,
}

/// Input source used for one running simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationSourceKind {
    /// A JCIM project was built to CAP and started in the simulator.
    Project,
    /// A raw CAP path was provided directly.
    Cap,
}

impl SimulationSourceKind {
    /// Return the canonical lowercase name used in user-facing surfaces.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Cap => "cap",
        }
    }
}

/// Engine mode used to host one running simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationEngineMode {
    /// The official simulator is running natively on the local machine.
    Native,
    /// The official simulator is running behind a managed container wrapper.
    Container,
}

impl SimulationEngineMode {
    /// Return the canonical lowercase name used in user-facing surfaces.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Container => "container",
        }
    }
}

/// Lifecycle status for a managed simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationStatusKind {
    /// Simulation initialization is in progress.
    Starting,
    /// Simulation is healthy and ready for traffic.
    Running,
    /// Simulation was stopped cleanly.
    Stopped,
    /// Simulation failed and can no longer serve requests.
    Failed,
}

impl SimulationStatusKind {
    /// Return the canonical lowercase status string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }
}

/// High-level overview of the local JCIM service state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverviewSummary {
    /// Number of known projects in the local registry.
    pub known_project_count: u32,
    /// Number of currently active simulations.
    pub active_simulation_count: u32,
}

/// One applet declared by a project manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppletSummary {
    /// Fully-qualified applet class name.
    pub class_name: String,
    /// Applet instance AID.
    pub aid: String,
}

/// Project metadata returned by registry and build operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSummary {
    /// Stable local project id.
    pub project_id: String,
    /// Human-facing project name.
    pub name: String,
    /// Absolute project root path.
    pub project_path: PathBuf,
    /// Java Card profile name.
    pub profile: String,
    /// Build strategy name.
    pub build_kind: String,
    /// Java package name.
    pub package_name: String,
    /// Package AID in uppercase hexadecimal.
    pub package_aid: String,
    /// Declared applets.
    pub applets: Vec<AppletSummary>,
}

/// Project summary plus the current manifest contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDetails {
    /// High-level project summary.
    pub project: ProjectSummary,
    /// Pretty TOML manifest text.
    pub manifest_toml: String,
}

/// One emitted build artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSummary {
    /// Artifact kind such as `cap`.
    pub kind: String,
    /// Absolute artifact path.
    pub path: PathBuf,
}

/// Structured build or simulation event line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventLine {
    /// Severity label such as `info` or `error`.
    pub level: String,
    /// Human-facing message payload.
    pub message: String,
}

/// Running simulation summary returned by simulator operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationSummary {
    /// Stable simulation id.
    pub simulation_id: String,
    /// Input source used to create the running simulation.
    pub source_kind: SimulationSourceKind,
    /// Owning project id when the simulation came from a project.
    pub project_id: Option<String>,
    /// Absolute project root path when the simulation came from a project.
    pub project_path: Option<PathBuf>,
    /// Absolute CAP path installed into the simulator.
    pub cap_path: PathBuf,
    /// Simulator engine mode.
    pub engine_mode: SimulationEngineMode,
    /// Current simulation lifecycle status.
    pub status: SimulationStatusKind,
    /// Reader name exposed by the simulation.
    pub reader_name: String,
    /// Human-facing health detail.
    pub health: String,
    /// Parsed ATR when known.
    pub atr: Option<Atr>,
    /// Active transport protocol summary when known.
    pub active_protocol: Option<ProtocolParameters>,
    /// Explicit ISO/IEC 7816 capability summary.
    pub iso_capabilities: IsoCapabilities,
    /// Tracked ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
    /// Number of packages visible in the simulation snapshot.
    pub package_count: u32,
    /// Number of applets visible in the simulation snapshot.
    pub applet_count: u32,
    /// Installed CAP package name when known.
    pub package_name: String,
    /// Installed CAP package AID when known.
    pub package_aid: String,
    /// Recent simulation events rendered for operators.
    pub recent_events: Vec<String>,
}

/// One physical PC/SC reader summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardReaderSummary {
    /// Reader name reported by the OS smart-card stack.
    pub name: String,
    /// Whether a card is currently present.
    pub card_present: bool,
}

/// Structured physical-card status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardStatusSummary {
    /// Effective reader name.
    pub reader_name: String,
    /// Whether a card is currently present.
    pub card_present: bool,
    /// Parsed ATR when available.
    pub atr: Option<Atr>,
    /// Active protocol summary when known.
    pub active_protocol: Option<ProtocolParameters>,
    /// Explicit ISO/IEC 7816 capability summary.
    pub iso_capabilities: IsoCapabilities,
    /// Current tracked ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
    /// Raw helper output lines preserved for diagnostics.
    pub lines: Vec<String>,
}

/// Result of one typed or raw APDU exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApduExchangeSummary {
    /// Command APDU transmitted by JCIM.
    pub command: CommandApdu,
    /// Parsed response APDU returned by the target.
    pub response: ResponseApdu,
    /// Updated ISO/IEC 7816 session state after applying the response.
    pub session_state: IsoSessionState,
}

/// Result of one `MANAGE CHANNEL` workflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManageChannelSummary {
    /// Opened or affected channel number when known.
    pub channel_number: Option<u8>,
    /// Underlying response APDU.
    pub response: ResponseApdu,
    /// Updated ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
}

/// Result of one secure-messaging session management workflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecureMessagingSummary {
    /// Updated ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
}

/// Result of one established GlobalPlatform secure-channel workflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpSecureChannelSummary {
    /// Established secure-channel metadata.
    pub secure_channel: globalplatform::EstablishedSecureChannel,
    /// Security domain AID selected for the authenticated session.
    pub selected_aid: Aid,
    /// Updated ISO/IEC 7816 session state.
    pub session_state: IsoSessionState,
}

/// Result of one reset workflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResetSummary {
    /// Parsed ATR when available after reset.
    pub atr: Option<Atr>,
    /// Updated ISO/IEC 7816 session state after reset.
    pub session_state: IsoSessionState,
}

/// Result of installing one CAP onto a physical card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardInstallSummary {
    /// Effective reader name when JCIM resolved one.
    pub reader_name: String,
    /// Installed CAP path.
    pub cap_path: PathBuf,
    /// Installed package name discovered from the CAP.
    pub package_name: String,
    /// Installed package AID.
    pub package_aid: String,
    /// Declared applets installed with the package.
    pub applets: Vec<AppletSummary>,
    /// Raw adapter output preserved for diagnostics.
    pub output_lines: Vec<String>,
}

/// Result of deleting one item from a physical card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardDeleteSummary {
    /// Effective reader name when JCIM resolved one.
    pub reader_name: String,
    /// Deleted AID.
    pub aid: String,
    /// Whether the command completed successfully.
    pub deleted: bool,
    /// Raw adapter output preserved for diagnostics.
    pub output_lines: Vec<String>,
}

/// One package visible on a physical card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardPackageSummary {
    /// Package AID.
    pub aid: String,
    /// Adapter-parsed human-facing description.
    pub description: String,
}

/// One applet visible on a physical card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardAppletSummary {
    /// Applet AID.
    pub aid: String,
    /// Adapter-parsed human-facing description.
    pub description: String,
}

/// Typed inventory of packages visible on a physical card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardPackageInventory {
    /// Effective reader name when JCIM resolved one.
    pub reader_name: String,
    /// Parsed package inventory.
    pub packages: Vec<CardPackageSummary>,
    /// Raw adapter output preserved for diagnostics.
    pub output_lines: Vec<String>,
}

/// Typed inventory of applets visible on a physical card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardAppletInventory {
    /// Effective reader name when JCIM resolved one.
    pub reader_name: String,
    /// Parsed applet inventory.
    pub applets: Vec<CardAppletSummary>,
    /// Raw adapter output preserved for diagnostics.
    pub output_lines: Vec<String>,
}

/// Result of `system setup`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetupSummary {
    /// Machine-local config file path.
    pub config_path: PathBuf,
    /// Human-facing setup message.
    pub message: String,
}

/// Local service status summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceStatusSummary {
    /// Local gRPC Unix-domain socket path.
    pub socket_path: PathBuf,
    /// Whether the queried service instance is running.
    pub running: bool,
    /// Number of known projects in the registry.
    pub known_project_count: u32,
    /// Number of active simulations currently managed by the service.
    pub active_simulation_count: u32,
}
