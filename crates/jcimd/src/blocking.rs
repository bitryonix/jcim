use jcim_core::error::JcimError;
use tonic::Status;

pub(crate) async fn blocking<F, T>(work: F) -> Result<T, Status>
where
    F: FnOnce() -> Result<T, JcimError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| Status::internal(format!("blocking task failed: {error}")))?
        .map_err(crate::translate::to_status)
}
