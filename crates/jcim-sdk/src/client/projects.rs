use std::path::{Path, PathBuf};

use jcim_api::v0_3::project_service_client::ProjectServiceClient;
use jcim_api::v0_3::{CleanProjectResponse, CreateProjectRequest};

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
        let project = ProjectServiceClient::new(self.channel.clone())
            .create_project(CreateProjectRequest {
                name: name.to_string(),
                directory: directory.as_ref().display().to_string(),
            })
            .await?
            .into_inner()
            .project
            .ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no project".to_string())
            })?;
        project_summary(project)
    }

    /// Load one project.
    pub async fn get_project(&self, project: &ProjectRef) -> Result<ProjectDetails> {
        let response = ProjectServiceClient::new(self.channel.clone())
            .get_project(project_selector(project))
            .await?
            .into_inner();
        Ok(ProjectDetails {
            project: project_summary(response.project.ok_or_else(|| {
                JcimSdkError::InvalidResponse("service returned no project".to_string())
            })?)?,
            manifest_toml: response.manifest_toml,
        })
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
