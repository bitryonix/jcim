use std::pin::Pin;

use tokio_stream::Stream;
use tonic::Status;

use jcim_app::JcimApp;

pub(crate) type RpcStream<T> =
    Pin<Box<dyn Stream<Item = std::result::Result<T, Status>> + Send + 'static>>;

#[derive(Clone)]
pub(crate) struct LocalRpc {
    pub(crate) app: JcimApp,
}

mod build;
mod card;
mod project;
mod simulator;
mod system;
mod workspace;
