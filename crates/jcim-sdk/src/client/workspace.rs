use jcim_api::v0_3::workspace_service_client::WorkspaceServiceClient;
use jcim_api::v0_3::{Empty, GetOverviewResponse};

use crate::error::{JcimSdkError, Result};
use crate::types::{OverviewSummary, ProjectSummary, SimulationSummary};

use super::JcimClient;
use super::proto::{project_summary, simulation_summary};

impl JcimClient {
    /// Fetch a high-level overview of the local JCIM service state.
    pub async fn overview(&self) -> Result<OverviewSummary> {
        let response = WorkspaceServiceClient::new(self.channel.clone())
            .get_overview(Empty {})
            .await?
            .into_inner();
        overview_summary_from_response(response)
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

/// Decode one workspace overview response and require the returned overview payload.
fn overview_summary_from_response(response: GetOverviewResponse) -> Result<OverviewSummary> {
    let overview = response
        .overview
        .ok_or_else(|| JcimSdkError::InvalidResponse("service returned no overview".to_string()))?;
    Ok(OverviewSummary {
        known_project_count: overview.known_project_count,
        active_simulation_count: overview.active_simulation_count,
    })
}

#[cfg(test)]
mod tests {
    use jcim_api::v0_3::Overview;

    use super::*;

    #[test]
    fn overview_summary_from_response_requires_payload() {
        let error = overview_summary_from_response(GetOverviewResponse { overview: None })
            .expect_err("missing overview payload should fail");
        assert!(matches!(error, JcimSdkError::InvalidResponse(_)));
        assert!(error.to_string().contains("service returned no overview"));
    }

    #[test]
    fn overview_summary_from_response_decodes_counts() {
        let summary = overview_summary_from_response(GetOverviewResponse {
            overview: Some(Overview {
                known_project_count: 3,
                active_simulation_count: 2,
            }),
        })
        .expect("decode overview");

        assert_eq!(summary.known_project_count, 3);
        assert_eq!(summary.active_simulation_count, 2);
    }
}
