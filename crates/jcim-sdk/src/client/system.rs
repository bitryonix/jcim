use jcim_api::v0_3::system_service_client::SystemServiceClient;
use jcim_api::v0_3::{
    DoctorResponse, Empty, GetServiceStatusResponse, SetupToolchainsRequest,
    SetupToolchainsResponse,
};

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
        Ok(setup_summary_from_response(response))
    }

    /// Return a doctor report for the current environment.
    pub async fn doctor(&self) -> Result<Vec<String>> {
        Ok(doctor_lines_from_response(
            SystemServiceClient::new(self.channel.clone())
                .doctor(Empty {})
                .await?
                .into_inner(),
        ))
    }

    /// Return current service status without mutating service state.
    pub async fn service_status(&self) -> Result<ServiceStatusSummary> {
        let response = SystemServiceClient::new(self.channel.clone())
            .get_service_status(Empty {})
            .await?
            .into_inner();
        service_status_summary_from_response(response)
    }
}

/// Decode one setup response into the stable SDK setup summary.
fn setup_summary_from_response(response: SetupToolchainsResponse) -> SetupSummary {
    setup_summary(response)
}

/// Extract the stable doctor line list from one system doctor response.
fn doctor_lines_from_response(response: DoctorResponse) -> Vec<String> {
    response.lines
}

/// Decode one service-status response into the stable SDK summary type.
fn service_status_summary_from_response(
    response: GetServiceStatusResponse,
) -> Result<ServiceStatusSummary> {
    service_status_summary(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_and_doctor_helpers_preserve_service_response_fields() {
        let setup = setup_summary_from_response(SetupToolchainsResponse {
            config_path: "/tmp/jcim/config/config.toml".to_string(),
            message: "saved machine-local JCIM settings".to_string(),
        });
        assert_eq!(
            setup.config_path,
            std::path::PathBuf::from("/tmp/jcim/config/config.toml")
        );
        assert_eq!(setup.message, "saved machine-local JCIM settings");

        let doctor = doctor_lines_from_response(DoctorResponse {
            lines: vec![
                "Managed data root: /tmp/jcim".to_string(),
                "Effective Java runtime: /usr/bin/java (configured)".to_string(),
            ],
        });
        assert_eq!(doctor.len(), 2);
        assert!(doctor[0].starts_with("Managed data root: "));
    }

    #[test]
    fn service_status_helper_decodes_wrapper_response() {
        let summary = service_status_summary_from_response(GetServiceStatusResponse {
            socket_path: "/tmp/jcim/run/jcimd.sock".to_string(),
            running: true,
            known_project_count: 2,
            active_simulation_count: 1,
            service_binary_path: "/tmp/bin/jcimd".to_string(),
            service_binary_fingerprint: "fingerprint".to_string(),
        })
        .expect("decode service status");

        assert!(summary.running);
        assert_eq!(summary.known_project_count, 2);
        assert_eq!(summary.active_simulation_count, 1);
        assert_eq!(
            summary.socket_path,
            std::path::PathBuf::from("/tmp/jcim/run/jcimd.sock")
        );
        assert_eq!(
            summary.service_binary_path,
            std::path::PathBuf::from("/tmp/bin/jcimd")
        );
        assert_eq!(summary.service_binary_fingerprint, "fingerprint");
    }
}
