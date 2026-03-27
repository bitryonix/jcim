use jcim_api::v0_3::system_service_client::SystemServiceClient;
use jcim_api::v0_3::{Empty, SetupToolchainsRequest};

use crate::error::Result;
use crate::types::{ServiceStatusSummary, SetupSummary};

use super::JcimClient;
use super::proto::{service_status_summary, setup_summary};

impl JcimClient {
    /// Persist machine-local toolchain settings.
    pub async fn setup_toolchains(&self, java_bin: Option<&str>) -> Result<SetupSummary> {
        let response = SystemServiceClient::new(self.channel.clone())
            .setup_toolchains(SetupToolchainsRequest {
                java_bin: java_bin.unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(setup_summary(response))
    }

    /// Return a doctor report for the current environment.
    pub async fn doctor(&self) -> Result<Vec<String>> {
        Ok(SystemServiceClient::new(self.channel.clone())
            .doctor(Empty {})
            .await?
            .into_inner()
            .lines)
    }

    /// Return current service status without mutating service state.
    pub async fn service_status(&self) -> Result<ServiceStatusSummary> {
        let response = SystemServiceClient::new(self.channel.clone())
            .get_service_status(Empty {})
            .await?
            .into_inner();
        service_status_summary(response)
    }
}
