use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

/// High-level status-word class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StatusWordClass {
    /// Successful processing.
    NormalProcessing,
    /// Warning processing state.
    Warning,
    /// Execution error.
    ExecutionError,
    /// Checking error or malformed request.
    CheckingError,
    /// One unmapped status-word family.
    Unknown,
}

/// Parsed ISO/IEC 7816 status word.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct StatusWord(u16);

impl StatusWord {
    /// `90 00`.
    pub const SUCCESS: Self = Self(0x9000);
    /// `61 00` class: response bytes remain available.
    pub const RESPONSE_BYTES_AVAILABLE: Self = Self(0x6100);
    /// `63 10`: more data available for `GET STATUS` continuation flows.
    pub const MORE_DATA_AVAILABLE: Self = Self(0x6310);
    /// `62 83`.
    pub const WARNING_SELECTED_FILE_INVALIDATED: Self = Self(0x6283);
    /// `63 C0` class: verification failed with retries remaining.
    pub const VERIFY_FAIL_RETRY_COUNTER_BASE: Self = Self(0x63C0);
    /// `69 82`.
    pub const SECURITY_STATUS_NOT_SATISFIED: Self = Self(0x6982);
    /// `69 83`.
    pub const AUTH_METHOD_BLOCKED: Self = Self(0x6983);
    /// `69 85`.
    pub const CONDITIONS_NOT_SATISFIED: Self = Self(0x6985);
    /// `69 86`.
    pub const COMMAND_NOT_ALLOWED: Self = Self(0x6986);
    /// `6A 82`.
    pub const FILE_OR_APPLICATION_NOT_FOUND: Self = Self(0x6A82);
    /// `6A 86`.
    pub const INCORRECT_P1_P2: Self = Self(0x6A86);
    /// `6A 88`.
    pub const DATA_NOT_FOUND: Self = Self(0x6A88);
    /// `67 00`.
    pub const WRONG_LENGTH: Self = Self(0x6700);
    /// `6C 00` class: exact length hint available.
    pub const CORRECT_LENGTH_HINT: Self = Self(0x6C00);
    /// `6D 00`.
    pub const INSTRUCTION_NOT_SUPPORTED: Self = Self(0x6D00);
    /// `6E 00`.
    pub const CLASS_NOT_SUPPORTED: Self = Self(0x6E00);

    /// Build one status-word helper from a raw value.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Borrow the raw value.
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Return the coarse status-word class.
    pub const fn class(self) -> StatusWordClass {
        match (self.0 >> 8) as u8 {
            0x90 | 0x61 => StatusWordClass::NormalProcessing,
            0x62 | 0x63 => StatusWordClass::Warning,
            0x64..=0x66 => StatusWordClass::ExecutionError,
            0x67..=0x6F => StatusWordClass::CheckingError,
            _ => StatusWordClass::Unknown,
        }
    }

    /// Report whether the status word represents successful completion.
    pub const fn is_success(self) -> bool {
        matches!(self.class(), StatusWordClass::NormalProcessing) && (self.0 >> 8) as u8 != 0x6F
    }

    /// Report whether the status word is warning-class.
    pub const fn is_warning(self) -> bool {
        matches!((self.0 >> 8) as u8, 0x62 | 0x63)
    }

    /// Return remaining response bytes hinted by `61 XX`, when present.
    pub const fn remaining_response_bytes(self) -> Option<usize> {
        if (self.0 >> 8) as u8 == 0x61 {
            let low = (self.0 & 0x00FF) as usize;
            Some(if low == 0 { 256 } else { low })
        } else {
            None
        }
    }

    /// Return retries remaining when encoded as `63 CX`.
    pub const fn retry_counter(self) -> Option<u8> {
        if (self.0 & 0xFFF0) == 0x63C0 {
            Some((self.0 & 0x000F) as u8)
        } else {
            None
        }
    }

    /// Return a corrected length hint when encoded as `6C XX`.
    pub const fn exact_length_hint(self) -> Option<usize> {
        if (self.0 >> 8) as u8 == 0x6C {
            let low = (self.0 & 0x00FF) as usize;
            Some(if low == 0 { 256 } else { low })
        } else {
            None
        }
    }

    /// Return one stable label for common status words and status-word classes.
    pub fn label(self) -> &'static str {
        match self.0 {
            0x9000 => "success",
            0x6310 => "more_data_available",
            0x6283 => "selected_file_invalidated",
            0x6982 => "security_status_not_satisfied",
            0x6983 => "authentication_method_blocked",
            0x6985 => "conditions_not_satisfied",
            0x6986 => "command_not_allowed",
            0x6A82 => "file_or_application_not_found",
            0x6A86 => "incorrect_p1_p2",
            0x6A88 => "data_not_found",
            0x6700 => "wrong_length",
            0x6D00 => "instruction_not_supported",
            0x6E00 => "class_not_supported",
            _ if (self.0 >> 8) as u8 == 0x61 => "response_bytes_available",
            _ if (self.0 & 0xFFF0) == 0x63C0 => "verify_failed_retries_remaining",
            _ if (self.0 >> 8) as u8 == 0x6C => "correct_length_hint",
            _ => "unknown_status",
        }
    }
}

impl From<u16> for StatusWord {
    fn from(value: u16) -> Self {
        Self::new(value)
    }
}

impl From<StatusWord> for u16 {
    fn from(value: StatusWord) -> Self {
        value.as_u16()
    }
}

impl Display for StatusWord {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04X}", self.0)
    }
}
