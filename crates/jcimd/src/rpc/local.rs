use std::pin::Pin;

use tokio_stream::Stream;
use tonic::Status;

use jcim_app::JcimApp;

/// Boxed tonic stream type returned by RPC methods that stream items back to clients.
pub(crate) type RpcStream<T> =
    Pin<Box<dyn Stream<Item = std::result::Result<T, Status>> + Send + 'static>>;

/// In-process RPC adapter that forwards tonic requests into the local application services.
#[derive(Clone)]
pub(crate) struct LocalRpc {
    /// Shared JCIM application façade used by the RPC method implementations.
    pub(crate) app: JcimApp,
}
