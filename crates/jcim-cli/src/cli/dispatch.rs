/// Dispatch helpers for build-oriented CLI commands.
mod build;
/// Dispatch helpers for physical-card CLI commands.
mod card;
/// Shared selector, parsing, and GP mapping helpers for CLI dispatch.
mod helpers;
/// Dispatch helpers for project-management CLI commands.
mod project;
/// Dispatch helpers for simulator CLI commands.
mod sim;
/// Dispatch helpers for system-management CLI commands.
mod system;

use super::args::Command;

/// Dispatch one parsed top-level CLI command to its task-oriented execution path.
pub(super) async fn dispatch(command: Command, json_mode: bool) -> Result<(), String> {
    match command {
        Command::Project { command } => project::run_project(command, json_mode).await,
        Command::Build(command) => build::run_build(command, json_mode).await,
        Command::Sim { command } => sim::run_sim(command, json_mode).await,
        Command::Card { command } => card::run_card(command, json_mode).await,
        Command::System { command } => system::run_system(command, json_mode).await,
    }
}
