use jcim_core::error::JcimError;
use tonic::Status;

/// Run one fallible blocking workload on Tokio's blocking pool and map failures to gRPC status.
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
