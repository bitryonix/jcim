//! Service bootstrap and lifecycle methods for the JCIM SDK.

/// Service bootstrap, socket connection, and restart helpers.
mod bootstrap;
/// Build-service request helpers and response validation.
mod build;
/// Physical-card request helpers and response validation.
mod cards;
/// Canonical SDK client handle and shared connection helpers.
mod handle;
/// Project-service request helpers and selector translation.
mod projects;
/// Protobuf-to-SDK translation helpers and response decoding.
mod proto;
/// Simulator-service request helpers and response validation.
mod simulations;
/// System-service request helpers and service bootstrap reporting.
mod system;
/// Workspace overview and listing helpers.
mod workspace;

pub use self::handle::JcimClient;
