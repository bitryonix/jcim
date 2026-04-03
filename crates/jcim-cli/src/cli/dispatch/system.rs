use std::path::PathBuf;

use jcim_config::project::ManagedPaths;
use jcim_sdk::{JcimClient, ServiceStatusSummary};

use super::super::{
    args::{SystemCommand, SystemServiceCommand},
    output,
};

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
