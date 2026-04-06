use jcim_sdk::iso7816::IsoSessionState;
use jcim_sdk::{
    BuildSummary, CardAppletInventory, CardDeleteSummary, CardInstallSummary, CardPackageInventory,
    EventLine, GpSecureChannelSummary, ManageChannelSummary, ProjectDetails, ProjectSummary,
    ResetSummary, ServiceStatusSummary, SimulationSummary, globalplatform,
};
use serde_json::{Value, json};

use super::{human, json as json_output};

/// Render one project summary in human-readable or JSON form.
pub(crate) fn print_project_summary(project: &ProjectSummary, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("project.summary", json!(project));
    } else {
        human::print_project_summary(project);
    }
}

/// Render one project-details payload in human-readable or JSON form.
pub(crate) fn print_project_details(details: &ProjectDetails, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("project.details", json!(details));
    } else {
        human::print_project_details(details);
    }
}

/// Render one build summary in human-readable or JSON form.
pub(crate) fn print_build_summary(summary: &BuildSummary, show_rebuilt: bool, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("build.summary", json!(summary));
    } else {
        human::print_build_summary(summary, show_rebuilt);
    }
}

/// Render one simulation summary in human-readable or JSON form.
pub(crate) fn print_simulation(simulation: &SimulationSummary, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("simulation.summary", json!(simulation));
    } else {
        human::print_simulation(simulation);
    }
}

/// Render one simulation list in human-readable or JSON form.
pub(crate) fn print_simulation_list(simulations: &[SimulationSummary], json_mode: bool) {
    if json_mode {
        json_output::print_json_value("simulation.list", simulation_list_payload(simulations));
    } else {
        print!("{}", render_simulation_list(simulations));
    }
}

/// Render one simulation event list in human-readable or JSON form.
pub(crate) fn print_simulation_events(events: &[EventLine], json_mode: bool) {
    if json_mode {
        json_output::print_json_value("simulation.events", json!({ "events": events }));
    } else {
        human::print_event_lines(events);
    }
}

/// Render one card-install summary in human-readable or JSON form.
pub(crate) fn print_card_install(summary: &CardInstallSummary, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("card.install", json!(summary));
    } else {
        human::print_card_install(summary);
    }
}

/// Render one card-delete summary in human-readable or JSON form.
pub(crate) fn print_card_delete(summary: &CardDeleteSummary, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("card.delete", json!(summary));
    } else {
        human::print_card_delete(summary);
    }
}

/// Render one package inventory in human-readable or JSON form.
pub(crate) fn print_package_inventory(inventory: &CardPackageInventory, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("card.packages", json!(inventory));
    } else {
        human::print_package_inventory(inventory);
    }
}

/// Render one applet inventory in human-readable or JSON form.
pub(crate) fn print_applet_inventory(inventory: &CardAppletInventory, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("card.applets", json!(inventory));
    } else {
        human::print_applet_inventory(inventory);
    }
}

/// Render the discovered card-reader list in human-readable or JSON form.
pub(crate) fn print_card_readers(readers: &[jcim_sdk::CardReaderSummary], json_mode: bool) {
    if json_mode {
        json_output::print_json_value("card.readers", json!({ "readers": readers }));
    } else {
        human::print_card_readers(readers);
    }
}

/// Render one card-status summary in human-readable or JSON form.
pub(crate) fn print_card_status(status: &jcim_sdk::CardStatusSummary, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("card.status", json!(status));
    } else {
        human::print_plain_lines(&status.lines);
    }
}

/// Render one reset summary in human-readable or JSON form using the provided JSON kind tag.
pub(crate) fn print_reset_summary(summary: &ResetSummary, kind: &str, json_mode: bool) {
    if json_mode {
        json_output::print_json_value(kind, json!(summary));
    } else {
        human::print_reset_summary(summary);
    }
}

/// Render one system-setup summary in human-readable or JSON form.
pub(crate) fn print_setup_summary(summary: &jcim_sdk::SetupSummary, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("system.setup", json!(summary));
    } else {
        println!("{}", summary.message);
    }
}

/// Render one doctor response in human-readable or JSON form.
pub(crate) fn print_doctor_lines(lines: &[String], json_mode: bool) {
    if json_mode {
        json_output::print_json_value("system.doctor", json!({ "lines": lines }));
    } else {
        human::print_plain_lines(lines);
    }
}

/// Render one service-status summary in human-readable or JSON form.
pub(crate) fn print_service_status(status: &ServiceStatusSummary, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("system.service_status", json!(status));
    } else {
        human::print_service_status(status);
    }
}

/// Render one ISO session-state summary in human-readable or JSON form.
pub(crate) fn print_iso_session_state(state: &IsoSessionState, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("session.iso", json!(state));
    } else {
        human::print_iso_session_state(state);
    }
}

/// Render one manage-channel summary in human-readable or JSON form.
pub(crate) fn print_manage_channel_summary(summary: &ManageChannelSummary, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("channel.summary", json!(summary));
    } else {
        human::print_manage_channel_summary(summary);
    }
}

/// Render one secure-messaging summary in human-readable or JSON form.
pub(crate) fn print_secure_messaging_summary(
    summary: &jcim_sdk::SecureMessagingSummary,
    json_mode: bool,
) {
    if json_mode {
        json_output::print_json_value("secure_messaging.summary", json!(summary));
    } else {
        human::print_secure_messaging_summary(summary);
    }
}

/// Render one GP secure-channel summary in human-readable or JSON form.
pub(crate) fn print_gp_secure_channel_summary(summary: &GpSecureChannelSummary, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("gp.secure_channel", json!(summary));
    } else {
        human::print_gp_secure_channel_summary(summary);
    }
}

/// Render one GP registry-status response in human-readable or JSON form.
pub(crate) fn print_gp_status_response(
    response: &globalplatform::GetStatusResponse,
    json_mode: bool,
) {
    if json_mode {
        json_output::print_json_value("gp.status", gp_status_payload(response));
    } else {
        human::print_gp_status_response(response);
    }
}

/// Render one APDU response in human-readable or JSON form.
pub(crate) fn print_apdu_response(response: &jcim_sdk::ResponseApdu, json_mode: bool) {
    if json_mode {
        json_output::print_json_value("apdu.response", apdu_response_payload(response));
    } else {
        human::print_apdu_response(response);
    }
}

/// Build the JSON payload used for simulation-list rendering.
fn simulation_list_payload(simulations: &[SimulationSummary]) -> Value {
    json!({ "simulations": simulations })
}

/// Render the human-readable simulation-list body.
fn render_simulation_list(simulations: &[SimulationSummary]) -> String {
    if simulations.is_empty() {
        return "No active simulations.\n".to_string();
    }

    let mut output = String::new();
    for simulation in simulations {
        output.push_str(&human::render_simulation(simulation));
        output.push('\n');
    }
    output
}

/// Build the JSON payload used for GP registry-status rendering.
fn gp_status_payload(response: &globalplatform::GetStatusResponse) -> Value {
    json!({
        "registry_kind": response.kind,
        "entries": response.entries,
        "more_data_available": response.more_data_available,
    })
}

/// Build the JSON payload used for APDU response rendering.
fn apdu_response_payload(response: &jcim_sdk::ResponseApdu) -> Value {
    json!({
        "response_hex": hex::encode_upper(response.to_bytes()),
        "status_word": format!("{:04X}", response.sw),
        "data_hex": hex::encode_upper(&response.data),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jcim_core::aid::Aid;
    use jcim_core::apdu::ResponseApdu;
    use jcim_core::globalplatform::{GetStatusResponse, RegistryEntry, RegistryKind};
    use jcim_core::iso7816::{
        Atr, IsoCapabilities, IsoSessionState, LogicalChannelState, PowerState, RetryCounterState,
        SecureMessagingProtocol, SecureMessagingState, StatusWord, TransportProtocol,
    };

    use super::*;

    #[test]
    fn simulation_list_rendering_covers_empty_and_non_empty_paths() {
        assert_eq!(render_simulation_list(&[]), "No active simulations.\n");

        let rendered = render_simulation_list(&[sample_simulation()]);
        assert!(rendered.contains("Simulation: sim-1"));
        assert!(rendered.contains("Status: running"));

        let payload = simulation_list_payload(&[sample_simulation()]);
        assert_eq!(
            payload["simulations"]
                .as_array()
                .expect("simulations")
                .len(),
            1
        );
    }

    #[test]
    fn apdu_and_gp_status_json_payloads_are_stable() {
        let apdu = apdu_response_payload(&ResponseApdu {
            data: vec![0xCA, 0xFE],
            sw: 0x9000,
        });
        assert_eq!(apdu["response_hex"], "CAFE9000");
        assert_eq!(apdu["status_word"], "9000");
        assert_eq!(apdu["data_hex"], "CAFE");

        let gp = gp_status_payload(&GetStatusResponse {
            kind: RegistryKind::Applications,
            entries: vec![RegistryEntry {
                kind: RegistryKind::Applications,
                aid: Aid::from_hex("A000000151000001").expect("aid"),
                life_cycle_state: 0x07,
                privileges: Some([0x01, 0x02, 0x03]),
                executable_load_file_aid: None,
                associated_security_domain_aid: None,
                executable_module_aids: Vec::new(),
                load_file_version: Some(vec![1, 2]),
                implicit_selection_parameters: vec![0xAA],
            }],
            more_data_available: true,
        });
        assert_eq!(gp["more_data_available"], true);
        assert_eq!(gp["entries"].as_array().expect("entries").len(), 1);
        assert_eq!(gp["registry_kind"], "Applications");
    }

    /// Build one representative running-simulation summary for rendering tests.
    fn sample_simulation() -> SimulationSummary {
        let atr = Atr::parse(&[0x3B, 0x80, 0x01, 0x00]).expect("atr");
        SimulationSummary {
            simulation_id: "sim-1".to_string(),
            project_id: "proj-1".to_string(),
            project_path: PathBuf::from("/tmp/project"),
            status: jcim_sdk::SimulationStatus::Running,
            reader_name: "Reader".to_string(),
            health: "healthy".to_string(),
            atr: Some(atr.clone()),
            active_protocol: Some(jcim_core::iso7816::ProtocolParameters::from_atr(&atr)),
            iso_capabilities: IsoCapabilities {
                protocols: vec![TransportProtocol::T1],
                extended_length: true,
                logical_channels: true,
                max_logical_channels: 4,
                secure_messaging: true,
                file_model_visibility: true,
                raw_apdu: true,
            },
            session_state: IsoSessionState {
                power_state: PowerState::On,
                atr: Some(atr),
                active_protocol: None,
                selected_aid: Some(Aid::from_hex("A000000151000001").expect("aid")),
                current_file: None,
                open_channels: vec![LogicalChannelState {
                    channel_number: 0,
                    selected_aid: None,
                    current_file: None,
                }],
                secure_messaging: SecureMessagingState {
                    active: true,
                    protocol: Some(SecureMessagingProtocol::Scp03),
                    security_level: Some(0x13),
                    session_id: Some("session-1".to_string()),
                    command_counter: 2,
                },
                verified_references: vec![0x81],
                retry_counters: vec![RetryCounterState {
                    reference: 0x81,
                    remaining: 3,
                }],
                last_status: Some(StatusWord::SUCCESS),
            },
            package_count: 1,
            applet_count: 1,
            package_name: "com.example.demo".to_string(),
            package_aid: "A000000151000001".to_string(),
            recent_events: vec!["info: started".to_string()],
        }
    }
}
