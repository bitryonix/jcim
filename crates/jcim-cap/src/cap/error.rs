use jcim_core::error::JcimError;

/// Build one CAP-format error with the provided message.
pub(super) fn cap_format(message: impl Into<String>) -> JcimError {
    JcimError::CapFormat(message.into())
}

/// Build one unsupported-feature error for CAP parsing or validation.
pub(super) fn unsupported(message: impl Into<String>) -> JcimError {
    JcimError::Unsupported(message.into())
}

/// Build the standard truncated-ZIP-data error used by archive parsing helpers.
pub(super) fn unexpected_end_of_zip_data() -> JcimError {
    cap_format("unexpected end of ZIP data".to_string())
}

/// Build the standard out-of-bounds ZIP-entry error used by archive parsing helpers.
pub(super) fn zip_entry_out_of_bounds() -> JcimError {
    cap_format("ZIP entry extends beyond the archive boundary".to_string())
}
