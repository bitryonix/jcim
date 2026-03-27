use tonic::{Request, Response, Status};

use jcim_api::v0_3::workspace_service_server::WorkspaceService;
use jcim_api::v0_3::{
    Empty, GetOverviewResponse, ListProjectsResponse, ListSimulationsResponse, Overview,
};

use super::LocalRpc;
use crate::blocking::blocking;
use crate::translate::{project_info, simulation_info};

#[tonic::async_trait]
impl WorkspaceService for LocalRpc {
    async fn get_overview(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<GetOverviewResponse>, Status> {
        let app = self.app.clone();
        let overview = blocking(move || app.overview()).await?;
        Ok(Response::new(GetOverviewResponse {
            overview: Some(Overview {
                known_project_count: overview.known_project_count,
                active_simulation_count: overview.active_simulation_count,
            }),
        }))
    }

    async fn list_projects(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListProjectsResponse>, Status> {
        let app = self.app.clone();
        let projects = blocking(move || app.list_projects()).await?;
        Ok(Response::new(ListProjectsResponse {
            projects: projects.into_iter().map(project_info).collect(),
        }))
    }

    async fn list_simulations(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListSimulationsResponse>, Status> {
        let app = self.app.clone();
        let simulations = blocking(move || app.list_simulations()).await?;
        Ok(Response::new(ListSimulationsResponse {
            simulations: simulations.into_iter().map(simulation_info).collect(),
        }))
    }
}
