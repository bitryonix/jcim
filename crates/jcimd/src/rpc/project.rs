use std::path::Path;

use tonic::{Request, Response, Status};

use jcim_api::v0_3::project_service_server::ProjectService;
use jcim_api::v0_3::{
    CleanProjectResponse, CreateProjectRequest, CreateProjectResponse, GetProjectResponse,
    ProjectSelector,
};

use super::LocalRpc;
use crate::blocking::blocking;
use crate::translate::{into_project_selector, project_details_response, project_info};

#[tonic::async_trait]
impl ProjectService for LocalRpc {
    async fn create_project(
        &self,
        request: Request<CreateProjectRequest>,
    ) -> Result<Response<CreateProjectResponse>, Status> {
        let CreateProjectRequest { name, directory } = request.into_inner();
        let app = self.app.clone();
        let details = blocking(move || app.create_project(&name, Path::new(&directory))).await?;
        Ok(Response::new(CreateProjectResponse {
            project: Some(project_info(details.project)),
        }))
    }

    async fn get_project(
        &self,
        request: Request<ProjectSelector>,
    ) -> Result<Response<GetProjectResponse>, Status> {
        let selector = into_project_selector(request.into_inner());
        let app = self.app.clone();
        let details = blocking(move || app.get_project(&selector)).await?;
        Ok(Response::new(project_details_response(details)))
    }

    async fn clean_project(
        &self,
        request: Request<ProjectSelector>,
    ) -> Result<Response<CleanProjectResponse>, Status> {
        let selector = into_project_selector(request.into_inner());
        let app = self.app.clone();
        let cleaned_path = blocking(move || app.clean_project(&selector)).await?;
        Ok(Response::new(CleanProjectResponse {
            cleaned_path: cleaned_path.display().to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use tonic::Request;

    use jcim_api::v0_3::project_service_server::ProjectService;

    use super::*;
    use crate::rpc::testsupport::{load_rpc, project_selector, temp_root};

    #[tokio::test]
    async fn project_rpc_maps_create_get_and_clean_requests() {
        let root = temp_root("project");
        let rpc = load_rpc(&root);
        let project_root = root.join("demo");

        let created = ProjectService::create_project(
            &rpc,
            Request::new(CreateProjectRequest {
                name: "Demo".to_string(),
                directory: project_root.display().to_string(),
            }),
        )
        .await
        .expect("create project")
        .into_inner();
        let created_project = created.project.expect("project payload");

        let loaded =
            ProjectService::get_project(&rpc, Request::new(project_selector(&project_root)))
                .await
                .expect("get project")
                .into_inner();
        let cleaned =
            ProjectService::clean_project(&rpc, Request::new(project_selector(&project_root)))
                .await
                .expect("clean project")
                .into_inner();

        assert_eq!(created_project.name, "Demo");
        assert_eq!(
            loaded.project.expect("loaded project").project_id,
            created_project.project_id
        );
        assert!(loaded.manifest_toml.contains("[project]"));
        assert!(cleaned.cleaned_path.ends_with(".jcim"));

        let _ = std::fs::remove_dir_all(root);
    }
}
