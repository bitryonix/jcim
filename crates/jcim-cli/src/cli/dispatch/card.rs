use jcim_sdk::{CardInstallSource, JcimClient};

use super::super::{
    args::{CardCommand, CardGpAuthCommand, CardGpCommand, CardIsoCommand},
    output,
};
use super::helpers::{
    gp_card_state, gp_occurrence, gp_registry_kind, gp_transition, parse_aid, parse_command_apdu,
    parse_secure_messaging_protocol, reader_ref, resolve_project_ref,
};

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
