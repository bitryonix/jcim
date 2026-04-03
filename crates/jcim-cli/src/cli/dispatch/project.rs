use std::path::PathBuf;

use jcim_sdk::JcimClient;
use serde_json::json;

use super::super::{args::ProjectCommand, output};
use super::helpers::resolve_project_ref;

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
