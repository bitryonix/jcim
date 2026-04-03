use jcim_sdk::{BuildSummary, JcimClient};

use super::super::{
    args::{BuildCommand, BuildSubcommand},
    output,
};
use super::helpers::resolve_project_ref;

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
