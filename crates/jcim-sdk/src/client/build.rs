use jcim_api::v0_3::build_service_client::BuildServiceClient;
use jcim_api::v0_3::{BuildProjectRequest, BuildProjectResponse};

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
        build_summary_from_response(response)
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

/// Decode one build response and fail closed when the service omits the project payload.
fn build_summary_from_response(response: BuildProjectResponse) -> Result<BuildSummary> {
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

#[cfg(test)]
mod tests {
    use jcim_api::v0_3::{AppletInfo, Artifact, ProjectInfo};

    use super::*;

    fn sample_project() -> ProjectInfo {
        ProjectInfo {
            project_id: "project-1".to_string(),
            name: "Demo".to_string(),
            project_path: "/tmp/demo".to_string(),
            profile: "classic304".to_string(),
            build_kind: "native".to_string(),
            package_name: "com.jcim.demo".to_string(),
            package_aid: "F000000001".to_string(),
            applets: vec![AppletInfo {
                class_name: "com.jcim.demo.DemoApplet".to_string(),
                aid: "F00000000101".to_string(),
            }],
        }
    }

    #[test]
    fn build_summary_from_response_requires_project_payload() {
        let error = build_summary_from_response(BuildProjectResponse {
            project: None,
            artifacts: Vec::new(),
            rebuilt: false,
        })
        .expect_err("missing project payload should fail");
        assert!(matches!(error, JcimSdkError::InvalidResponse(_)));
        assert!(error.to_string().contains("service returned no project"));
    }

    #[test]
    fn build_summary_from_response_decodes_artifacts() {
        let summary = build_summary_from_response(BuildProjectResponse {
            project: Some(sample_project()),
            artifacts: vec![Artifact {
                kind: "cap".to_string(),
                path: "/tmp/demo/.jcim/build/demo.cap".to_string(),
            }],
            rebuilt: true,
        })
        .expect("decode build summary");

        assert_eq!(summary.project.project_id, "project-1");
        assert_eq!(summary.artifacts.len(), 1);
        assert!(summary.rebuilt);
    }
}
