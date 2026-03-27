//! Service bootstrap and lifecycle methods for the JCIM SDK.

#![allow(clippy::missing_docs_in_private_items)]

mod bootstrap;
mod build;
mod cards;
mod projects;
mod proto;
mod simulations;
mod system;
mod workspace;

use tonic::transport::Channel;

use jcim_config::project::ManagedPaths;

use crate::connection::CardConnection;
use crate::error::{JcimSdkError, Result};
use crate::types::{CardConnectionLocator, CardConnectionTarget, SimulationStatus};

/// Canonical async Rust client for the local JCIM service.
#[derive(Clone)]
pub struct JcimClient {
    managed_paths: ManagedPaths,
    channel: Channel,
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
                let reader_name = status.reader_name.trim().to_string();
                if reader_name.is_empty() {
                    return Err(JcimSdkError::InvalidResponse(
                        "service returned an empty reader name for an opened card connection"
                            .to_string(),
                    ));
                }
                CardConnectionLocator::Reader { reader_name }
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
