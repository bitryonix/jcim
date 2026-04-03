use jcim_sdk::{JcimClient, SimulationRef};

use super::super::{
    args::{SimCommand, SimGpAuthCommand, SimGpCommand, SimIsoCommand},
    output,
};
use super::helpers::{
    gp_card_state, gp_occurrence, gp_registry_kind, gp_transition, parse_aid, parse_command_apdu,
    parse_secure_messaging_protocol, resolve_project_ref, resolve_simulation_ref,
};

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
