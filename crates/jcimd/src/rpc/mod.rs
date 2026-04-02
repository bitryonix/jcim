use std::pin::Pin;

use tokio_stream::Stream;
use tonic::Status;

use jcim_app::JcimApp;

/// Boxed tonic stream type returned by RPC methods that stream items back to clients.
pub(crate) type RpcStream<T> =
    Pin<Box<dyn Stream<Item = std::result::Result<T, Status>> + Send + 'static>>;

#[derive(Clone)]
/// In-process RPC adapter that forwards tonic requests into the local application services.
pub(crate) struct LocalRpc {
    /// Shared JCIM application façade used by the RPC method implementations.
    pub(crate) app: JcimApp,
}

/// Build RPC adapter methods.
mod build;
/// Card RPC adapter methods.
mod card;
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
