use tonic::Status;

use jcim_api::v0_3::GetServiceStatusResponse;
use jcim_app::ServiceStatusSummary;
use jcim_core::error::JcimError;

/// Encode service-status state into the RPC service-status response envelope.
pub(crate) fn service_status_response(status: ServiceStatusSummary) -> GetServiceStatusResponse {
    GetServiceStatusResponse {
        socket_path: status.socket_path.display().to_string(),
        running: status.running,
        known_project_count: status.known_project_count,
        active_simulation_count: status.active_simulation_count,
        service_binary_path: status.service_binary_path.display().to_string(),
        service_binary_fingerprint: status.service_binary_fingerprint,
    }
}

/// Map one application error into the tonic transport status used by the local daemon API.
pub(crate) fn to_status(error: JcimError) -> Status {
    match error {
        JcimError::Unsupported(message)
        | JcimError::InvalidAid(message)
        | JcimError::InvalidApdu(message)
        | JcimError::Gp(message)
        | JcimError::CapFormat(message)
        | JcimError::MalformedBackendReply(message) => Status::invalid_argument(message),
        JcimError::BackendUnavailable(message)
        | JcimError::BackendExited(message)
        | JcimError::BackendStartup(message) => Status::unavailable(message),
        other => Status::internal(other.to_string()),
    }
}
