//! CAP parsing and export resolution for JCIM.
//!
//! # Why this exists
//! The runtime, backend verifier, and CLI all need the same CAP parsing rules. Centralizing that
//! work here keeps Java Card package validation independent from transport or service-shell code.
//!
//! # Role in the system
//! Use [`cap`] to parse CAP archives and [`export`] to validate imported packages against known
//! exports before installation or backend verification.

pub mod cap;
pub mod export;
pub mod prelude;
