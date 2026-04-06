use tonic::transport::Channel;

use jcim_config::project::ManagedPaths;

use crate::connection::CardConnection;
use crate::error::{JcimSdkError, Result};
use crate::types::{CardConnectionLocator, CardConnectionTarget, SimulationStatus};

use super::bootstrap;

/// Canonical async Rust client for the local JCIM service.
#[derive(Clone)]
pub struct JcimClient {
    /// Managed path layout used to locate the local socket and runtime metadata.
    pub(super) managed_paths: ManagedPaths,
    /// Shared gRPC channel reused by task-oriented service clients.
    pub(super) channel: Channel,
}

impl JcimClient {
    /// Return the managed local paths associated with this client.
    pub fn managed_paths(&self) -> &ManagedPaths {
        &self.managed_paths
    }

    /// Open one unified APDU connection against a real reader or one virtual simulation.
    pub async fn open_card_connection(
        &self,
        target: CardConnectionTarget,
    ) -> Result<CardConnection> {
        let locator = match target {
            CardConnectionTarget::Reader(reader) => {
                let status = self.validated_card_status_for_connection(reader).await?;
                reader_connection_locator(&status)?
            }
            CardConnectionTarget::ExistingSimulation(simulation) => {
                let summary = self.validated_running_simulation(simulation).await?;
                CardConnectionLocator::Simulation {
                    simulation: summary.simulation_ref(),
                    owned: false,
                }
            }
            CardConnectionTarget::StartSimulation(project) => {
                let summary = self.start_simulation(project).await?;
                if summary.status != SimulationStatus::Running {
                    let _ = self.stop_simulation(summary.simulation_ref()).await;
                    return Err(bootstrap::invalid_connection_target(format!(
                        "simulation `{}` is not running; current status is {:?}",
                        summary.simulation_id, summary.status
                    )));
                }
                CardConnectionLocator::Simulation {
                    simulation: summary.simulation_ref(),
                    owned: true,
                }
            }
        };
        Ok(CardConnection::new(self.clone(), locator))
    }
}

/// Convert a validated card-status response into the unified reader connection locator.
fn reader_connection_locator(
    status: &crate::types::CardStatusSummary,
) -> Result<CardConnectionLocator> {
    let reader_name = status.reader_name.trim().to_string();
    if reader_name.is_empty() {
        return Err(JcimSdkError::InvalidResponse(
            "service returned an empty reader name for an opened card connection".to_string(),
        ));
    }
    Ok(CardConnectionLocator::Reader { reader_name })
}

#[cfg(test)]
mod tests {
    use jcim_core::iso7816::IsoCapabilities;

    use super::*;

    #[test]
    fn reader_connection_locator_requires_non_empty_reader_names() {
        let error = reader_connection_locator(&crate::types::CardStatusSummary {
            reader_name: "   ".to_string(),
            card_present: true,
            atr: None,
            active_protocol: None,
            iso_capabilities: IsoCapabilities::default(),
            session_state: jcim_core::iso7816::IsoSessionState::default(),
            lines: Vec::new(),
        })
        .expect_err("empty reader names should fail");
        assert!(matches!(error, JcimSdkError::InvalidResponse(_)));
        assert!(error.to_string().contains("empty reader name"));
    }
}
