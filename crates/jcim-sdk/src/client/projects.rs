use std::path::{Path, PathBuf};

use jcim_api::v0_3::project_service_client::ProjectServiceClient;
use jcim_api::v0_3::{
    CleanProjectResponse, CreateProjectRequest, CreateProjectResponse, GetProjectResponse,
};

use crate::error::{JcimSdkError, Result};
use crate::types::{ProjectDetails, ProjectRef, ProjectSummary, owned_path};

use super::JcimClient;
use super::proto::{project_selector, project_summary};

impl JcimClient {
    /// Create and register one project skeleton.
    pub async fn create_project(
        &self,
        name: &str,
        directory: impl AsRef<Path>,
    ) -> Result<ProjectSummary> {
        let response = ProjectServiceClient::new(self.channel.clone())
            .create_project(CreateProjectRequest {
                name: name.to_string(),
                directory: directory.as_ref().display().to_string(),
            })
            .await?
            .into_inner();
        created_project_summary(response)
    }

    /// Load one project.
    pub async fn get_project(&self, project: &ProjectRef) -> Result<ProjectDetails> {
        let response = ProjectServiceClient::new(self.channel.clone())
            .get_project(project_selector(project))
            .await?
            .into_inner();
        project_details_from_response(response)
    }

    /// Clean one project's generated local state.
    pub async fn clean_project(&self, project: &ProjectRef) -> Result<PathBuf> {
        let CleanProjectResponse { cleaned_path } = ProjectServiceClient::new(self.channel.clone())
            .clean_project(project_selector(project))
            .await?
            .into_inner();
        Ok(owned_path(cleaned_path))
    }
}

/// Decode one project-creation response and require the returned project payload.
fn created_project_summary(response: CreateProjectResponse) -> Result<ProjectSummary> {
    let project = response
        .project
        .ok_or_else(|| JcimSdkError::InvalidResponse("service returned no project".to_string()))?;
    project_summary(project)
}

/// Decode one project-details response and require the returned project payload.
fn project_details_from_response(response: GetProjectResponse) -> Result<ProjectDetails> {
    Ok(ProjectDetails {
        project: project_summary(response.project.ok_or_else(|| {
            JcimSdkError::InvalidResponse("service returned no project".to_string())
        })?)?,
        manifest_toml: response.manifest_toml,
    })
}

#[cfg(test)]
mod tests {
    use jcim_api::v0_3::{AppletInfo, ProjectInfo};

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
    fn created_project_summary_requires_project_payload() {
        let error = created_project_summary(CreateProjectResponse { project: None })
            .expect_err("missing project payload should fail");
        assert!(matches!(error, JcimSdkError::InvalidResponse(_)));
        assert!(error.to_string().contains("service returned no project"));
    }

    #[test]
    fn project_details_from_response_decodes_project_and_manifest() {
        let details = project_details_from_response(GetProjectResponse {
            project: Some(sample_project()),
            manifest_toml: "[project]\nname = \"Demo\"\n".to_string(),
        })
        .expect("decode project details");

        assert_eq!(details.project.project_id, "project-1");
        assert!(details.manifest_toml.contains("[project]"));
    }
}
