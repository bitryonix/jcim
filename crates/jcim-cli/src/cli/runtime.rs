use clap::Parser;

use super::args::Cli;
use super::dispatch;
use super::error::CliError;

/// Parse and execute one CLI command.
pub(crate) async fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    let json_mode = cli.json;
    dispatch::dispatch(cli.command, json_mode)
        .await
        .map_err(|message| CliError::new(message, json_mode))
}
