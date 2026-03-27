use jcim_api::v0_3::Empty;
use jcim_api::v0_3::workspace_service_client::WorkspaceServiceClient;

use crate::error::{JcimSdkError, Result};
use crate::types::{OverviewSummary, ProjectSummary, SimulationSummary};

use super::JcimClient;
use super::proto::{project_summary, simulation_summary};

impl JcimClient {
    /// Fetch a high-level overview of the local JCIM service state.
    pub async fn overview(&self) -> Result<OverviewSummary> {
        let overview = WorkspaceServiceClient::new(self.channel.clone())
            .get_overview(Empty {})
            .await?
            .into_inner()
            .overview
            .ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no overview".to_string())
            })?;
        Ok(OverviewSummary {
            known_project_count: overview.known_project_count,
            active_simulation_count: overview.active_simulation_count,
        })
    }

    /// List known projects.
    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let response = WorkspaceServiceClient::new(self.channel.clone())
            .list_projects(Empty {})
            .await?
            .into_inner();
        response.projects.into_iter().map(project_summary).collect()
    }

    /// List active simulations.
    pub async fn list_simulations(&self) -> Result<Vec<SimulationSummary>> {
        let response = WorkspaceServiceClient::new(self.channel.clone())
            .list_simulations(Empty {})
            .await?
            .into_inner();
        response
            .simulations
            .into_iter()
            .map(simulation_summary)
            .collect()
    }
}
