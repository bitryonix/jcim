use jcim_core::error::JcimError;

pub(super) fn cap_format(message: impl Into<String>) -> JcimError {
    JcimError::CapFormat(message.into())
}

pub(super) fn unsupported(message: impl Into<String>) -> JcimError {
    JcimError::Unsupported(message.into())
}

pub(super) fn unexpected_end_of_zip_data() -> JcimError {
    cap_format("unexpected end of ZIP data".to_string())
}

pub(super) fn zip_entry_out_of_bounds() -> JcimError {
    cap_format("ZIP entry extends beyond the archive boundary".to_string())
}
