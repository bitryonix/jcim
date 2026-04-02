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

#[cfg(test)]
mod tests {
    use tonic::Request;

    use jcim_api::v0_3::workspace_service_server::WorkspaceService;
    use jcim_app::{ProjectSelectorInput, SimulationSelectorInput};

    use super::*;
    use crate::rpc::testsupport::{
        acquire_local_service_lock, create_demo_project, load_rpc, temp_root,
    };

    #[tokio::test]
    async fn workspace_rpc_maps_overview_and_lists() {
        let _service_lock = acquire_local_service_lock();
        let root = temp_root("workspace");
        let rpc = load_rpc(&root);
        let project_root = create_demo_project(&rpc, &root, "Demo");
        let simulation = rpc
            .app
            .start_project_simulation(&ProjectSelectorInput {
                project_path: Some(project_root.clone()),
                project_id: None,
            })
            .await
            .expect("start simulation");

        let overview = WorkspaceService::get_overview(&rpc, Request::new(Empty {}))
            .await
            .expect("overview")
            .into_inner();
        let projects = WorkspaceService::list_projects(&rpc, Request::new(Empty {}))
            .await
            .expect("list projects")
            .into_inner();
        let simulations = WorkspaceService::list_simulations(&rpc, Request::new(Empty {}))
            .await
            .expect("list simulations")
            .into_inner();

        assert_eq!(
            overview
                .overview
                .expect("overview payload")
                .known_project_count,
            1
        );
        assert_eq!(projects.projects.len(), 1);
        assert_eq!(simulations.simulations.len(), 1);
        assert_eq!(
            simulations.simulations[0].simulation_id,
            simulation.simulation_id
        );

        let _ = rpc
            .app
            .stop_simulation(&SimulationSelectorInput {
                simulation_id: simulation.simulation_id,
            })
            .await;
        let _ = std::fs::remove_dir_all(root);
    }
}
