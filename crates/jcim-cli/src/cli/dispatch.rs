use std::path::PathBuf;

use jcim_config::project::{ManagedPaths, find_project_manifest};
use jcim_core::apdu::CommandApdu;
use jcim_sdk::iso7816::SecureMessagingProtocol;
use jcim_sdk::{
    Aid, BuildSummary, CardInstallSource, JcimClient, ProjectRef, ReaderRef, ServiceStatusSummary,
    SimulationRef, globalplatform,
};
use serde_json::json;

use super::args::{
    BuildCommand, BuildSubcommand, CardCommand, CardGpAuthCommand, CardGpCommand, CardIsoCommand,
    Command, GpCardStateArg, GpOccurrenceArg, GpRegistryKindArg, GpTransitionArg, ProjectCommand,
    ProjectSelectorArgs, SimCommand, SimGpAuthCommand, SimGpCommand, SimIsoCommand, SystemCommand,
    SystemServiceCommand,
};
use super::output;

/// Dispatch one parsed top-level CLI command to its task-oriented execution path.
pub(super) async fn dispatch(command: Command, json_mode: bool) -> Result<(), String> {
    match command {
        Command::Project { command } => run_project(command, json_mode).await,
        Command::Build(command) => run_build(command, json_mode).await,
        Command::Sim { command } => run_sim(command, json_mode).await,
        Command::Card { command } => run_card(command, json_mode).await,
        Command::System { command } => run_system(command, json_mode).await,
    }
}

/// Execute one project subcommand and render its result in the requested output mode.
pub(super) async fn run_project(command: ProjectCommand, json_mode: bool) -> Result<(), String> {
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
            output::print_project_summary(&project, json_mode);
        }
        ProjectCommand::Show(args) => {
            let project = resolve_project_ref(args)?;
            let details = client
                .get_project(&project)
                .await
                .map_err(|error| error.to_string())?;
            output::print_project_details(&details, json_mode);
        }
        ProjectCommand::Clean(args) => {
            let cleaned_path = client
                .clean_project(&resolve_project_ref(args)?)
                .await
                .map_err(|error| error.to_string())?;
            if json_mode {
                output::print_json_value("project.clean", json!({ "cleaned_path": cleaned_path }));
            } else {
                println!("Cleaned: {}", cleaned_path.display());
            }
        }
    }
    Ok(())
}

/// Execute one build subcommand and render its result in the requested output mode.
pub(super) async fn run_build(command: BuildCommand, json_mode: bool) -> Result<(), String> {
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
            output::print_build_summary(
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
            output::print_build_summary(&summary, true, json_mode);
        }
    }
    Ok(())
}

/// Execute one simulation subcommand and render its result in the requested output mode.
pub(super) async fn run_sim(command: SimCommand, json_mode: bool) -> Result<(), String> {
    let client = JcimClient::connect_or_start()
        .await
        .map_err(|error| error.to_string())?;
    match command {
        SimCommand::Start(args) => {
            let simulation = client
                .start_simulation(resolve_project_ref(args.project)?)
                .await
                .map_err(|error| error.to_string())?;
            output::print_simulation(&simulation, json_mode);
        }
        SimCommand::Stop(args) => {
            let simulation = client
                .stop_simulation(resolve_simulation_ref(&client, args.simulation).await?)
                .await
                .map_err(|error| error.to_string())?;
            output::print_simulation(&simulation, json_mode);
        }
        SimCommand::Status(args) => {
            if let Some(simulation_id) = args.simulation {
                let simulation = client
                    .get_simulation(SimulationRef::new(simulation_id))
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_simulation(&simulation, json_mode);
            } else {
                let simulations = client
                    .list_simulations()
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_simulation_list(&simulations, json_mode);
            }
        }
        SimCommand::Logs(args) => {
            let events = client
                .simulation_events(resolve_simulation_ref(&client, args.simulation).await?)
                .await
                .map_err(|error| error.to_string())?;
            output::print_simulation_events(&events, json_mode);
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
            output::print_apdu_response(&response, json_mode);
        }
        SimCommand::Reset(args) => {
            let summary = client
                .reset_simulation_summary(resolve_simulation_ref(&client, args.simulation).await?)
                .await
                .map_err(|error| error.to_string())?;
            output::print_reset_summary(&summary, "simulation.reset", json_mode);
        }
        SimCommand::Iso { command } => match command {
            SimIsoCommand::Status(args) => {
                let session_state = client
                    .get_simulation_session_state(
                        resolve_simulation_ref(&client, args.simulation).await?,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_iso_session_state(&session_state, json_mode);
            }
            SimIsoCommand::Select(args) => {
                let response = client
                    .iso_select_application_on_simulation(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        &parse_aid(&args.aid)?,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_apdu_response(&response, json_mode);
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
                output::print_manage_channel_summary(&summary, json_mode);
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
                output::print_manage_channel_summary(&summary, json_mode);
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
                output::print_secure_messaging_summary(&summary, json_mode);
            }
            SimIsoCommand::SecureAdvance(args) => {
                let summary = client
                    .advance_simulation_secure_messaging(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        args.increment,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_secure_messaging_summary(&summary, json_mode);
            }
            SimIsoCommand::SecureClose(args) => {
                let summary = client
                    .close_simulation_secure_messaging(
                        resolve_simulation_ref(&client, args.simulation).await?,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_secure_messaging_summary(&summary, json_mode);
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
                    output::print_gp_secure_channel_summary(&summary, json_mode);
                }
                SimGpAuthCommand::Close(args) => {
                    let summary = client
                        .close_gp_secure_channel_on_simulation(
                            resolve_simulation_ref(&client, args.simulation).await?,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    output::print_secure_messaging_summary(&summary, json_mode);
                }
            },
            SimGpCommand::SelectIsd(args) => {
                let response = client
                    .gp_select_issuer_security_domain_on_simulation(
                        resolve_simulation_ref(&client, args.simulation).await?,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_apdu_response(&response, json_mode);
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
                output::print_gp_status_response(&response, json_mode);
            }
            SimGpCommand::SetCardStatus(args) => {
                let response = client
                    .gp_set_card_status_on_simulation(
                        resolve_simulation_ref(&client, args.simulation.simulation).await?,
                        gp_card_state(args.state),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_apdu_response(&response, json_mode);
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
                output::print_apdu_response(&response, json_mode);
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
                output::print_apdu_response(&response, json_mode);
            }
        },
    }
    Ok(())
}

/// Execute one physical-card subcommand and render its result in the requested output mode.
pub(super) async fn run_card(command: CardCommand, json_mode: bool) -> Result<(), String> {
    let client = JcimClient::connect_or_start()
        .await
        .map_err(|error| error.to_string())?;
    match command {
        CardCommand::Readers => {
            let readers = client
                .list_readers()
                .await
                .map_err(|error| error.to_string())?;
            output::print_card_readers(&readers, json_mode);
        }
        CardCommand::Status(args) => {
            let status = client
                .get_card_status_on(reader_ref(args.reader))
                .await
                .map_err(|error| error.to_string())?;
            output::print_card_status(&status, json_mode);
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
            output::print_card_install(&summary, json_mode);
        }
        CardCommand::Delete(args) => {
            let summary = client
                .delete_item_on(&args.aid, reader_ref(args.reader.reader))
                .await
                .map_err(|error| error.to_string())?;
            output::print_card_delete(&summary, json_mode);
        }
        CardCommand::Packages(args) => {
            let inventory = client
                .list_packages_on(reader_ref(args.reader))
                .await
                .map_err(|error| error.to_string())?;
            output::print_package_inventory(&inventory, json_mode);
        }
        CardCommand::Applets(args) => {
            let inventory = client
                .list_applets_on(reader_ref(args.reader))
                .await
                .map_err(|error| error.to_string())?;
            output::print_applet_inventory(&inventory, json_mode);
        }
        CardCommand::Apdu(args) => {
            let response = client
                .transmit_card_apdu_on(
                    &parse_command_apdu(&args.apdu_hex)?,
                    reader_ref(args.reader.reader),
                )
                .await
                .map_err(|error| error.to_string())?;
            output::print_apdu_response(&response, json_mode);
        }
        CardCommand::Reset(args) => {
            let summary = client
                .reset_card_summary_on(reader_ref(args.reader))
                .await
                .map_err(|error| error.to_string())?;
            output::print_reset_summary(&summary, "card.reset", json_mode);
        }
        CardCommand::Iso { command } => match command {
            CardIsoCommand::Status(args) => {
                let session_state = client
                    .get_card_session_state_on(reader_ref(args.reader))
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_iso_session_state(&session_state, json_mode);
            }
            CardIsoCommand::Select(args) => {
                let response = client
                    .iso_select_application_on_card_with_reader(
                        &parse_aid(&args.aid)?,
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_apdu_response(&response, json_mode);
            }
            CardIsoCommand::ChannelOpen(args) => {
                let summary = client
                    .manage_card_channel_on(true, None, reader_ref(args.reader))
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_manage_channel_summary(&summary, json_mode);
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
                output::print_manage_channel_summary(&summary, json_mode);
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
                output::print_secure_messaging_summary(&summary, json_mode);
            }
            CardIsoCommand::SecureAdvance(args) => {
                let summary = client
                    .advance_card_secure_messaging_on(
                        args.increment,
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_secure_messaging_summary(&summary, json_mode);
            }
            CardIsoCommand::SecureClose(args) => {
                let summary = client
                    .close_card_secure_messaging_on(reader_ref(args.reader))
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_secure_messaging_summary(&summary, json_mode);
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
                    output::print_gp_secure_channel_summary(&summary, json_mode);
                }
                CardGpAuthCommand::Close(args) => {
                    let summary = client
                        .close_gp_secure_channel_on_card_with_reader(reader_ref(args.reader))
                        .await
                        .map_err(|error| error.to_string())?;
                    output::print_secure_messaging_summary(&summary, json_mode);
                }
            },
            CardGpCommand::SelectIsd(args) => {
                let response = client
                    .gp_select_issuer_security_domain_on_card_with_reader(reader_ref(args.reader))
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_apdu_response(&response, json_mode);
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
                output::print_gp_status_response(&response, json_mode);
            }
            CardGpCommand::SetCardStatus(args) => {
                let response = client
                    .gp_set_card_status_on_card_with_reader(
                        gp_card_state(args.state),
                        reader_ref(args.reader.reader),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                output::print_apdu_response(&response, json_mode);
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
                output::print_apdu_response(&response, json_mode);
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
                output::print_apdu_response(&response, json_mode);
            }
        },
    }
    Ok(())
}

/// Execute one system subcommand and render its result in the requested output mode.
pub(super) async fn run_system(command: SystemCommand, json_mode: bool) -> Result<(), String> {
    match command {
        SystemCommand::Setup(args) => {
            let client = JcimClient::connect_or_start()
                .await
                .map_err(|error| error.to_string())?;
            let setup = client
                .setup_toolchains(args.java_bin.as_deref())
                .await
                .map_err(|error| error.to_string())?;
            output::print_setup_summary(&setup, json_mode);
        }
        SystemCommand::Doctor => {
            let client = JcimClient::connect_or_start()
                .await
                .map_err(|error| error.to_string())?;
            let lines = client.doctor().await.map_err(|error| error.to_string())?;
            output::print_doctor_lines(&lines, json_mode);
        }
        SystemCommand::Service {
            command: SystemServiceCommand::Status,
        } => {
            let managed_paths = ManagedPaths::discover().map_err(|error| error.to_string())?;
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
                    service_binary_path: PathBuf::new(),
                    service_binary_fingerprint: String::new(),
                },
            };
            output::print_service_status(&status, json_mode);
        }
    }
    Ok(())
}

/// Resolve CLI project selector arguments into the canonical SDK project selector.
fn resolve_project_ref(args: ProjectSelectorArgs) -> Result<ProjectRef, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    resolve_project_ref_from_cwd(args, &cwd)
}

/// Resolve the current working directory into a project selector when no explicit selector was given.
fn resolve_project_ref_from_cwd(
    args: ProjectSelectorArgs,
    cwd: &std::path::Path,
) -> Result<ProjectRef, String> {
    if let Some(project_id) = args.id {
        return Ok(ProjectRef::from_id(project_id));
    }
    if let Some(project) = args.project {
        return Ok(ProjectRef::from_path(project));
    }
    if find_project_manifest(cwd).is_some() {
        Ok(ProjectRef::from_path(cwd))
    } else {
        Err(
            "no project selected; pass `--project`, `--id`, or run inside a JCIM project"
                .to_string(),
        )
    }
}

/// Resolve an optional simulation id into one concrete running simulation selector.
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
    resolve_simulation_ref_from_summaries(simulations.as_slice())
}

/// Choose one simulation selector from the currently known simulation summaries.
fn resolve_simulation_ref_from_summaries(
    simulations: &[jcim_sdk::SimulationSummary],
) -> Result<SimulationRef, String> {
    match simulations {
        [simulation] => Ok(simulation.simulation_ref()),
        [] => Err("no active simulations are available".to_string()),
        _ => Err("multiple simulations are active; pass `--simulation`".to_string()),
    }
}

/// Build one SDK reader selector from an optional CLI reader name.
fn reader_ref(reader: Option<String>) -> ReaderRef {
    match reader {
        Some(reader) => ReaderRef::named(reader),
        None => ReaderRef::Default,
    }
}

/// Parse one CLI APDU hex string into the typed command APDU model.
fn parse_command_apdu(apdu_hex: &str) -> Result<CommandApdu, String> {
    let bytes = hex::decode(apdu_hex).map_err(|error| error.to_string())?;
    CommandApdu::parse(&bytes).map_err(|error| error.to_string())
}

/// Parse one CLI AID string into the typed AID model.
fn parse_aid(aid: &str) -> Result<Aid, String> {
    Aid::from_hex(aid).map_err(|error| error.to_string())
}

/// Parse one CLI secure-messaging protocol token into the typed ISO protocol model.
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

/// Map the CLI GP registry-kind enum to the maintained domain enum.
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

/// Map the CLI GP occurrence enum to the maintained domain enum.
fn gp_occurrence(value: GpOccurrenceArg) -> globalplatform::GetStatusOccurrence {
    match value {
        GpOccurrenceArg::FirstOrAll => globalplatform::GetStatusOccurrence::FirstOrAll,
        GpOccurrenceArg::Next => globalplatform::GetStatusOccurrence::Next,
    }
}

/// Map the CLI GP card-state enum to the maintained domain enum.
fn gp_card_state(value: GpCardStateArg) -> globalplatform::CardLifeCycle {
    match value {
        GpCardStateArg::OpReady => globalplatform::CardLifeCycle::OpReady,
        GpCardStateArg::Initialized => globalplatform::CardLifeCycle::Initialized,
        GpCardStateArg::Secured => globalplatform::CardLifeCycle::Secured,
        GpCardStateArg::CardLocked => globalplatform::CardLifeCycle::CardLocked,
        GpCardStateArg::Terminated => globalplatform::CardLifeCycle::Terminated,
    }
}

/// Map the CLI GP lock-transition enum to the maintained domain enum.
fn gp_transition(value: GpTransitionArg) -> globalplatform::LockTransition {
    match value {
        GpTransitionArg::Lock => globalplatform::LockTransition::Lock,
        GpTransitionArg::Unlock => globalplatform::LockTransition::Unlock,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jcim_core::iso7816::{IsoCapabilities, IsoSessionState};
    use jcim_sdk::SimulationStatus;

    use super::*;

    fn sample_simulation(simulation_id: &str) -> jcim_sdk::SimulationSummary {
        jcim_sdk::SimulationSummary {
            simulation_id: simulation_id.to_string(),
            project_id: "project-1".to_string(),
            project_path: PathBuf::from("/tmp/project"),
            status: SimulationStatus::Running,
            reader_name: "Reader".to_string(),
            health: "ready".to_string(),
            atr: None,
            active_protocol: None,
            iso_capabilities: IsoCapabilities::default(),
            session_state: IsoSessionState::default(),
            package_count: 1,
            applet_count: 1,
            package_name: "com.jcim.demo".to_string(),
            package_aid: "F000000001".to_string(),
            recent_events: Vec::new(),
        }
    }

    #[test]
    fn resolve_project_ref_prefers_id_then_path_then_manifest_directory() {
        let cwd = PathBuf::from("/tmp/jcim-cli-dispatch");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        std::fs::write(cwd.join("jcim.toml"), "[project]\nname = 'demo'\n")
            .expect("write manifest");

        assert_eq!(
            resolve_project_ref_from_cwd(
                ProjectSelectorArgs {
                    id: Some("project-id".to_string()),
                    project: Some(PathBuf::from("/tmp/other")),
                },
                &cwd,
            )
            .expect("resolve by id"),
            ProjectRef::from_id("project-id")
        );
        assert_eq!(
            resolve_project_ref_from_cwd(
                ProjectSelectorArgs {
                    id: None,
                    project: Some(PathBuf::from("/tmp/project")),
                },
                &cwd,
            )
            .expect("resolve by path"),
            ProjectRef::from_path("/tmp/project")
        );
        assert_eq!(
            resolve_project_ref_from_cwd(
                ProjectSelectorArgs {
                    id: None,
                    project: None,
                },
                &cwd,
            )
            .expect("resolve by cwd"),
            ProjectRef::from_path(&cwd)
        );

        let _ = std::fs::remove_dir_all(cwd);
    }

    #[test]
    fn resolve_project_ref_reports_missing_selection_without_manifest() {
        let cwd = PathBuf::from("/tmp/jcim-cli-dispatch-missing");
        std::fs::create_dir_all(&cwd).expect("create cwd");

        let error = resolve_project_ref_from_cwd(
            ProjectSelectorArgs {
                id: None,
                project: None,
            },
            &cwd,
        )
        .expect_err("missing project selection should fail");
        assert!(error.contains("no project selected"));

        let _ = std::fs::remove_dir_all(cwd);
    }

    #[test]
    fn resolve_simulation_ref_from_summaries_handles_zero_one_and_many() {
        let none = resolve_simulation_ref_from_summaries(&[])
            .expect_err("empty simulation lists should fail");
        assert!(none.contains("no active simulations"));

        let single = resolve_simulation_ref_from_summaries(&[sample_simulation("sim-1")])
            .expect("single simulation");
        assert_eq!(single, SimulationRef::new("sim-1"));

        let many = resolve_simulation_ref_from_summaries(&[
            sample_simulation("sim-1"),
            sample_simulation("sim-2"),
        ])
        .expect_err("multiple simulations should fail");
        assert!(many.contains("multiple simulations are active"));
    }

    #[test]
    fn parse_helpers_accept_supported_values_and_reject_invalid_input() {
        let apdu = parse_command_apdu("00A4040000").expect("parse apdu");
        assert_eq!(hex::encode_upper(apdu.to_bytes()), "00A4040000");
        assert!(parse_command_apdu("XYZ").is_err());

        let aid = parse_aid("A000000003000000").expect("parse aid");
        assert_eq!(aid.to_hex(), "A000000003000000");
        assert!(parse_aid("invalid").is_err());

        assert_eq!(
            parse_secure_messaging_protocol("iso-7816").expect("iso protocol"),
            SecureMessagingProtocol::Iso7816
        );
        assert_eq!(
            parse_secure_messaging_protocol("scp02").expect("scp02 protocol"),
            SecureMessagingProtocol::Scp02
        );
        assert_eq!(
            parse_secure_messaging_protocol("other:custom").expect("custom protocol"),
            SecureMessagingProtocol::Other("custom".to_string())
        );
        assert!(parse_secure_messaging_protocol("unknown").is_err());
    }

    #[test]
    fn gp_mapping_helpers_convert_cli_enums_to_domain_enums() {
        assert_eq!(
            gp_registry_kind(GpRegistryKindArg::LoadFilesAndModules),
            globalplatform::RegistryKind::ExecutableLoadFilesAndModules
        );
        assert_eq!(
            gp_occurrence(GpOccurrenceArg::Next),
            globalplatform::GetStatusOccurrence::Next
        );
        assert_eq!(
            gp_card_state(GpCardStateArg::CardLocked),
            globalplatform::CardLifeCycle::CardLocked
        );
        assert_eq!(
            gp_transition(GpTransitionArg::Unlock),
            globalplatform::LockTransition::Unlock
        );
    }
}
