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
