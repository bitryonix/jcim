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
