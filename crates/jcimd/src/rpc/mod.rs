/// Build RPC adapter methods.
mod build;
/// Card RPC adapter methods.
mod card;
/// Shared local RPC adapter types.
mod local;
/// Project RPC adapter methods.
mod project;
/// Simulator RPC adapter methods.
mod simulator;
/// System RPC adapter methods.
mod system;
/// Test-only fixtures for direct `LocalRpc` adapter tests.
#[cfg(test)]
mod testsupport;
/// Workspace RPC adapter methods.
mod workspace;

pub(crate) use self::local::{LocalRpc, RpcStream};
