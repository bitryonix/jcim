use jcim_api::v0_3::BuildProjectRequest;
use jcim_api::v0_3::build_service_client::BuildServiceClient;

use crate::error::{JcimSdkError, Result};
use crate::types::{ArtifactSummary, BuildSummary, ProjectRef};

use super::JcimClient;
use super::proto::{artifact_summary, project_selector, project_summary};

impl JcimClient {
    /// Build one project and return the current artifact set.
    pub async fn build_project(&self, project: &ProjectRef) -> Result<BuildSummary> {
        let response = BuildServiceClient::new(self.channel.clone())
            .build_project(BuildProjectRequest {
                project: Some(project_selector(project)),
            })
            .await?
            .into_inner();
        let artifacts = response
            .artifacts
            .into_iter()
            .map(artifact_summary)
            .collect::<Result<Vec<_>>>()?;
        Ok(BuildSummary {
            project: project_summary(response.project.ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no project".to_string())
            })?)?,
            artifacts,
            rebuilt: response.rebuilt,
        })
    }

    /// Return the current recorded artifact set for one project without rebuilding it.
    pub async fn get_artifacts(&self, project: &ProjectRef) -> Result<Vec<ArtifactSummary>> {
        let response = BuildServiceClient::new(self.channel.clone())
            .get_artifacts(project_selector(project))
            .await?
            .into_inner();
        response
            .artifacts
            .into_iter()
            .map(artifact_summary)
            .collect()
    }
}
