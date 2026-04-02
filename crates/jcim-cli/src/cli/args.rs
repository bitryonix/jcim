use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Root CLI parser with one global JSON-mode switch and one task-oriented subcommand.
#[derive(Debug, Parser)]
#[command(name = "jcim")]
#[command(about = "JCIM 0.3 local simulator workbench CLI")]
pub(super) struct Cli {
    /// Emit structured JSON instead of human-readable text.
    #[arg(long, global = true)]
    pub(super) json: bool,
    /// Top-level task-oriented command selected by the caller.
    #[command(subcommand)]
    pub(super) command: Command,
}

/// Top-level task-oriented CLI command family.
#[derive(Debug, Subcommand)]
pub(super) enum Command {
    /// Create, show, or clean projects.
    Project {
        /// Project subcommand selected by the caller.
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Build the current or selected project.
    Build(BuildCommand),
    /// Start, inspect, and control managed simulations.
    Sim {
        /// Simulation subcommand selected by the caller.
        #[command(subcommand)]
        command: SimCommand,
    },
    /// Interact with physical readers and cards.
    Card {
        /// Physical-card subcommand selected by the caller.
        #[command(subcommand)]
        command: CardCommand,
    },
    /// Configure and inspect the local JCIM service.
    System {
        /// System subcommand selected by the caller.
        #[command(subcommand)]
        command: SystemCommand,
    },
}

/// Project-oriented CLI subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum ProjectCommand {
    /// Create a new JCIM project skeleton.
    New(ProjectNewArgs),
    /// Show the current project manifest and metadata.
    Show(ProjectSelectorArgs),
    /// Remove generated project-local build state.
    Clean(ProjectSelectorArgs),
}

/// Arguments for the `project new` command.
#[derive(Debug, Args)]
pub(super) struct ProjectNewArgs {
    /// Human-facing project name.
    pub(super) name: String,
    /// Directory where the project should be created.
    #[arg(long)]
    pub(super) directory: Option<PathBuf>,
}

/// Reusable project selector arguments shared across commands.
#[derive(Debug, Args, Clone)]
pub(super) struct ProjectSelectorArgs {
    /// Project directory or `jcim.toml` path.
    #[arg(long)]
    pub(super) project: Option<PathBuf>,
    /// Registered project id.
    #[arg(long)]
    pub(super) id: Option<String>,
}

/// Arguments for the `build` command family.
#[derive(Debug, Args)]
pub(super) struct BuildCommand {
    /// Optional build subcommand such as `artifacts`.
    #[command(subcommand)]
    pub(super) command: Option<BuildSubcommand>,
    /// Project selector used when the build operates on one project.
    #[command(flatten)]
    pub(super) project: ProjectSelectorArgs,
}

/// Build-oriented CLI subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum BuildSubcommand {
    /// Show the current persisted artifact set for a project.
    Artifacts(ProjectSelectorArgs),
}

/// Simulation-oriented CLI subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum SimCommand {
    /// Start a new simulation from a project.
    Start(SimStartArgs),
    /// Stop a simulation.
    Stop(SimulationArgs),
    /// Show current simulations or one selected simulation.
    Status(SimulationArgs),
    /// Show retained simulation events.
    Logs(SimulationArgs),
    /// Send one APDU to a running simulation.
    Apdu(SimApduArgs),
    /// Reset a running simulation.
    Reset(SimulationArgs),
    /// Run typed ISO/IEC 7816 operations against a simulation.
    Iso {
        /// Typed ISO/IEC 7816 simulation subcommand selected by the caller.
        #[command(subcommand)]
        command: SimIsoCommand,
    },
    /// Run typed GlobalPlatform administration workflows against a simulation.
    Gp {
        /// Typed GlobalPlatform simulation subcommand selected by the caller.
        #[command(subcommand)]
        command: SimGpCommand,
    },
}

/// Typed ISO/IEC 7816 simulation subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum SimIsoCommand {
    /// Show the tracked ISO/IEC 7816 session state.
    Status(SimulationArgs),
    /// Send a typed `SELECT` by AID.
    Select(SimIsoSelectArgs),
    /// Open one logical channel.
    ChannelOpen(SimulationArgs),
    /// Close one logical channel.
    ChannelClose(SimIsoChannelCloseArgs),
    /// Mark secure messaging as active.
    SecureOpen(SimIsoSecureOpenArgs),
    /// Advance the tracked secure-messaging counter.
    SecureAdvance(SimIsoSecureAdvanceArgs),
    /// Clear the tracked secure-messaging state.
    SecureClose(SimulationArgs),
}

/// Typed GlobalPlatform simulation subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum SimGpCommand {
    /// Open or close one authenticated GP secure channel.
    Auth {
        /// GP secure-channel auth subcommand selected by the caller.
        #[command(subcommand)]
        command: SimGpAuthCommand,
    },
    /// Select the issuer security domain.
    SelectIsd(SimulationArgs),
    /// Run `GET STATUS`.
    GetStatus(SimGpGetStatusArgs),
    /// Change the card life cycle state with `SET STATUS`.
    SetCardStatus(SimGpSetCardStatusArgs),
    /// Lock or unlock one application with `SET STATUS`.
    SetApplicationStatus(SimGpSetTargetStatusArgs),
    /// Lock or unlock one security domain and its applications with `SET STATUS`.
    SetSecurityDomainStatus(SimGpSetTargetStatusArgs),
}

/// Arguments for the `sim start` command.
#[derive(Debug, Args)]
pub(super) struct SimStartArgs {
    /// Project selector used to choose the simulation source project.
    #[command(flatten)]
    pub(super) project: ProjectSelectorArgs,
}

/// Reusable simulation selector arguments shared across commands.
#[derive(Debug, Args)]
pub(super) struct SimulationArgs {
    /// Simulation id. When omitted and exactly one simulation exists, JCIM uses that one.
    #[arg(long)]
    pub(super) simulation: Option<String>,
}

/// Arguments for the `sim apdu` command.
#[derive(Debug, Args)]
pub(super) struct SimApduArgs {
    /// Simulation selector used to choose the running simulation target.
    #[command(flatten)]
    pub(super) simulation: SimulationArgs,
    /// Raw APDU in hexadecimal.
    pub(super) apdu_hex: String,
}

/// Arguments for the `sim iso select` command.
#[derive(Debug, Args)]
pub(super) struct SimIsoSelectArgs {
    /// Simulation selector used to choose the running simulation target.
    #[command(flatten)]
    pub(super) simulation: SimulationArgs,
    /// Application identifier to select.
    #[arg(long)]
    pub(super) aid: String,
}

/// Arguments for the `sim iso channel-close` command.
#[derive(Debug, Args)]
pub(super) struct SimIsoChannelCloseArgs {
    /// Simulation selector used to choose the running simulation target.
    #[command(flatten)]
    pub(super) simulation: SimulationArgs,
    /// Logical channel number to close.
    #[arg(long)]
    pub(super) channel: u8,
}

/// Arguments for the `sim iso secure-open` command.
#[derive(Debug, Args)]
pub(super) struct SimIsoSecureOpenArgs {
    /// Simulation selector used to choose the running simulation target.
    #[command(flatten)]
    pub(super) simulation: SimulationArgs,
    /// Secure messaging protocol: `iso7816`, `scp02`, `scp03`, or `other:<label>`.
    #[arg(long)]
    pub(super) protocol: String,
    /// Security level byte.
    #[arg(long)]
    pub(super) security_level: Option<u8>,
    /// Optional session identifier.
    #[arg(long)]
    pub(super) session_id: Option<String>,
}

/// Arguments for the `sim iso secure-advance` command.
#[derive(Debug, Args)]
pub(super) struct SimIsoSecureAdvanceArgs {
    /// Simulation selector used to choose the running simulation target.
    #[command(flatten)]
    pub(super) simulation: SimulationArgs,
    /// Counter increment, defaults to 1.
    #[arg(long, default_value_t = 1)]
    pub(super) increment: u32,
}

/// Arguments for the `sim gp get-status` command.
#[derive(Debug, Args)]
pub(super) struct SimGpGetStatusArgs {
    /// Simulation selector used to choose the running simulation target.
    #[command(flatten)]
    pub(super) simulation: SimulationArgs,
    /// Registry subset to query.
    #[arg(long, value_enum)]
    pub(super) kind: GpRegistryKindArg,
    /// Whether to request the first page or a continuation page.
    #[arg(long, value_enum, default_value = "first-or-all")]
    pub(super) occurrence: GpOccurrenceArg,
}

/// GP secure-channel simulation auth subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum SimGpAuthCommand {
    /// Open one authenticated GP secure channel.
    Open(SimGpAuthOpenArgs),
    /// Close the current authenticated GP secure channel.
    Close(SimulationArgs),
}

/// Arguments for the `sim gp auth open` command.
#[derive(Debug, Args)]
pub(super) struct SimGpAuthOpenArgs {
    /// Simulation selector used to choose the running simulation target.
    #[command(flatten)]
    pub(super) simulation: SimulationArgs,
    /// GP keyset name. When omitted, JCIM uses `JCIM_GP_DEFAULT_KEYSET`.
    #[arg(long)]
    pub(super) keyset: Option<String>,
    /// GP security level byte. Defaults to `0x01` when omitted.
    #[arg(long)]
    pub(super) security_level: Option<u8>,
}

/// Arguments for the `sim gp set-card-status` command.
#[derive(Debug, Args)]
pub(super) struct SimGpSetCardStatusArgs {
    /// Simulation selector used to choose the running simulation target.
    #[command(flatten)]
    pub(super) simulation: SimulationArgs,
    /// Target card life cycle state.
    #[arg(long, value_enum)]
    pub(super) state: GpCardStateArg,
}

/// Arguments for the `sim gp set-application-status` and `set-security-domain-status` commands.
#[derive(Debug, Args)]
pub(super) struct SimGpSetTargetStatusArgs {
    /// Simulation selector used to choose the running simulation target.
    #[command(flatten)]
    pub(super) simulation: SimulationArgs,
    /// Target application or security-domain AID.
    #[arg(long)]
    pub(super) aid: String,
    /// Lock transition to apply.
    #[arg(long, value_enum)]
    pub(super) transition: GpTransitionArg,
}

/// Physical-card-oriented CLI subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum CardCommand {
    /// List visible PC/SC readers.
    Readers,
    /// Show reader and card status.
    Status(CardReaderArgs),
    /// Install a CAP onto a physical card.
    Install(CardInstallArgs),
    /// Delete a package from a physical card by AID.
    Delete(CardDeleteArgs),
    /// List packages visible on a physical card.
    Packages(CardReaderArgs),
    /// List applets visible on a physical card.
    Applets(CardReaderArgs),
    /// Send one APDU to a physical card.
    Apdu(CardApduArgs),
    /// Reset a physical card.
    Reset(CardReaderArgs),
    /// Run typed ISO/IEC 7816 operations against a physical card.
    Iso {
        /// Typed ISO/IEC 7816 physical-card subcommand selected by the caller.
        #[command(subcommand)]
        command: CardIsoCommand,
    },
    /// Run typed GlobalPlatform administration workflows against a physical card.
    Gp {
        /// Typed GlobalPlatform physical-card subcommand selected by the caller.
        #[command(subcommand)]
        command: CardGpCommand,
    },
}

/// Typed ISO/IEC 7816 physical-card subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum CardIsoCommand {
    /// Show the tracked ISO/IEC 7816 session state.
    Status(CardReaderArgs),
    /// Send a typed `SELECT` by AID.
    Select(CardIsoSelectArgs),
    /// Open one logical channel.
    ChannelOpen(CardReaderArgs),
    /// Close one logical channel.
    ChannelClose(CardIsoChannelCloseArgs),
    /// Mark secure messaging as active.
    SecureOpen(CardIsoSecureOpenArgs),
    /// Advance the tracked secure-messaging counter.
    SecureAdvance(CardIsoSecureAdvanceArgs),
    /// Clear the tracked secure-messaging state.
    SecureClose(CardReaderArgs),
}

/// Typed GlobalPlatform physical-card subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum CardGpCommand {
    /// Open or close one authenticated GP secure channel.
    Auth {
        /// GP secure-channel auth subcommand selected by the caller.
        #[command(subcommand)]
        command: CardGpAuthCommand,
    },
    /// Select the issuer security domain.
    SelectIsd(CardReaderArgs),
    /// Run `GET STATUS`.
    GetStatus(CardGpGetStatusArgs),
    /// Change the card life cycle state with `SET STATUS`.
    SetCardStatus(CardGpSetCardStatusArgs),
    /// Lock or unlock one application with `SET STATUS`.
    SetApplicationStatus(CardGpSetTargetStatusArgs),
    /// Lock or unlock one security domain and its applications with `SET STATUS`.
    SetSecurityDomainStatus(CardGpSetTargetStatusArgs),
}

/// Reusable physical-reader selector arguments shared across card commands.
#[derive(Debug, Args)]
pub(super) struct CardReaderArgs {
    /// Physical reader name.
    #[arg(long)]
    pub(super) reader: Option<String>,
}

/// Arguments for the `card install` command.
#[derive(Debug, Args)]
pub(super) struct CardInstallArgs {
    /// Project selector used to resolve the CAP source when `--cap` is omitted.
    #[command(flatten)]
    pub(super) project: ProjectSelectorArgs,
    /// Physical reader name.
    #[arg(long)]
    pub(super) reader: Option<String>,
    /// Explicit CAP path. When omitted, JCIM uses the project CAP artifact.
    #[arg(long)]
    pub(super) cap: Option<PathBuf>,
}

/// Arguments for the `card delete` command.
#[derive(Debug, Args)]
pub(super) struct CardDeleteArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// Package AID to delete.
    pub(super) aid: String,
}

/// Arguments for the `card apdu` command.
#[derive(Debug, Args)]
pub(super) struct CardApduArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// Raw APDU in hexadecimal.
    pub(super) apdu_hex: String,
}

/// Arguments for the `card iso select` command.
#[derive(Debug, Args)]
pub(super) struct CardIsoSelectArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// Application identifier to select.
    #[arg(long)]
    pub(super) aid: String,
}

/// Arguments for the `card iso channel-close` command.
#[derive(Debug, Args)]
pub(super) struct CardIsoChannelCloseArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// Logical channel number to close.
    #[arg(long)]
    pub(super) channel: u8,
}

/// Arguments for the `card iso secure-open` command.
#[derive(Debug, Args)]
pub(super) struct CardIsoSecureOpenArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// Secure messaging protocol: `iso7816`, `scp02`, `scp03`, or `other:<label>`.
    #[arg(long)]
    pub(super) protocol: String,
    /// Security level byte.
    #[arg(long)]
    pub(super) security_level: Option<u8>,
    /// Optional session identifier.
    #[arg(long)]
    pub(super) session_id: Option<String>,
}

/// Arguments for the `card iso secure-advance` command.
#[derive(Debug, Args)]
pub(super) struct CardIsoSecureAdvanceArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// Counter increment, defaults to 1.
    #[arg(long, default_value_t = 1)]
    pub(super) increment: u32,
}

/// Arguments for the `card gp get-status` command.
#[derive(Debug, Args)]
pub(super) struct CardGpGetStatusArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// Registry subset to query.
    #[arg(long, value_enum)]
    pub(super) kind: GpRegistryKindArg,
    /// Whether to request the first page or a continuation page.
    #[arg(long, value_enum, default_value = "first-or-all")]
    pub(super) occurrence: GpOccurrenceArg,
}

/// GP secure-channel physical-card auth subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum CardGpAuthCommand {
    /// Open one authenticated GP secure channel.
    Open(CardGpAuthOpenArgs),
    /// Close the current authenticated GP secure channel.
    Close(CardReaderArgs),
}

/// Arguments for the `card gp auth open` command.
#[derive(Debug, Args)]
pub(super) struct CardGpAuthOpenArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// GP keyset name. When omitted, JCIM uses `JCIM_GP_DEFAULT_KEYSET`.
    #[arg(long)]
    pub(super) keyset: Option<String>,
    /// GP security level byte. Defaults to `0x01` when omitted.
    #[arg(long)]
    pub(super) security_level: Option<u8>,
}

/// Arguments for the `card gp set-card-status` command.
#[derive(Debug, Args)]
pub(super) struct CardGpSetCardStatusArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// Target card life cycle state.
    #[arg(long, value_enum)]
    pub(super) state: GpCardStateArg,
}

/// Arguments for the `card gp set-application-status` and `set-security-domain-status` commands.
#[derive(Debug, Args)]
pub(super) struct CardGpSetTargetStatusArgs {
    /// Reader selector used to choose the physical card target.
    #[command(flatten)]
    pub(super) reader: CardReaderArgs,
    /// Target application or security-domain AID.
    #[arg(long)]
    pub(super) aid: String,
    /// Lock transition to apply.
    #[arg(long, value_enum)]
    pub(super) transition: GpTransitionArg,
}

/// CLI enum for GP registry subsets accepted by `get-status`.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum GpRegistryKindArg {
    /// Query the issuer security domain registry.
    Isd,
    /// Query installed applications.
    Applications,
    /// Query load files only.
    LoadFiles,
    /// Query load files and their modules.
    LoadFilesAndModules,
}

/// CLI enum for GP `GET STATUS` paging behavior.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum GpOccurrenceArg {
    /// Request the first page, or all entries when the platform returns them in one response.
    FirstOrAll,
    /// Request the next continuation page.
    Next,
}

/// CLI enum for GP card life-cycle states accepted by `set-card-status`.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum GpCardStateArg {
    /// Card is operationally ready.
    OpReady,
    /// Card is initialized but not yet secured.
    Initialized,
    /// Card is secured.
    Secured,
    /// Card is locked.
    CardLocked,
    /// Card is terminated.
    Terminated,
}

/// CLI enum for GP application and security-domain lock transitions.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum GpTransitionArg {
    /// Lock the target.
    Lock,
    /// Unlock the target.
    Unlock,
}

/// System-oriented CLI subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum SystemCommand {
    /// Persist machine-local toolchain settings.
    Setup(SystemSetupArgs),
    /// Show a doctor report for the local environment.
    Doctor,
    /// Show local service status without starting it.
    Service {
        /// Local-service subcommand selected by the caller.
        #[command(subcommand)]
        command: SystemServiceCommand,
    },
}

/// Arguments for the `system setup` command.
#[derive(Debug, Args)]
pub(super) struct SystemSetupArgs {
    /// Override the Java executable used by JCIM-managed tools.
    #[arg(long)]
    pub(super) java_bin: Option<String>,
}

/// Local-service subcommands under `system service`.
#[derive(Debug, Subcommand)]
pub(super) enum SystemServiceCommand {
    /// Show the current local service socket and status.
    Status,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{
        CardCommand, CardGpAuthCommand, CardGpCommand, CardIsoCommand, Cli, Command, SimCommand,
        SimGpAuthCommand, SimGpCommand, SimIsoCommand,
    };

    #[test]
    fn sim_start_accepts_project_selector() {
        let cli = Cli::try_parse_from([
            "jcim",
            "sim",
            "start",
            "--project",
            "examples/satochip/workdir",
        ])
        .expect("parse project");
        match cli.command {
            Command::Sim {
                command: SimCommand::Start(args),
            } => {
                assert_eq!(
                    args.project.project.expect("project path"),
                    std::path::PathBuf::from("examples/satochip/workdir")
                );
                assert!(args.project.id.is_none());
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn sim_iso_select_accepts_aid() {
        let cli = Cli::try_parse_from([
            "jcim",
            "sim",
            "iso",
            "select",
            "--simulation",
            "sim-123",
            "--aid",
            "A0000001510000",
        ])
        .expect("parse sim iso select");
        match cli.command {
            Command::Sim {
                command:
                    SimCommand::Iso {
                        command: SimIsoCommand::Select(args),
                    },
            } => {
                assert_eq!(args.simulation.simulation.as_deref(), Some("sim-123"));
                assert_eq!(args.aid, "A0000001510000");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn card_iso_secure_open_accepts_protocol_and_level() {
        let cli = Cli::try_parse_from([
            "jcim",
            "card",
            "iso",
            "secure-open",
            "--reader",
            "Reader 0",
            "--protocol",
            "scp03",
            "--security-level",
            "3",
            "--session-id",
            "session-1",
        ])
        .expect("parse card iso secure-open");
        match cli.command {
            Command::Card {
                command:
                    CardCommand::Iso {
                        command: CardIsoCommand::SecureOpen(args),
                    },
            } => {
                assert_eq!(args.reader.reader.as_deref(), Some("Reader 0"));
                assert_eq!(args.protocol, "scp03");
                assert_eq!(args.security_level, Some(3));
                assert_eq!(args.session_id.as_deref(), Some("session-1"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn sim_gp_get_status_accepts_kind_and_occurrence() {
        let cli = Cli::try_parse_from([
            "jcim",
            "sim",
            "gp",
            "get-status",
            "--simulation",
            "sim-123",
            "--kind",
            "applications",
            "--occurrence",
            "next",
        ])
        .expect("parse sim gp get-status");
        match cli.command {
            Command::Sim {
                command:
                    SimCommand::Gp {
                        command: SimGpCommand::GetStatus(args),
                    },
            } => {
                assert_eq!(args.simulation.simulation.as_deref(), Some("sim-123"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn sim_gp_auth_open_accepts_keyset_and_level() {
        let cli = Cli::try_parse_from([
            "jcim",
            "sim",
            "gp",
            "auth",
            "open",
            "--simulation",
            "sim-123",
            "--keyset",
            "admin",
            "--security-level",
            "3",
        ])
        .expect("parse sim gp auth open");
        match cli.command {
            Command::Sim {
                command:
                    SimCommand::Gp {
                        command:
                            SimGpCommand::Auth {
                                command: SimGpAuthCommand::Open(args),
                            },
                    },
            } => {
                assert_eq!(args.simulation.simulation.as_deref(), Some("sim-123"));
                assert_eq!(args.keyset.as_deref(), Some("admin"));
                assert_eq!(args.security_level, Some(3));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn card_gp_set_card_status_accepts_state() {
        let cli = Cli::try_parse_from([
            "jcim",
            "card",
            "gp",
            "set-card-status",
            "--reader",
            "Reader 0",
            "--state",
            "terminated",
        ])
        .expect("parse card gp set-card-status");
        match cli.command {
            Command::Card {
                command:
                    CardCommand::Gp {
                        command: CardGpCommand::SetCardStatus(args),
                    },
            } => {
                assert_eq!(args.reader.reader.as_deref(), Some("Reader 0"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn card_gp_auth_open_accepts_keyset_and_reader() {
        let cli = Cli::try_parse_from([
            "jcim", "card", "gp", "auth", "open", "--reader", "Reader 0", "--keyset", "admin",
        ])
        .expect("parse card gp auth open");
        match cli.command {
            Command::Card {
                command:
                    CardCommand::Gp {
                        command:
                            CardGpCommand::Auth {
                                command: CardGpAuthCommand::Open(args),
                            },
                    },
            } => {
                assert_eq!(args.reader.reader.as_deref(), Some("Reader 0"));
                assert_eq!(args.keyset.as_deref(), Some("admin"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
