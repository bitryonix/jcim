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
        // `tonic` requires `Result<T, Status>` stream items here, so boxing the error would only
        // add noise around a transport-mandated signature.
        #[allow(clippy::result_large_err)]
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

#[cfg(test)]
mod tests {
    use tokio_stream::StreamExt;
    use tonic::Request;

    use jcim_api::v0_3::build_service_server::BuildService;

    use super::*;
    use crate::rpc::testsupport::{create_demo_project, load_rpc, project_selector, temp_root};

    #[tokio::test]
    async fn build_rpc_maps_build_artifacts_and_event_streams() {
        let root = temp_root("build");
        let rpc = load_rpc(&root);
        let project_root = create_demo_project(&rpc, &root, "Demo");

        let built = BuildService::build_project(
            &rpc,
            Request::new(BuildProjectRequest {
                project: Some(project_selector(&project_root)),
            }),
        )
        .await
        .expect("build project")
        .into_inner();
        let artifacts =
            BuildService::get_artifacts(&rpc, Request::new(project_selector(&project_root)))
                .await
                .expect("get artifacts")
                .into_inner();
        let mut stream =
            BuildService::stream_build_events(&rpc, Request::new(project_selector(&project_root)))
                .await
                .expect("stream build events")
                .into_inner();
        let first_event = stream
            .next()
            .await
            .expect("first event")
            .expect("build event");

        assert!(built.project.is_some());
        assert!(!built.artifacts.is_empty());
        assert_eq!(artifacts.artifacts.len(), built.artifacts.len());
        assert!(!first_event.project_id.is_empty());
        assert!(!first_event.message.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }
}
