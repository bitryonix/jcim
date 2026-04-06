//! Pure-Rust CAP/JAR parsing utilities.

/// ZIP archive readers for CAP/JAR inputs.
mod archive;
/// CAP manifest and component parsers.
mod components;
/// Small CAP-specific error constructors.
mod error;
/// Top-level CAP parsing orchestration.
mod parser;
/// CAP parser regression tests that exercise compact archive shapes.
#[cfg(test)]
mod tests;
/// Public CAP package types and constructors.
mod types;
/// Card-profile compatibility validation helpers.
mod validation;

pub use self::types::{CapApplet, CapFileVersion, CapPackage, ImportedPackage};
