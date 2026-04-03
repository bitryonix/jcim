use jcim_api::v0_3::{GetServiceStatusResponse, ResetCardResponse, ResetSimulationResponse};

use crate::error::Result;
use crate::types::{ResetSummary, ServiceStatusSummary, SetupSummary, owned_path};

use super::iso::{atr_from_proto, iso_session_state_from_proto};

/// Decode one simulation reset response into the unified SDK reset summary.
pub(in crate::client) fn reset_summary_from_simulation_proto(
    response: ResetSimulationResponse,
) -> Result<ResetSummary> {
    reset_summary_from_parts(response.atr, response.session_state)
}

/// Decode one physical-card reset response into the unified SDK reset summary.
pub(in crate::client) fn reset_summary_from_card_proto(
    response: ResetCardResponse,
) -> Result<ResetSummary> {
    reset_summary_from_parts(response.atr, response.session_state)
}

/// Build one reset summary, preferring the explicit reset ATR and otherwise falling back to the
/// session snapshot so the existing summary contract stays intact.
fn reset_summary_from_parts(
    atr: Option<jcim_api::v0_3::AtrInfo>,
    session_state: Option<jcim_api::v0_3::IsoSessionStateInfo>,
) -> Result<ResetSummary> {
    let atr = atr_from_proto(atr)?;
    let session_state = iso_session_state_from_proto(session_state)?;
    Ok(ResetSummary {
        atr: atr.or_else(|| session_state.atr.clone()),
        session_state,
    })
}

/// Decode one service-status response into the stable SDK summary type.
pub(in crate::client) fn service_status_summary(
    response: GetServiceStatusResponse,
) -> Result<ServiceStatusSummary> {
    let GetServiceStatusResponse {
        socket_path,
        running,
        known_project_count,
        active_simulation_count,
        service_binary_path,
        service_binary_fingerprint,
    } = response;
    Ok(ServiceStatusSummary {
        socket_path: owned_path(socket_path),
        running,
        known_project_count,
        active_simulation_count,
        service_binary_path: owned_path(service_binary_path),
        service_binary_fingerprint,
    })
}

/// Decode one toolchain-setup response into the stable SDK summary type.
pub(in crate::client) fn setup_summary(
    response: jcim_api::v0_3::SetupToolchainsResponse,
) -> SetupSummary {
    SetupSummary {
        config_path: owned_path(response.config_path),
        message: response.message,
    }
}
