//! Command parsing and task-oriented CLI execution.

#![allow(clippy::missing_docs_in_private_items)]

mod args;
mod dispatch;
mod output;

use clap::Parser;

use self::args::Cli;

pub(crate) struct CliError {
    message: String,
    json_mode: bool,
}

impl CliError {
    fn new(message: String, json_mode: bool) -> Self {
        Self { message, json_mode }
    }

    pub(crate) fn json_mode(&self) -> bool {
        self.json_mode
    }

    pub(crate) fn json_output(&self) -> String {
        output::json_error(&self.message)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

/// Parse and execute one CLI command.
pub(crate) async fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    let json_mode = cli.json;
    dispatch::dispatch(cli.command, json_mode)
        .await
        .map_err(|message| CliError::new(message, json_mode))
}
