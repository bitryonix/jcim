mod human;
mod json;

use jcim_sdk::iso7816::IsoSessionState;
use jcim_sdk::{
    BuildSummary, CardAppletInventory, CardDeleteSummary, CardInstallSummary, CardPackageInventory,
    EventLine, GpSecureChannelSummary, ManageChannelSummary, ProjectDetails, ProjectSummary,
    ResetSummary, ServiceStatusSummary, SimulationSummary, globalplatform,
};
use serde_json::{Value, json};

pub(super) fn json_error(message: &str) -> String {
    json::json_error(message)
}

pub(super) fn print_json_value(kind: &str, payload: Value) {
    json::print_json_value(kind, payload);
}

pub(super) fn print_project_summary(project: &ProjectSummary, json_mode: bool) {
    if json_mode {
        json::print_json_value("project.summary", json!(project));
    } else {
        human::print_project_summary(project);
    }
}

pub(super) fn print_project_details(details: &ProjectDetails, json_mode: bool) {
    if json_mode {
        json::print_json_value("project.details", json!(details));
    } else {
        human::print_project_details(details);
    }
}

pub(super) fn print_build_summary(summary: &BuildSummary, show_rebuilt: bool, json_mode: bool) {
    if json_mode {
        json::print_json_value("build.summary", json!(summary));
    } else {
        human::print_build_summary(summary, show_rebuilt);
    }
}

pub(super) fn print_simulation(simulation: &SimulationSummary, json_mode: bool) {
    if json_mode {
        json::print_json_value("simulation.summary", json!(simulation));
    } else {
        human::print_simulation(simulation);
    }
}

pub(super) fn print_simulation_list(simulations: &[SimulationSummary], json_mode: bool) {
    if json_mode {
        json::print_json_value("simulation.list", json!({ "simulations": simulations }));
    } else if simulations.is_empty() {
        println!("No active simulations.");
    } else {
        for simulation in simulations {
            human::print_simulation(simulation);
            println!();
        }
    }
}

pub(super) fn print_simulation_events(events: &[EventLine], json_mode: bool) {
    if json_mode {
        json::print_json_value("simulation.events", json!({ "events": events }));
    } else {
        human::print_event_lines(events);
    }
}

pub(super) fn print_card_install(summary: &CardInstallSummary, json_mode: bool) {
    if json_mode {
        json::print_json_value("card.install", json!(summary));
    } else {
        human::print_card_install(summary);
    }
}

pub(super) fn print_card_delete(summary: &CardDeleteSummary, json_mode: bool) {
    if json_mode {
        json::print_json_value("card.delete", json!(summary));
    } else {
        human::print_card_delete(summary);
    }
}

pub(super) fn print_package_inventory(inventory: &CardPackageInventory, json_mode: bool) {
    if json_mode {
        json::print_json_value("card.packages", json!(inventory));
    } else {
        human::print_package_inventory(inventory);
    }
}

pub(super) fn print_applet_inventory(inventory: &CardAppletInventory, json_mode: bool) {
    if json_mode {
        json::print_json_value("card.applets", json!(inventory));
    } else {
        human::print_applet_inventory(inventory);
    }
}

pub(super) fn print_card_readers(readers: &[jcim_sdk::CardReaderSummary], json_mode: bool) {
    if json_mode {
        json::print_json_value("card.readers", json!({ "readers": readers }));
    } else {
        human::print_card_readers(readers);
    }
}

pub(super) fn print_card_status(status: &jcim_sdk::CardStatusSummary, json_mode: bool) {
    if json_mode {
        json::print_json_value("card.status", json!(status));
    } else {
        human::print_plain_lines(&status.lines);
    }
}

pub(super) fn print_reset_summary(summary: &ResetSummary, kind: &str, json_mode: bool) {
    if json_mode {
        json::print_json_value(kind, json!(summary));
    } else {
        human::print_reset_summary(summary);
    }
}

pub(super) fn print_setup_summary(summary: &jcim_sdk::SetupSummary, json_mode: bool) {
    if json_mode {
        json::print_json_value("system.setup", json!(summary));
    } else {
        println!("{}", summary.message);
    }
}

pub(super) fn print_doctor_lines(lines: &[String], json_mode: bool) {
    if json_mode {
        json::print_json_value("system.doctor", json!({ "lines": lines }));
    } else {
        human::print_plain_lines(lines);
    }
}

pub(super) fn print_service_status(status: &ServiceStatusSummary, json_mode: bool) {
    if json_mode {
        json::print_json_value("system.service_status", json!(status));
    } else {
        human::print_service_status(status);
    }
}

pub(super) fn print_iso_session_state(state: &IsoSessionState, json_mode: bool) {
    if json_mode {
        json::print_json_value("session.iso", json!(state));
    } else {
        human::print_iso_session_state(state);
    }
}

pub(super) fn print_manage_channel_summary(summary: &ManageChannelSummary, json_mode: bool) {
    if json_mode {
        json::print_json_value("channel.summary", json!(summary));
    } else {
        human::print_manage_channel_summary(summary);
    }
}

pub(super) fn print_secure_messaging_summary(
    summary: &jcim_sdk::SecureMessagingSummary,
    json_mode: bool,
) {
    if json_mode {
        json::print_json_value("secure_messaging.summary", json!(summary));
    } else {
        human::print_secure_messaging_summary(summary);
    }
}

pub(super) fn print_gp_secure_channel_summary(summary: &GpSecureChannelSummary, json_mode: bool) {
    if json_mode {
        json::print_json_value("gp.secure_channel", json!(summary));
    } else {
        human::print_gp_secure_channel_summary(summary);
    }
}

pub(super) fn print_gp_status_response(
    response: &globalplatform::GetStatusResponse,
    json_mode: bool,
) {
    if json_mode {
        json::print_json_value(
            "gp.status",
            json!({
                "registry_kind": response.kind,
                "entries": response.entries,
                "more_data_available": response.more_data_available,
            }),
        );
    } else {
        human::print_gp_status_response(response);
    }
}

pub(super) fn print_apdu_response(response: &jcim_sdk::ResponseApdu, json_mode: bool) {
    let response_hex = hex::encode_upper(response.to_bytes());
    if json_mode {
        json::print_json_value(
            "apdu.response",
            json!({
                "response_hex": response_hex,
                "status_word": format!("{:04X}", response.sw),
                "data_hex": hex::encode_upper(&response.data),
            }),
        );
    } else {
        human::print_apdu_response(response);
    }
}
