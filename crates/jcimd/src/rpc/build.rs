use tonic::{Request, Response, Status};

use jcim_api::v0_3::build_service_server::BuildService;
use jcim_api::v0_3::{
    BuildEvent, BuildProjectRequest, BuildProjectResponse, GetArtifactsResponse, ProjectSelector,
};

use super::{LocalRpc, RpcStream};
use crate::blocking::blocking;
use crate::translate::{artifact_info, into_project_selector, project_info};

#[tonic::async_trait]
impl BuildService for LocalRpc {
    type StreamBuildEventsStream = RpcStream<BuildEvent>;

    async fn build_project(
        &self,
        request: Request<BuildProjectRequest>,
    ) -> Result<Response<BuildProjectResponse>, Status> {
        let selector = into_project_selector(request.into_inner().project.unwrap_or_default());
        let app = self.app.clone();
        let (project, artifacts, rebuilt) = blocking(move || app.build_project(&selector)).await?;
        Ok(Response::new(BuildProjectResponse {
            project: Some(project_info(project)),
            artifacts: artifacts.into_iter().map(artifact_info).collect(),
            rebuilt,
        }))
    }

    async fn get_artifacts(
        &self,
        request: Request<ProjectSelector>,
    ) -> Result<Response<GetArtifactsResponse>, Status> {
        let selector = into_project_selector(request.into_inner());
        let app = self.app.clone();
        let (project, artifacts) = blocking(move || app.get_artifacts(&selector)).await?;
        Ok(Response::new(GetArtifactsResponse {
            project: Some(project_info(project)),
            artifacts: artifacts.into_iter().map(artifact_info).collect(),
        }))
    }

    async fn stream_build_events(
        &self,
        request: Request<ProjectSelector>,
    ) -> Result<Response<Self::StreamBuildEventsStream>, Status> {
        let selector = into_project_selector(request.into_inner());
        let app = self.app.clone();
        let project = blocking({
            let app = app.clone();
            let selector = selector.clone();
            move || app.get_project(&selector)
        })
        .await?;
        let events = blocking(move || app.build_events(&selector)).await?;
        let project_id = project.project.project_id;
        let stream = tokio_stream::iter(events.into_iter().map(move |event| {
            Ok(BuildEvent {
                project_id: project_id.clone(),
                level: event.level,
                message: event.message,
            })
        }));
        Ok(Response::new(Box::pin(stream)))
    }
}
