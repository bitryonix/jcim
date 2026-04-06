/// Human-readable CLI rendering helpers.
mod human;
/// JSON envelope and machine-readable rendering helpers.
mod json;
/// High-level rendering entrypoints for task-oriented CLI responses.
mod render;

pub(crate) use self::json::{json_error, print_json_value};
pub(crate) use self::render::{
    print_apdu_response, print_applet_inventory, print_build_summary, print_card_delete,
    print_card_install, print_card_readers, print_card_status, print_doctor_lines,
    print_gp_secure_channel_summary, print_gp_status_response, print_iso_session_state,
    print_manage_channel_summary, print_package_inventory, print_project_details,
    print_project_summary, print_reset_summary, print_secure_messaging_summary,
    print_service_status, print_setup_summary, print_simulation, print_simulation_events,
    print_simulation_list,
};
