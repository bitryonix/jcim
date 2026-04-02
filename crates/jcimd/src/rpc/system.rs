use tonic::{Request, Response, Status};

use jcim_api::v0_3::system_service_server::SystemService;
use jcim_api::v0_3::{
    DoctorResponse, Empty, GetServiceStatusResponse, SetupToolchainsRequest,
    SetupToolchainsResponse,
};

use super::LocalRpc;
use crate::blocking::blocking;
use crate::translate::service_status_response;

#[tonic::async_trait]
impl SystemService for LocalRpc {
    async fn setup_toolchains(
        &self,
        request: Request<SetupToolchainsRequest>,
    ) -> Result<Response<SetupToolchainsResponse>, Status> {
        let request = request.into_inner();
        let java_bin = (!request.java_bin.is_empty()).then_some(request.java_bin);
        let app = self.app.clone();
        let setup = blocking(move || app.setup_toolchains(java_bin.as_deref())).await?;
        Ok(Response::new(SetupToolchainsResponse {
            config_path: setup.config_path.display().to_string(),
            message: setup.message,
        }))
    }

    async fn doctor(&self, _request: Request<Empty>) -> Result<Response<DoctorResponse>, Status> {
        let app = self.app.clone();
        let lines = blocking(move || app.doctor()).await?;
        Ok(Response::new(DoctorResponse { lines }))
    }

    async fn get_service_status(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<GetServiceStatusResponse>, Status> {
        let app = self.app.clone();
        let status = blocking(move || app.service_status()).await?;
        Ok(Response::new(service_status_response(status)))
    }
}

#[cfg(test)]
mod tests {
    use tonic::Request;

    use jcim_api::v0_3::system_service_server::SystemService;

    use super::*;
    use crate::rpc::testsupport::{create_demo_project, load_rpc, temp_root};

    #[tokio::test]
    async fn system_rpc_maps_setup_doctor_and_service_status() {
        let root = temp_root("system");
        let rpc = load_rpc(&root);
        let _project_root = create_demo_project(&rpc, &root, "Demo");

        let setup = SystemService::setup_toolchains(
            &rpc,
            Request::new(SetupToolchainsRequest {
                java_bin: "/custom/java".to_string(),
            }),
        )
        .await
        .expect("setup toolchains")
        .into_inner();
        let doctor = SystemService::doctor(&rpc, Request::new(Empty {}))
            .await
            .expect("doctor")
            .into_inner();
        let status = SystemService::get_service_status(&rpc, Request::new(Empty {}))
            .await
            .expect("service status")
            .into_inner();

        assert!(setup.config_path.ends_with("config.toml"));
        assert!(setup.message.contains("saved machine-local JCIM settings"));
        assert!(
            doctor
                .lines
                .iter()
                .any(|line| line.starts_with("Effective Java runtime: "))
        );
        assert!(status.running);
        assert_eq!(status.known_project_count, 1);
        assert!(status.socket_path.ends_with(".sock"));

        let _ = std::fs::remove_dir_all(root);
    }
}
