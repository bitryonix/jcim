use super::output;

/// Internal CLI error type that preserves the requested output mode for rendering.
pub(crate) struct CliError {
    /// User-facing message rendered to stderr or the JSON error envelope.
    message: String,
    /// Whether the originating command requested JSON-mode output.
    json_mode: bool,
}

impl CliError {
    /// Construct one CLI error with its associated output mode.
    pub(crate) fn new(message: String, json_mode: bool) -> Self {
        Self { message, json_mode }
    }

    /// Return whether this error originated from a JSON-mode command.
    pub(crate) fn json_mode(&self) -> bool {
        self.json_mode
    }

    /// Render this error using the stable CLI JSON error envelope.
    pub(crate) fn json_output(&self) -> String {
        output::json_error(&self.message)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::CliError;

    #[test]
    fn cli_error_preserves_display_and_json_mode() {
        let error = CliError::new("boom".to_string(), true);

        assert_eq!(error.to_string(), "boom");
        assert!(error.json_mode());
    }

    #[test]
    fn cli_error_json_output_uses_standard_error_envelope() {
        let error = CliError::new("boom".to_string(), false);
        let json = serde_json::from_str::<Value>(&error.json_output()).expect("json output");

        assert!(!error.json_mode());
        assert_eq!(json["schema_version"], "jcim-cli.v2");
        assert_eq!(json["kind"], "error");
        assert_eq!(json["message"], "boom");
    }
}
