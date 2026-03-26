//! Command parsing and task-oriented CLI execution.

#![allow(clippy::missing_docs_in_private_items)]

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use jcim_config::project::find_project_manifest;
use jcim_core::apdu::CommandApdu;
use jcim_sdk::iso7816::{FileSelection, IsoSessionState, SecureMessagingProtocol};
use jcim_sdk::{
    Aid, BuildSummary, CardAppletInventory, CardDeleteSummary, CardInstallSource,
    CardInstallSummary, CardPackageInventory, GpSecureChannelSummary, JcimClient,
    ManageChannelSummary, ProjectDetails, ProjectRef, ReaderRef, SecureMessagingSummary,
    ServiceStatusSummary, SimulationInput, SimulationRef, SimulationSourceKind, SimulationStatus,
    SimulationSummary, globalplatform,
};
use serde_json::json;

/// Parse and execute one CLI command.
pub(crate) async fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Project { command } => run_project(command, cli.json).await,
        Command::Build(command) => run_build(command, cli.json).await,
        Command::Sim { command } => run_sim(command, cli.json).await,
        Command::Card { command } => run_card(command, cli.json).await,
        Command::System { command } => run_system(command, cli.json).await,
    }
}

#[derive(Debug, Parser)]
#[command(name = "jcim")]
#[command(about = "JCIM 0.2 local simulator workbench CLI")]
struct Cli {
    /// Emit structured JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create, show, or clean projects.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Build the current or selected project.
    Build(BuildCommand),
    /// Start, inspect, and control CAP-first simulations.
    Sim {
        #[command(subcommand)]
        command: SimCommand,
    },
    /// Interact with physical readers and cards.
    Card {
        #[command(subcommand)]
        command: CardCommand,
    },
    /// Configure and inspect the local JCIM service.
    System {
        #[command(subcommand)]
        command: SystemCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Create a new JCIM project skeleton.
    New(ProjectNewArgs),
    /// Show the current project manifest and metadata.
    Show(ProjectSelectorArgs),
    /// Remove generated project-local build state.
    Clean(ProjectSelectorArgs),
}

#[derive(Debug, Args)]
struct ProjectNewArgs {
    /// Human-facing project name.
    name: String,
    /// Directory where the project should be created.
    #[arg(long)]
    directory: Option<PathBuf>,
}

#[derive(Debug, Args, Clone)]
struct ProjectSelectorArgs {
    /// Project directory or `jcim.toml` path.
    #[arg(long)]
    project: Option<PathBuf>,
    /// Registered project id.
    #[arg(long)]
    id: Option<String>,
}

#[derive(Debug, Args)]
struct BuildCommand {
    #[command(subcommand)]
    command: Option<BuildSubcommand>,
    #[command(flatten)]
    project: ProjectSelectorArgs,
}

#[derive(Debug, Subcommand)]
enum BuildSubcommand {
    /// Show the current persisted artifact set for a project.
    Artifacts(ProjectSelectorArgs),
}

#[derive(Debug, Subcommand)]
enum SimCommand {
    /// Start a new simulation from a project or raw CAP.
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
        #[command(subcommand)]
        command: SimIsoCommand,
    },
    /// Run typed GlobalPlatform administration workflows against a simulation.
    Gp {
        #[command(subcommand)]
        command: SimGpCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SimIsoCommand {
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

#[derive(Debug, Subcommand)]
enum SimGpCommand {
    /// Open or close one authenticated GP secure channel.
    Auth {
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

#[derive(Debug, Args)]
struct SimStartArgs {
    #[command(flatten)]
    project: ProjectSelectorArgs,
    /// Raw CAP path to start directly in the simulator.
    #[arg(long)]
    cap: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct SimulationArgs {
    /// Simulation id. When omitted and exactly one simulation exists, JCIM uses that one.
    #[arg(long)]
    simulation: Option<String>,
}

#[derive(Debug, Args)]
struct SimApduArgs {
    #[command(flatten)]
    simulation: SimulationArgs,
    /// Raw APDU in hexadecimal.
    apdu_hex: String,
}

#[derive(Debug, Args)]
struct SimIsoSelectArgs {
    #[command(flatten)]
    simulation: SimulationArgs,
    /// Application identifier to select.
    #[arg(long)]
    aid: String,
}

#[derive(Debug, Args)]
struct SimIsoChannelCloseArgs {
    #[command(flatten)]
    simulation: SimulationArgs,
    /// Logical channel number to close.
    #[arg(long)]
    channel: u8,
}

#[derive(Debug, Args)]
struct SimIsoSecureOpenArgs {
    #[command(flatten)]
    simulation: SimulationArgs,
    /// Secure messaging protocol: `iso7816`, `scp02`, `scp03`, or `other:<label>`.
    #[arg(long)]
    protocol: String,
    /// Security level byte.
    #[arg(long)]
    security_level: Option<u8>,
    /// Optional session identifier.
    #[arg(long)]
    session_id: Option<String>,
}

#[derive(Debug, Args)]
struct SimIsoSecureAdvanceArgs {
    #[command(flatten)]
    simulation: SimulationArgs,
    /// Counter increment, defaults to 1.
    #[arg(long, default_value_t = 1)]
    increment: u32,
}

#[derive(Debug, Args)]
struct SimGpGetStatusArgs {
    #[command(flatten)]
    simulation: SimulationArgs,
    /// Registry subset to query.
    #[arg(long, value_enum)]
    kind: GpRegistryKindArg,
    /// Whether to request the first page or a continuation page.
    #[arg(long, value_enum, default_value = "first-or-all")]
    occurrence: GpOccurrenceArg,
}

#[derive(Debug, Subcommand)]
enum SimGpAuthCommand {
    /// Open one authenticated GP secure channel.
    Open(SimGpAuthOpenArgs),
    /// Close the current authenticated GP secure channel.
    Close(SimulationArgs),
}

#[derive(Debug, Args)]
struct SimGpAuthOpenArgs {
    #[command(flatten)]
    simulation: SimulationArgs,
    /// GP keyset name. When omitted, JCIM uses `JCIM_GP_DEFAULT_KEYSET`.
    #[arg(long)]
    keyset: Option<String>,
    /// GP security level byte. Defaults to `0x01` when omitted.
    #[arg(long)]
    security_level: Option<u8>,
}

#[derive(Debug, Args)]
struct SimGpSetCardStatusArgs {
    #[command(flatten)]
    simulation: SimulationArgs,
    /// Target card life cycle state.
    #[arg(long, value_enum)]
    state: GpCardStateArg,
}

#[derive(Debug, Args)]
struct SimGpSetTargetStatusArgs {
    #[command(flatten)]
    simulation: SimulationArgs,
    /// Target application or security-domain AID.
    #[arg(long)]
    aid: String,
    /// Lock transition to apply.
    #[arg(long, value_enum)]
    transition: GpTransitionArg,
}

#[derive(Debug, Subcommand)]
enum CardCommand {
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
        #[command(subcommand)]
        command: CardIsoCommand,
    },
    /// Run typed GlobalPlatform administration workflows against a physical card.
    Gp {
        #[command(subcommand)]
        command: CardGpCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CardIsoCommand {
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

#[derive(Debug, Subcommand)]
enum CardGpCommand {
    /// Open or close one authenticated GP secure channel.
    Auth {
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

#[derive(Debug, Args)]
struct CardReaderArgs {
    /// Physical reader name.
    #[arg(long)]
    reader: Option<String>,
}

#[derive(Debug, Args)]
struct CardInstallArgs {
    #[command(flatten)]
    project: ProjectSelectorArgs,
    /// Physical reader name.
    #[arg(long)]
    reader: Option<String>,
    /// Explicit CAP path. When omitted, JCIM uses the project CAP artifact.
    #[arg(long)]
    cap: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CardDeleteArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// Package AID to delete.
    aid: String,
}

#[derive(Debug, Args)]
struct CardApduArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// Raw APDU in hexadecimal.
    apdu_hex: String,
}

#[derive(Debug, Args)]
struct CardIsoSelectArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// Application identifier to select.
    #[arg(long)]
    aid: String,
}

#[derive(Debug, Args)]
struct CardIsoChannelCloseArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// Logical channel number to close.
    #[arg(long)]
    channel: u8,
}

#[derive(Debug, Args)]
struct CardIsoSecureOpenArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// Secure messaging protocol: `iso7816`, `scp02`, `scp03`, or `other:<label>`.
    #[arg(long)]
    protocol: String,
    /// Security level byte.
    #[arg(long)]
    security_level: Option<u8>,
    /// Optional session identifier.
    #[arg(long)]
    session_id: Option<String>,
}

#[derive(Debug, Args)]
struct CardIsoSecureAdvanceArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// Counter increment, defaults to 1.
    #[arg(long, default_value_t = 1)]
    increment: u32,
}

#[derive(Debug, Args)]
struct CardGpGetStatusArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// Registry subset to query.
    #[arg(long, value_enum)]
    kind: GpRegistryKindArg,
    /// Whether to request the first page or a continuation page.
    #[arg(long, value_enum, default_value = "first-or-all")]
    occurrence: GpOccurrenceArg,
}

#[derive(Debug, Subcommand)]
enum CardGpAuthCommand {
    /// Open one authenticated GP secure channel.
    Open(CardGpAuthOpenArgs),
    /// Close the current authenticated GP secure channel.
    Close(CardReaderArgs),
}

#[derive(Debug, Args)]
struct CardGpAuthOpenArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// GP keyset name. When omitted, JCIM uses `JCIM_GP_DEFAULT_KEYSET`.
    #[arg(long)]
    keyset: Option<String>,
    /// GP security level byte. Defaults to `0x01` when omitted.
    #[arg(long)]
    security_level: Option<u8>,
}

#[derive(Debug, Args)]
struct CardGpSetCardStatusArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// Target card life cycle state.
    #[arg(long, value_enum)]
    state: GpCardStateArg,
}

#[derive(Debug, Args)]
struct CardGpSetTargetStatusArgs {
    #[command(flatten)]
    reader: CardReaderArgs,
    /// Target application or security-domain AID.
    #[arg(long)]
    aid: String,
    /// Lock transition to apply.
    #[arg(long, value_enum)]
    transition: GpTransitionArg,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GpRegistryKindArg {
    Isd,
    Applications,
    LoadFiles,
    LoadFilesAndModules,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GpOccurrenceArg {
    FirstOrAll,
    Next,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GpCardStateArg {
    OpReady,
    Initialized,
    Secured,
    CardLocked,
    Terminated,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GpTransitionArg {
    Lock,
    Unlock,
}

#[derive(Debug, Subcommand)]
enum SystemCommand {
    /// Persist machine-local toolchain settings.
    Setup(SystemSetupArgs),
    /// Show a doctor report for the local environment.
    Doctor,
    /// Show local service status without starting it.
    Service {
        #[command(subcommand)]
        command: SystemServiceCommand,
    },
}

#[derive(Debug, Args)]
struct SystemSetupArgs {
    /// Override the Java executable used by JCIM-managed tools.
    #[arg(long)]
    java_bin: Option<String>,
}

#[derive(Debug, Subcommand)]
enum SystemServiceCommand {
    /// Show the current local service socket and status.
    Status,
}

async fn run_project(command: ProjectCommand, json_mode: bool) -> Result<(), String> {
    let client = JcimClient::connect_or_start()
        .await
        .map_err(|error| error.to_string())?;
    match command {
        ProjectCommand::New(args) => {
            let directory = args.directory.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(&args.name)
            });
            let project = client
                .create_project(&args.name, &directory)
                .await
                .map_err(|error| error.to_string())?;
            print_project_summary(&project, json_mode);
        }
        ProjectCommand::Show(args) => {
            let project = resolve_project_ref(args)?;
            let details = client
                .get_project(&project)
                .await
                .map_err(|error| error.to_string())?;
            print_project_details(&details, json_mode);
        }
        ProjectCommand::Clean(args) => {
            let cleaned_path = client
                .clean_project(&resolve_project_ref(args)?)
                .await
                .map_err(|error| error.to_string())?;
            if json_mode {
                println!("{}", json!({ "cleaned_path": cleaned_path }));
            } else {
                println!("Cleaned: {}", cleaned_path.display());
            }
        }
    }
    Ok(())
}

async fn run_build(command: BuildCommand, json_mode: bool) -> Result<(), String> {
    let client = JcimClient::connect_or_start()
        .await
        .map_err(|error| error.to_string())?;
    match command.command {
        Some(BuildSubcommand::Artifacts(args)) => {
            let project_ref = resolve_project_ref(args)?;
            let project = client
                .get_project(&project_ref)
                .await
                .map_err(|error| error.to_string())?
                .project;
            let artifacts = client
                .get_artifacts(&project_ref)
                .await
                .map_err(|error| error.to_string())?;
            print_build_summary(
                &BuildSummary {
                    project,
                    artifacts,
                    rebuilt: false,
                },
                false,
                json_mode,
            );
        }
        None => {
            let summary = client
                .build_project(&resolve_project_ref(command.project)?)
                .await
                .map_err(|error| error.to_string())?;
            print_build_summary(&summary, true, json_mode);
        }
    }
    Ok(())
}

async fn run_sim(command: SimCommand, json_mode: bool) -> Result<(), String> {
    let client = JcimClient::connect_or_start()
        .await
        .map_err(|error| error.to_string())?;
    match command {
        SimCommand::Start(args) => {
            if args.cap.is_some() && (args.project.project.is_some() || args.project.id.is_some()) {
                return Err("pass either `--cap` or a project selector, not both".to_string());
            }
            let input = if let Some(cap_path) = args.cap {
                SimulationInput::Cap(cap_path)
            } else {
                SimulationInput::Project(resolve_project_ref(args.project)?)
            };
            let simulation = client
                .start_simulation(input)
                .await
                .map_err(|error| error.to_string())?;
            print_simulation(&simulation, json_mode);
        }
        SimCommand::Stop(args) => {
            let simulation = client
                .stop_simulation(resolve_simulation_ref(&client, args.simulation).await?)
                .await
                .map_err(|error| error.to_string())?;
            print_simulation(&simulation, json_mode);
        }
        SimCommand::Status(args) => {
            if let Some(simulation_id) = args.simulation {
                let simulation = client
                    .get_simulation(SimulationRef::new(simulation_id))
                    .await
                    .map_err(|error| error.to_string())?;
                print_simulation(&simulation, json_mode);
            } else {
                let simulations = client
                    .list_simulations()
                    .await
                    .map_err(|error| error.to_string())?;
                if json_mode {
                    println!("{}", json!({ "simulations": simulations }));
                } else if simulations.is_empty() {
                    println!("No active simulations.");
                } else {
                    for simulation in simulations {
                        print_simulation_human(&simulation);
                        println!();
                    }
                }
            }
        }
        SimCommand::Logs(args) => {
            let events = client
                .simulation_events(resolve_simulation_ref(&client, args.simulation).await?)
                .await
                .map_err(|error| error.to_string())?;
            if json_mode {
                println!("{}", json!({ "events": events }));
            } else {
                for event in events {
                    println!("[{}] {}", event.level, event.message);
                }
            }
        }
        SimCommand::Apdu(args) => {
            let apdu = parse_command_apdu(&args.apdu_hex)?;
            let response = client
                .transmit_sim_apdu(
                    resolve_simulation_ref(&client, args.simulation.simulation).await?,
                    &apdu,
                )
                .await
                .map_err(|error| error.to_string())?;
            print_apdu_response(&response, json_mode);
        }
        SimCommand::Reset(args) => {
            let atr_hex = client
                .reset_simulation(resolve_simulation_ref(&client, args.simulation).await?)
                .await
                .map_err(|error| error.to_string())?;
            if json_mode {
                println!("{}", json!({ "atr_hex": atr_hex }));
            } else {
                println!("{atr_hex}");
            }
        }
        SimCommand::Iso { command } => match command {
            SimIsoCommand::Status(args) => {
                let session_state = client
                    .get_simulation_session_state(
                        resolve_simulation_ref(&client, args.simulation).await?,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_iso_session_state(&session_state, json_mode);
            }
            SimIsoCommand::Select(args) => {
                let response = client
                    .iso_select_application_on_simulation(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        &parse_aid(&args.aid)?,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
            SimIsoCommand::ChannelOpen(args) => {
                let summary = client
                    .manage_simulation_channel(
                        resolve_simulation_ref(&client, args.simulation).await?,
                        true,
                        None,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_manage_channel_summary(&summary, json_mode);
            }
            SimIsoCommand::ChannelClose(args) => {
                let summary = client
                    .manage_simulation_channel(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        false,
                        Some(args.channel),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_manage_channel_summary(&summary, json_mode);
            }
            SimIsoCommand::SecureOpen(args) => {
                let summary = client
                    .open_simulation_secure_messaging(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        Some(parse_secure_messaging_protocol(&args.protocol)?),
                        args.security_level,
                        args.session_id,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_secure_messaging_summary(&summary, json_mode);
            }
            SimIsoCommand::SecureAdvance(args) => {
                let summary = client
                    .advance_simulation_secure_messaging(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        args.increment,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_secure_messaging_summary(&summary, json_mode);
            }
            SimIsoCommand::SecureClose(args) => {
                let summary = client
                    .close_simulation_secure_messaging(
                        resolve_simulation_ref(&client, args.simulation).await?,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_secure_messaging_summary(&summary, json_mode);
            }
        },
        SimCommand::Gp { command } => match command {
            SimGpCommand::Auth { command } => match command {
                SimGpAuthCommand::Open(args) => {
                    let summary = client
                        .open_gp_secure_channel_on_simulation(
                            resolve_simulation_ref(&client, args.simulation.simulation).await?,
                            args.keyset.as_deref(),
                            args.security_level,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    print_gp_secure_channel_summary(&summary, json_mode);
                }
                SimGpAuthCommand::Close(args) => {
                    let summary = client
                        .close_gp_secure_channel_on_simulation(
                            resolve_simulation_ref(&client, args.simulation).await?,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    print_secure_messaging_summary(&summary, json_mode);
                }
            },
            SimGpCommand::SelectIsd(args) => {
                let response = client
                    .gp_select_issuer_security_domain_on_simulation(
                        resolve_simulation_ref(&client, args.simulation).await?,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
            SimGpCommand::GetStatus(args) => {
                let response = client
                    .gp_get_status_on_simulation(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        gp_registry_kind(args.kind),
                        gp_occurrence(args.occurrence),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_gp_status_response(&response, json_mode);
            }
            SimGpCommand::SetCardStatus(args) => {
                let response = client
                    .gp_set_card_status_on_simulation(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        gp_card_state(args.state),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
            SimGpCommand::SetApplicationStatus(args) => {
                let response = client
                    .gp_set_application_status_on_simulation(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        &parse_aid(&args.aid)?,
                        gp_transition(args.transition),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
            SimGpCommand::SetSecurityDomainStatus(args) => {
                let response = client
                    .gp_set_security_domain_status_on_simulation(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        &parse_aid(&args.aid)?,
                        gp_transition(args.transition),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
        },
    }
    Ok(())
}

async fn run_card(command: CardCommand, json_mode: bool) -> Result<(), String> {
    let client = JcimClient::connect_or_start()
        .await
        .map_err(|error| error.to_string())?;
    match command {
        CardCommand::Readers => {
            let readers = client
                .list_readers()
                .await
                .map_err(|error| error.to_string())?;
            if json_mode {
                println!("{}", json!({ "readers": readers }));
            } else if readers.is_empty() {
                println!("No PC/SC readers found.");
            } else {
                for reader in readers {
                    println!(
                        "{}\t{}",
                        reader.name,
                        if reader.card_present {
                            "present"
                        } else {
                            "empty"
                        }
                    );
                }
            }
        }
        CardCommand::Status(args) => {
            let status = client
                .get_card_status_on(reader_ref(args.reader))
                .await
                .map_err(|error| error.to_string())?;
            if json_mode {
                println!("{}", json!(status));
            } else {
                for line in status.lines {
                    println!("{line}");
                }
            }
        }
        CardCommand::Install(args) => {
            if args.cap.is_some() && (args.project.project.is_some() || args.project.id.is_some()) {
                return Err("pass either `--cap` or a project selector, not both".to_string());
            }
            let source = if let Some(cap_path) = args.cap {
                CardInstallSource::Cap(cap_path)
            } else {
                CardInstallSource::Project(resolve_project_ref(args.project)?)
            };
            let summary = client
                .install_cap_on(source, reader_ref(args.reader))
                .await
                .map_err(|error| error.to_string())?;
            print_card_install(&summary, json_mode);
        }
        CardCommand::Delete(args) => {
            let summary = client
                .delete_item_on(&args.aid, reader_ref(args.reader.reader))
                .await
                .map_err(|error| error.to_string())?;
            print_card_delete(&summary, json_mode);
        }
        CardCommand::Packages(args) => {
            let inventory = client
                .list_packages_on(reader_ref(args.reader))
                .await
                .map_err(|error| error.to_string())?;
            print_package_inventory(&inventory, json_mode);
        }
        CardCommand::Applets(args) => {
            let inventory = client
                .list_applets_on(reader_ref(args.reader))
                .await
                .map_err(|error| error.to_string())?;
            print_applet_inventory(&inventory, json_mode);
        }
        CardCommand::Apdu(args) => {
            let response = client
                .transmit_card_apdu_on(
                    &parse_command_apdu(&args.apdu_hex)?,
                    reader_ref(args.reader.reader),
                )
                .await
                .map_err(|error| error.to_string())?;
            print_apdu_response(&response, json_mode);
        }
        CardCommand::Reset(args) => {
            let atr_hex = client
                .reset_card_on(reader_ref(args.reader))
                .await
                .map_err(|error| error.to_string())?;
            if json_mode {
                println!("{}", json!({ "atr_hex": atr_hex }));
            } else {
                println!("{atr_hex}");
            }
        }
        CardCommand::Iso { command } => match command {
            CardIsoCommand::Status(args) => {
                let session_state = client
                    .get_card_session_state_on(reader_ref(args.reader))
                    .await
                    .map_err(|error| error.to_string())?;
                print_iso_session_state(&session_state, json_mode);
            }
            CardIsoCommand::Select(args) => {
                let response = client
                    .iso_select_application_on_card_with_reader(
                        &parse_aid(&args.aid)?,
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
            CardIsoCommand::ChannelOpen(args) => {
                let summary = client
                    .manage_card_channel_on(true, None, reader_ref(args.reader))
                    .await
                    .map_err(|error| error.to_string())?;
                print_manage_channel_summary(&summary, json_mode);
            }
            CardIsoCommand::ChannelClose(args) => {
                let summary = client
                    .manage_card_channel_on(
                        false,
                        Some(args.channel),
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_manage_channel_summary(&summary, json_mode);
            }
            CardIsoCommand::SecureOpen(args) => {
                let summary = client
                    .open_card_secure_messaging_on(
                        Some(parse_secure_messaging_protocol(&args.protocol)?),
                        args.security_level,
                        args.session_id,
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_secure_messaging_summary(&summary, json_mode);
            }
            CardIsoCommand::SecureAdvance(args) => {
                let summary = client
                    .advance_card_secure_messaging_on(
                        args.increment,
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_secure_messaging_summary(&summary, json_mode);
            }
            CardIsoCommand::SecureClose(args) => {
                let summary = client
                    .close_card_secure_messaging_on(reader_ref(args.reader))
                    .await
                    .map_err(|error| error.to_string())?;
                print_secure_messaging_summary(&summary, json_mode);
            }
        },
        CardCommand::Gp { command } => match command {
            CardGpCommand::Auth { command } => match command {
                CardGpAuthCommand::Open(args) => {
                    let summary = client
                        .open_gp_secure_channel_on_card_with_reader(
                            args.keyset.as_deref(),
                            args.security_level,
                            reader_ref(args.reader.reader),
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    print_gp_secure_channel_summary(&summary, json_mode);
                }
                CardGpAuthCommand::Close(args) => {
                    let summary = client
                        .close_gp_secure_channel_on_card_with_reader(reader_ref(args.reader))
                        .await
                        .map_err(|error| error.to_string())?;
                    print_secure_messaging_summary(&summary, json_mode);
                }
            },
            CardGpCommand::SelectIsd(args) => {
                let response = client
                    .gp_select_issuer_security_domain_on_card_with_reader(reader_ref(args.reader))
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
            CardGpCommand::GetStatus(args) => {
                let response = client
                    .gp_get_status_on_card_with_reader(
                        gp_registry_kind(args.kind),
                        gp_occurrence(args.occurrence),
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_gp_status_response(&response, json_mode);
            }
            CardGpCommand::SetCardStatus(args) => {
                let response = client
                    .gp_set_card_status_on_card_with_reader(
                        gp_card_state(args.state),
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
            CardGpCommand::SetApplicationStatus(args) => {
                let response = client
                    .gp_set_application_status_on_card_with_reader(
                        &parse_aid(&args.aid)?,
                        gp_transition(args.transition),
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
            CardGpCommand::SetSecurityDomainStatus(args) => {
                let response = client
                    .gp_set_security_domain_status_on_card_with_reader(
                        &parse_aid(&args.aid)?,
                        gp_transition(args.transition),
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                print_apdu_response(&response, json_mode);
            }
        },
    }
    Ok(())
}

async fn run_system(command: SystemCommand, json_mode: bool) -> Result<(), String> {
    match command {
        SystemCommand::Setup(args) => {
            let client = JcimClient::connect_or_start()
                .await
                .map_err(|error| error.to_string())?;
            let setup = client
                .setup_toolchains(args.java_bin.as_deref())
                .await
                .map_err(|error| error.to_string())?;
            if json_mode {
                println!("{}", json!(setup));
            } else {
                println!("{}", setup.message);
            }
        }
        SystemCommand::Doctor => {
            let client = JcimClient::connect_or_start()
                .await
                .map_err(|error| error.to_string())?;
            let lines = client.doctor().await.map_err(|error| error.to_string())?;
            if json_mode {
                println!("{}", json!({ "lines": lines }));
            } else {
                for line in lines {
                    println!("{line}");
                }
            }
        }
        SystemCommand::Service {
            command: SystemServiceCommand::Status,
        } => {
            let managed_paths = jcim_config::project::ManagedPaths::discover()
                .map_err(|error| error.to_string())?;
            let status = match JcimClient::connect_with_paths(managed_paths.clone()).await {
                Ok(client) => client
                    .service_status()
                    .await
                    .map_err(|error| error.to_string())?,
                Err(_) => ServiceStatusSummary {
                    socket_path: managed_paths.service_socket_path,
                    running: false,
                    known_project_count: 0,
                    active_simulation_count: 0,
                },
            };
            print_service_status(&status, json_mode);
        }
    }
    Ok(())
}

fn resolve_project_ref(args: ProjectSelectorArgs) -> Result<ProjectRef, String> {
    if let Some(project_id) = args.id {
        return Ok(ProjectRef::from_id(project_id));
    }
    if let Some(project) = args.project {
        return Ok(ProjectRef::from_path(project));
    }
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    if find_project_manifest(&cwd).is_some() {
        Ok(ProjectRef::from_path(cwd))
    } else {
        Err(
            "no project selected; pass `--project`, `--id`, or run inside a JCIM project"
                .to_string(),
        )
    }
}

async fn resolve_simulation_ref(
    client: &JcimClient,
    requested: Option<String>,
) -> Result<SimulationRef, String> {
    if let Some(simulation_id) = requested {
        return Ok(SimulationRef::new(simulation_id));
    }
    let simulations = client
        .list_simulations()
        .await
        .map_err(|error| error.to_string())?;
    match simulations.as_slice() {
        [simulation] => Ok(simulation.simulation_ref()),
        [] => Err("no active simulations are available".to_string()),
        _ => Err("multiple simulations are active; pass `--simulation`".to_string()),
    }
}

fn reader_ref(reader: Option<String>) -> ReaderRef {
    match reader {
        Some(reader) => ReaderRef::named(reader),
        None => ReaderRef::Default,
    }
}

fn parse_command_apdu(apdu_hex: &str) -> Result<CommandApdu, String> {
    let bytes = hex::decode(apdu_hex).map_err(|error| error.to_string())?;
    CommandApdu::parse(&bytes).map_err(|error| error.to_string())
}

fn parse_aid(aid: &str) -> Result<Aid, String> {
    Aid::from_hex(aid).map_err(|error| error.to_string())
}

fn parse_secure_messaging_protocol(value: &str) -> Result<SecureMessagingProtocol, String> {
    let normalized = value.trim();
    match normalized.to_ascii_lowercase().as_str() {
        "iso7816" | "iso-7816" | "iso_7816" => Ok(SecureMessagingProtocol::Iso7816),
        "scp02" => Ok(SecureMessagingProtocol::Scp02),
        "scp03" => Ok(SecureMessagingProtocol::Scp03),
        _ => normalized
            .strip_prefix("other:")
            .or_else(|| normalized.strip_prefix("OTHER:"))
            .map(|label| SecureMessagingProtocol::Other(label.to_string()))
            .ok_or_else(|| {
                "unsupported secure messaging protocol; use `iso7816`, `scp02`, `scp03`, or `other:<label>`".to_string()
            }),
    }
}

fn gp_registry_kind(value: GpRegistryKindArg) -> globalplatform::RegistryKind {
    match value {
        GpRegistryKindArg::Isd => globalplatform::RegistryKind::IssuerSecurityDomain,
        GpRegistryKindArg::Applications => globalplatform::RegistryKind::Applications,
        GpRegistryKindArg::LoadFiles => globalplatform::RegistryKind::ExecutableLoadFiles,
        GpRegistryKindArg::LoadFilesAndModules => {
            globalplatform::RegistryKind::ExecutableLoadFilesAndModules
        }
    }
}

fn gp_occurrence(value: GpOccurrenceArg) -> globalplatform::GetStatusOccurrence {
    match value {
        GpOccurrenceArg::FirstOrAll => globalplatform::GetStatusOccurrence::FirstOrAll,
        GpOccurrenceArg::Next => globalplatform::GetStatusOccurrence::Next,
    }
}

fn gp_card_state(value: GpCardStateArg) -> globalplatform::CardLifeCycle {
    match value {
        GpCardStateArg::OpReady => globalplatform::CardLifeCycle::OpReady,
        GpCardStateArg::Initialized => globalplatform::CardLifeCycle::Initialized,
        GpCardStateArg::Secured => globalplatform::CardLifeCycle::Secured,
        GpCardStateArg::CardLocked => globalplatform::CardLifeCycle::CardLocked,
        GpCardStateArg::Terminated => globalplatform::CardLifeCycle::Terminated,
    }
}

fn gp_transition(value: GpTransitionArg) -> globalplatform::LockTransition {
    match value {
        GpTransitionArg::Lock => globalplatform::LockTransition::Lock,
        GpTransitionArg::Unlock => globalplatform::LockTransition::Unlock,
    }
}

fn print_project_summary(project: &jcim_sdk::ProjectSummary, json_mode: bool) {
    if json_mode {
        println!("{}", json!(project));
    } else {
        print_project_human(project);
    }
}

fn print_project_details(details: &ProjectDetails, json_mode: bool) {
    if json_mode {
        println!("{}", json!(details));
    } else {
        print_project_human(&details.project);
        println!();
        println!("{}", details.manifest_toml);
    }
}

fn print_project_human(project: &jcim_sdk::ProjectSummary) {
    println!("Project: {}", project.name);
    println!("Project ID: {}", project.project_id);
    println!("Path: {}", project.project_path.display());
    println!("Profile: {}", project.profile);
    println!("Build: {}", project.build_kind);
    println!(
        "Package: {} ({})",
        project.package_name, project.package_aid
    );
    if !project.applets.is_empty() {
        println!("Applets:");
        for applet in &project.applets {
            println!("  {} ({})", applet.class_name, applet.aid);
        }
    }
}

fn print_build_summary(summary: &BuildSummary, show_rebuilt: bool, json_mode: bool) {
    if json_mode {
        println!("{}", json!(summary));
    } else {
        print_project_human(&summary.project);
        if show_rebuilt {
            println!("Rebuilt: {}", if summary.rebuilt { "yes" } else { "no" });
        }
        println!("Artifacts:");
        for artifact in &summary.artifacts {
            println!("  {}: {}", artifact.kind, artifact.path.display());
        }
    }
}

fn print_simulation(simulation: &SimulationSummary, json_mode: bool) {
    if json_mode {
        println!("{}", json!(simulation));
    } else {
        print_simulation_human(simulation);
    }
}

fn print_simulation_human(simulation: &SimulationSummary) {
    println!("Simulation: {}", simulation.simulation_id);
    println!("Source: {}", simulation_source_name(simulation.source_kind));
    if let Some(project_id) = &simulation.project_id {
        println!("Project ID: {project_id}");
    }
    if let Some(project_path) = &simulation.project_path {
        println!("Project Path: {}", project_path.display());
    }
    println!("CAP: {}", simulation.cap_path.display());
    println!("Engine: {}", simulation_engine_name(simulation.engine_mode));
    println!("Status: {}", simulation_status_name(simulation.status));
    println!("Reader: {}", simulation.reader_name);
    println!("Health: {}", simulation.health);
    if let Some(atr) = &simulation.atr {
        println!("ATR: {}", atr.to_hex());
    }
    if let Some(protocol) = &simulation.active_protocol
        && let Some(active) = protocol.protocol
    {
        println!("Protocol: {active}");
    }
    println!(
        "Installed package: {} ({})",
        simulation.package_name, simulation.package_aid
    );
    println!(
        "Packages/applets: {}/{}",
        simulation.package_count, simulation.applet_count
    );
    if !simulation.recent_events.is_empty() {
        println!("Events:");
        for event in &simulation.recent_events {
            println!("  {event}");
        }
    }
}

fn print_card_install(summary: &CardInstallSummary, json_mode: bool) {
    if json_mode {
        println!("{}", json!(summary));
    } else {
        println!("Reader: {}", summary.reader_name);
        println!("CAP: {}", summary.cap_path.display());
        println!(
            "Installed package: {} ({})",
            summary.package_name, summary.package_aid
        );
        if !summary.applets.is_empty() {
            println!("Applets:");
            for applet in &summary.applets {
                println!("  {} ({})", applet.class_name, applet.aid);
            }
        }
        print_output_lines(&summary.output_lines);
    }
}

fn print_card_delete(summary: &CardDeleteSummary, json_mode: bool) {
    if json_mode {
        println!("{}", json!(summary));
    } else {
        println!("Reader: {}", summary.reader_name);
        println!("Deleted: {}", summary.aid);
        print_output_lines(&summary.output_lines);
    }
}

fn print_package_inventory(inventory: &CardPackageInventory, json_mode: bool) {
    if json_mode {
        println!("{}", json!(inventory));
    } else if inventory.packages.is_empty() {
        print_output_lines(&inventory.output_lines);
    } else {
        println!("Reader: {}", inventory.reader_name);
        for package in &inventory.packages {
            if package.description.is_empty() {
                println!("{}", package.aid);
            } else {
                println!("{} {}", package.aid, package.description);
            }
        }
    }
}

fn print_applet_inventory(inventory: &CardAppletInventory, json_mode: bool) {
    if json_mode {
        println!("{}", json!(inventory));
    } else if inventory.applets.is_empty() {
        print_output_lines(&inventory.output_lines);
    } else {
        println!("Reader: {}", inventory.reader_name);
        for applet in &inventory.applets {
            if applet.description.is_empty() {
                println!("{}", applet.aid);
            } else {
                println!("{} {}", applet.aid, applet.description);
            }
        }
    }
}

fn print_output_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

fn print_iso_session_state(state: &IsoSessionState, json_mode: bool) {
    if json_mode {
        println!("{}", json!(state));
        return;
    }

    println!(
        "Power: {}",
        match state.power_state {
            jcim_sdk::iso7816::PowerState::Off => "off",
            jcim_sdk::iso7816::PowerState::On => "on",
        }
    );
    if let Some(atr) = &state.atr {
        println!("ATR: {}", atr.to_hex());
    }
    if let Some(protocol) = &state.active_protocol
        && let Some(active) = protocol.protocol
    {
        println!("Protocol: {active}");
    }
    if let Some(aid) = &state.selected_aid {
        println!("Selected AID: {}", aid.to_hex());
    }
    if let Some(selection) = &state.current_file {
        println!("Current file: {}", file_selection_label(selection));
    }
    if !state.open_channels.is_empty() {
        println!("Channels:");
        for channel in &state.open_channels {
            let selected = channel
                .selected_aid
                .as_ref()
                .map(|aid| aid.to_hex())
                .unwrap_or_else(|| "-".to_string());
            let current_file = channel
                .current_file
                .as_ref()
                .map(file_selection_label)
                .unwrap_or_else(|| "-".to_string());
            println!(
                "  {}  selected={} file={}",
                channel.channel_number, selected, current_file
            );
        }
    }
    if state.secure_messaging.active {
        let protocol = match &state.secure_messaging.protocol {
            Some(SecureMessagingProtocol::Iso7816) => "iso7816".to_string(),
            Some(SecureMessagingProtocol::Scp02) => "scp02".to_string(),
            Some(SecureMessagingProtocol::Scp03) => "scp03".to_string(),
            Some(SecureMessagingProtocol::Other(label)) => format!("other:{label}"),
            None => "unknown".to_string(),
        };
        println!(
            "Secure messaging: active protocol={} counter={}",
            protocol, state.secure_messaging.command_counter
        );
        if let Some(level) = state.secure_messaging.security_level {
            println!("Security level: {level:02X}");
        }
        if let Some(session_id) = &state.secure_messaging.session_id {
            println!("Session ID: {session_id}");
        }
    } else {
        println!("Secure messaging: inactive");
    }
    if !state.verified_references.is_empty() {
        println!(
            "Verified references: {}",
            state
                .verified_references
                .iter()
                .map(|value| format!("{value:02X}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !state.retry_counters.is_empty() {
        println!("Retry counters:");
        for counter in &state.retry_counters {
            println!("  {:02X}: {}", counter.reference, counter.remaining);
        }
    }
    if let Some(status) = state.last_status {
        println!("Last status: {status} ({})", status.label());
    }
}

fn print_manage_channel_summary(summary: &ManageChannelSummary, json_mode: bool) {
    if json_mode {
        println!("{}", json!(summary));
        return;
    }

    if let Some(channel_number) = summary.channel_number {
        println!("Channel: {channel_number}");
    }
    print_apdu_response(&summary.response, false);
    println!();
    print_iso_session_state(&summary.session_state, false);
}

fn print_secure_messaging_summary(summary: &SecureMessagingSummary, json_mode: bool) {
    if json_mode {
        println!("{}", json!(summary));
        return;
    }
    print_iso_session_state(&summary.session_state, false);
}

fn print_gp_secure_channel_summary(summary: &GpSecureChannelSummary, json_mode: bool) {
    if json_mode {
        println!("{}", json!(summary));
        return;
    }

    println!("Keyset: {}", summary.secure_channel.keyset.name);
    println!(
        "Protocol: {}",
        match summary.secure_channel.keyset.mode {
            globalplatform::ScpMode::Scp02 => "scp02",
            globalplatform::ScpMode::Scp03 => "scp03",
        }
    );
    println!(
        "Security level: {:02X}",
        summary.secure_channel.security_level.as_byte()
    );
    println!("Session ID: {}", summary.secure_channel.session_id);
    println!("Selected AID: {}", summary.selected_aid.to_hex());
    println!();
    print_iso_session_state(&summary.session_state, false);
}

fn print_gp_status_response(response: &globalplatform::GetStatusResponse, json_mode: bool) {
    if json_mode {
        println!("{}", json!(response));
        return;
    }

    println!(
        "Registry: {}",
        match response.kind {
            globalplatform::RegistryKind::IssuerSecurityDomain => "issuer-security-domain",
            globalplatform::RegistryKind::Applications => "applications",
            globalplatform::RegistryKind::ExecutableLoadFiles => "load-files",
            globalplatform::RegistryKind::ExecutableLoadFilesAndModules => "load-files-and-modules",
        }
    );
    println!(
        "More data available: {}",
        if response.more_data_available {
            "yes"
        } else {
            "no"
        }
    );
    if response.entries.is_empty() {
        println!("Entries: none");
        return;
    }
    println!("Entries:");
    for entry in &response.entries {
        println!("  AID: {}", entry.aid.to_hex());
        println!("  Life cycle: {:02X}", entry.life_cycle_state);
        if let Some(privileges) = entry.privileges {
            println!(
                "  Privileges: {}",
                hex::encode_upper([privileges[0], privileges[1], privileges[2]])
            );
        }
        if let Some(aid) = &entry.executable_load_file_aid {
            println!("  Load file: {}", aid.to_hex());
        }
        if let Some(aid) = &entry.associated_security_domain_aid {
            println!("  Associated SD: {}", aid.to_hex());
        }
        if !entry.executable_module_aids.is_empty() {
            println!(
                "  Modules: {}",
                entry
                    .executable_module_aids
                    .iter()
                    .map(|aid| aid.to_hex())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if let Some(version) = &entry.load_file_version {
            println!("  Version: {}", hex::encode_upper(version));
        }
        if !entry.implicit_selection_parameters.is_empty() {
            println!(
                "  Implicit selection: {}",
                hex::encode_upper(&entry.implicit_selection_parameters)
            );
        }
        println!();
    }
}

fn file_selection_label(selection: &FileSelection) -> String {
    match selection {
        FileSelection::ByName(bytes) => format!("name:{}", hex::encode_upper(bytes)),
        FileSelection::FileId(file_id) => format!("fid:{file_id:04X}"),
        FileSelection::Path(path) => format!("path:{}", hex::encode_upper(path)),
    }
}

fn print_apdu_response(response: &jcim_core::apdu::ResponseApdu, json_mode: bool) {
    let response_hex = hex::encode_upper(response.to_bytes());
    if json_mode {
        println!(
            "{}",
            json!({
                "response_hex": response_hex,
                "status_word": format!("{:04X}", response.sw),
                "data_hex": hex::encode_upper(&response.data),
            })
        );
    } else {
        println!("{response_hex}");
    }
}

fn print_service_status(response: &ServiceStatusSummary, json_mode: bool) {
    if json_mode {
        println!("{}", json!(response));
    } else {
        println!("Socket: {}", response.socket_path.display());
        println!("Running: {}", if response.running { "yes" } else { "no" });
        println!("Known projects: {}", response.known_project_count);
        println!("Active simulations: {}", response.active_simulation_count);
    }
}

fn simulation_source_name(kind: SimulationSourceKind) -> &'static str {
    match kind {
        SimulationSourceKind::Project => "project",
        SimulationSourceKind::Cap => "cap",
        SimulationSourceKind::Unknown => "unknown",
    }
}

fn simulation_engine_name(mode: jcim_sdk::SimulationEngineMode) -> &'static str {
    match mode {
        jcim_sdk::SimulationEngineMode::Native => "native",
        jcim_sdk::SimulationEngineMode::Container => "container",
        jcim_sdk::SimulationEngineMode::Unknown => "unknown",
    }
}

fn simulation_status_name(status: SimulationStatus) -> &'static str {
    match status {
        SimulationStatus::Starting => "starting",
        SimulationStatus::Running => "running",
        SimulationStatus::Stopped => "stopped",
        SimulationStatus::Failed => "failed",
        SimulationStatus::Unknown => "unknown",
    }
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
                assert!(args.cap.is_none());
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn sim_start_accepts_raw_cap_input() {
        let cli =
            Cli::try_parse_from(["jcim", "sim", "start", "--cap", "card.cap"]).expect("parse cap");
        match cli.command {
            Command::Sim {
                command: SimCommand::Start(args),
            } => {
                assert_eq!(
                    args.cap.expect("cap path"),
                    std::path::PathBuf::from("card.cap")
                );
                assert!(args.project.project.is_none());
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
