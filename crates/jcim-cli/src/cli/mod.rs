//! Command parsing and task-oriented CLI execution.

/// Typed clap parser definitions and reusable argument groups.
mod args;
/// Command execution and selector/parsing helpers.
mod dispatch;
/// CLI error type that preserves JSON-mode rendering context.
mod error;
/// Human-readable and JSON rendering helpers.
mod output;
/// Parse and execute one CLI command.
mod runtime;

pub(crate) use self::runtime::run;
