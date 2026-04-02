use jcim_api::v0_3::simulator_service_client::SimulatorServiceClient;
use jcim_api::v0_3::{
    GetSimulationResponse, ManageChannelRequest, ManageChannelResponse, OpenGpSecureChannelRequest,
    SecureMessagingAdvanceRequest, SecureMessagingRequest, SecureMessagingResponse,
    StartSimulationRequest, StartSimulationResponse, StopSimulationResponse, TransmitApduRequest,
    TransmitRawApduRequest, TransmitRawApduResponse,
};

use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::iso7816::{IsoSessionState, SecureMessagingProtocol};
use jcim_core::{globalplatform, iso7816};

use crate::error::{JcimSdkError, Result};
use crate::types::{
    ApduExchangeSummary, EventLine, GpSecureChannelSummary, ManageChannelSummary, ProjectRef,
    ResetSummary, SecureMessagingSummary, SimulationRef, SimulationStatus, SimulationSummary,
};

use super::JcimClient;
use super::bootstrap::invalid_connection_target;
use super::proto::{
    command_apdu_frame, gp_secure_channel_from_proto, iso_session_state_from_proto,
    project_selector, reset_summary_from_simulation_proto, response_apdu_from_proto,
    secure_messaging_protocol_fields, simulation_selector, simulation_summary,
};

impl JcimClient {
    /// Start one simulation from a JCIM project.
    pub async fn start_simulation(&self, project: ProjectRef) -> Result<SimulationSummary> {
        let request = StartSimulationRequest {
            project: Some(project_selector(&project)),
        };
        let response = SimulatorServiceClient::new(self.channel.clone())
            .start_simulation(request)
            .await?
            .into_inner();
        started_simulation_summary(response)
    }

    /// Get one simulation by id.
    pub async fn get_simulation(&self, simulation: SimulationRef) -> Result<SimulationSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .get_simulation(simulation_selector(simulation.simulation_id))
            .await?
            .into_inner();
        fetched_simulation_summary(response)
    }

    /// Stop one simulation.
    pub async fn stop_simulation(&self, simulation: SimulationRef) -> Result<SimulationSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .stop_simulation(simulation_selector(simulation.simulation_id))
            .await?
            .into_inner();
        stopped_simulation_summary(response)
    }

    /// Return retained simulation event lines.
    pub async fn simulation_events(&self, simulation: SimulationRef) -> Result<Vec<EventLine>> {
        let mut stream = SimulatorServiceClient::new(self.channel.clone())
            .stream_simulation_events(simulation_selector(simulation.simulation_id))
            .await?
            .into_inner();
        let mut events = Vec::new();
        while let Some(event) = stream.message().await? {
            events.push(EventLine {
                level: event.level,
                message: event.message,
            });
        }
        Ok(events)
    }

    /// Send one APDU to a running simulation.
    pub async fn transmit_sim_apdu(
        &self,
        simulation: SimulationRef,
        apdu: &CommandApdu,
    ) -> Result<ResponseApdu> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .transmit_apdu(TransmitApduRequest {
                simulation: Some(simulation_selector(simulation.simulation_id)),
                command: Some(command_apdu_frame(apdu)),
            })
            .await?
            .into_inner()
            .response;
        response_apdu_from_proto(response)
    }

    /// Fetch the current tracked ISO/IEC 7816 session state for one running simulation.
    pub async fn get_simulation_session_state(
        &self,
        simulation: SimulationRef,
    ) -> Result<IsoSessionState> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .get_session_state(simulation_selector(simulation.simulation_id))
            .await?
            .into_inner();
        iso_session_state_from_proto(response.session_state)
    }

    /// Send one raw APDU byte sequence to a running simulation.
    pub async fn transmit_raw_sim_apdu(
        &self,
        simulation: SimulationRef,
        apdu: &[u8],
    ) -> Result<ApduExchangeSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .transmit_raw_apdu(TransmitRawApduRequest {
                simulation: Some(simulation_selector(simulation.simulation_id)),
                apdu: apdu.to_vec(),
            })
            .await?
            .into_inner();
        raw_simulation_exchange_summary(response)
    }

    /// Open or close one logical channel on a running simulation.
    pub async fn manage_simulation_channel(
        &self,
        simulation: SimulationRef,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<ManageChannelSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .manage_channel(ManageChannelRequest {
                simulation: Some(simulation_selector(simulation.simulation_id)),
                open,
                channel_number: channel_number.map(u32::from),
            })
            .await?
            .into_inner();
        simulation_manage_channel_summary(response)
    }

    /// Mark secure messaging as active for one running simulation.
    pub async fn open_simulation_secure_messaging(
        &self,
        simulation: SimulationRef,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<SecureMessagingSummary> {
        let (protocol, protocol_label) = secure_messaging_protocol_fields(protocol.as_ref());
        let response = SimulatorServiceClient::new(self.channel.clone())
            .open_secure_messaging(SecureMessagingRequest {
                simulation: Some(simulation_selector(simulation.simulation_id)),
                protocol,
                security_level: security_level.map(u32::from),
                session_id: session_id.unwrap_or_default(),
                protocol_label,
            })
            .await?
            .into_inner();
        simulation_secure_messaging_summary(response)
    }

    /// Advance the secure-messaging command counter for one running simulation.
    pub async fn advance_simulation_secure_messaging(
        &self,
        simulation: SimulationRef,
        increment_by: u32,
    ) -> Result<SecureMessagingSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .advance_secure_messaging(SecureMessagingAdvanceRequest {
                simulation: Some(simulation_selector(simulation.simulation_id)),
                increment_by,
            })
            .await?
            .into_inner();
        simulation_secure_messaging_summary(response)
    }

    /// Clear the tracked secure-messaging state for one running simulation.
    pub async fn close_simulation_secure_messaging(
        &self,
        simulation: SimulationRef,
    ) -> Result<SecureMessagingSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .close_secure_messaging(simulation_selector(simulation.simulation_id))
            .await?
            .into_inner();
        simulation_secure_messaging_summary(response)
    }

    /// Open one typed GP secure channel on a running simulation.
    pub async fn open_gp_secure_channel_on_simulation(
        &self,
        simulation: SimulationRef,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
    ) -> Result<GpSecureChannelSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .open_gp_secure_channel(OpenGpSecureChannelRequest {
                simulation: Some(simulation_selector(simulation.simulation_id)),
                keyset_name: keyset_name.unwrap_or_default().to_string(),
                security_level: security_level.map(u32::from),
            })
            .await?
            .into_inner();
        gp_secure_channel_from_proto(response.secure_channel)
    }

    /// Close one typed GP secure channel on a running simulation.
    pub async fn close_gp_secure_channel_on_simulation(
        &self,
        simulation: SimulationRef,
    ) -> Result<SecureMessagingSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .close_gp_secure_channel(simulation_selector(simulation.simulation_id))
            .await?
            .into_inner();
        simulation_secure_messaging_summary(response)
    }

    /// Send one ISO/IEC 7816 `SELECT` by application identifier to a running simulation.
    pub async fn iso_select_application_on_simulation(
        &self,
        simulation: SimulationRef,
        aid: &Aid,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(simulation, &iso7816::select_by_name(aid))
            .await
    }

    /// Send one GlobalPlatform `SELECT` for the issuer security domain to a running simulation.
    pub async fn gp_select_issuer_security_domain_on_simulation(
        &self,
        simulation: SimulationRef,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(simulation, &globalplatform::select_issuer_security_domain())
            .await
    }

    /// Run one typed GlobalPlatform `GET STATUS` request against a running simulation.
    pub async fn gp_get_status_on_simulation(
        &self,
        simulation: SimulationRef,
        kind: globalplatform::RegistryKind,
        occurrence: globalplatform::GetStatusOccurrence,
    ) -> Result<globalplatform::GetStatusResponse> {
        let response = self
            .transmit_sim_apdu(simulation, &globalplatform::get_status(kind, occurrence))
            .await?;
        Ok(globalplatform::parse_get_status(kind, &response)?)
    }

    /// Set one GlobalPlatform card life cycle state inside a running simulation.
    pub async fn gp_set_card_status_on_simulation(
        &self,
        simulation: SimulationRef,
        state: globalplatform::CardLifeCycle,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(simulation, &globalplatform::set_card_status(state))
            .await
    }

    /// Lock or unlock one application inside a running simulation.
    pub async fn gp_set_application_status_on_simulation(
        &self,
        simulation: SimulationRef,
        aid: &Aid,
        transition: globalplatform::LockTransition,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(
            simulation,
            &globalplatform::set_application_status(aid, transition),
        )
        .await
    }

    /// Lock or unlock one security domain and its applications inside a running simulation.
    pub async fn gp_set_security_domain_status_on_simulation(
        &self,
        simulation: SimulationRef,
        aid: &Aid,
        transition: globalplatform::LockTransition,
    ) -> Result<ResponseApdu> {
        self.transmit_sim_apdu(
            simulation,
            &globalplatform::set_security_domain_status(aid, transition),
        )
        .await
    }

    /// Reset one running simulation and return the typed reset summary.
    pub async fn reset_simulation_summary(
        &self,
        simulation: SimulationRef,
    ) -> Result<ResetSummary> {
        let response = SimulatorServiceClient::new(self.channel.clone())
            .reset_simulation(simulation_selector(simulation.simulation_id))
            .await?
            .into_inner();
        reset_summary_from_simulation_proto(response)
    }

    /// Fetch one simulation summary and reject empty ids or non-running simulation targets for connections.
    pub(super) async fn validated_running_simulation(
        &self,
        simulation: SimulationRef,
    ) -> Result<SimulationSummary> {
        if simulation.simulation_id.trim().is_empty() {
            return Err(invalid_connection_target(
                "simulation connection requires a non-empty simulation id".to_string(),
            ));
        }
        let summary = self.get_simulation(simulation).await?;
        if summary.status != SimulationStatus::Running {
            return Err(invalid_connection_target(format!(
                "simulation `{}` is not running; current status is {:?}",
                summary.simulation_id, summary.status
            )));
        }
        Ok(summary)
    }
}

/// Decode one optional simulation payload and fail closed when the service omits it.
fn simulation_from_proto(
    simulation: Option<jcim_api::v0_3::SimulationInfo>,
) -> Result<SimulationSummary> {
    simulation
        .ok_or_else(|| JcimSdkError::InvalidResponse("service returned no simulation".to_string()))
        .and_then(simulation_summary)
}

/// Decode one simulation-start response into the stable SDK simulation summary.
fn started_simulation_summary(response: StartSimulationResponse) -> Result<SimulationSummary> {
    simulation_from_proto(response.simulation)
}

/// Decode one simulation-fetch response into the stable SDK simulation summary.
fn fetched_simulation_summary(response: GetSimulationResponse) -> Result<SimulationSummary> {
    simulation_from_proto(response.simulation)
}

/// Decode one simulation-stop response into the stable SDK simulation summary.
fn stopped_simulation_summary(response: StopSimulationResponse) -> Result<SimulationSummary> {
    simulation_from_proto(response.simulation)
}

/// Decode one raw simulation APDU exchange response into the unified SDK summary type.
fn raw_simulation_exchange_summary(
    response: TransmitRawApduResponse,
) -> Result<ApduExchangeSummary> {
    Ok(ApduExchangeSummary {
        command: CommandApdu::parse(&response.apdu)?,
        response: response_apdu_from_proto(response.response)?,
        session_state: iso_session_state_from_proto(response.session_state)?,
    })
}

/// Decode one simulation manage-channel response into the unified SDK summary type.
fn simulation_manage_channel_summary(
    response: ManageChannelResponse,
) -> Result<ManageChannelSummary> {
    Ok(ManageChannelSummary {
        channel_number: response.channel_number.map(|value| value as u8),
        response: response_apdu_from_proto(response.response)?,
        session_state: iso_session_state_from_proto(response.session_state)?,
    })
}

/// Decode one simulation secure-messaging response into the unified SDK summary type.
fn simulation_secure_messaging_summary(
    response: SecureMessagingResponse,
) -> Result<SecureMessagingSummary> {
    Ok(SecureMessagingSummary {
        session_state: iso_session_state_from_proto(response.session_state)?,
    })
}

#[cfg(test)]
mod tests {
    use jcim_api::v0_3::{
        IsoCapabilitiesInfo, IsoSessionStateInfo, ResponseApduFrame, SecureMessagingStateInfo,
        SimulationInfo, SimulationStatus as ProtoSimulationStatus,
    };

    use super::*;

    fn sample_simulation() -> SimulationInfo {
        SimulationInfo {
            simulation_id: "sim-1".to_string(),
            project_id: "project-1".to_string(),
            project_path: "/tmp/demo".to_string(),
            status: ProtoSimulationStatus::Running as i32,
            reader_name: "Reader".to_string(),
            health: "ready".to_string(),
            atr: None,
            active_protocol: None,
            iso_capabilities: Some(IsoCapabilitiesInfo::default()),
            session_state: Some(IsoSessionStateInfo::default()),
            package_count: 1,
            applet_count: 1,
            package_name: "com.jcim.demo".to_string(),
            package_aid: "F000000001".to_string(),
            recent_events: Vec::new(),
        }
    }

    #[test]
    fn simulation_responses_require_payloads() {
        let error = started_simulation_summary(StartSimulationResponse { simulation: None })
            .expect_err("missing simulation payload should fail");
        assert!(matches!(error, JcimSdkError::InvalidResponse(_)));
        assert!(error.to_string().contains("service returned no simulation"));
    }

    #[test]
    fn fetched_simulation_summary_decodes_simulation_payload() {
        let summary = fetched_simulation_summary(GetSimulationResponse {
            simulation: Some(sample_simulation()),
        })
        .expect("decode simulation");

        assert_eq!(summary.simulation_id, "sim-1");
        assert_eq!(summary.status, SimulationStatus::Running);
    }

    #[test]
    fn simulation_manage_channel_summary_decodes_response_fields() {
        let summary = simulation_manage_channel_summary(ManageChannelResponse {
            channel_number: Some(2),
            response: Some(ResponseApduFrame {
                data: Vec::new(),
                sw: 0x9000,
                ..ResponseApduFrame::default()
            }),
            session_state: Some(IsoSessionStateInfo::default()),
        })
        .expect("decode manage channel summary");

        assert_eq!(summary.channel_number, Some(2));
        assert_eq!(summary.response.sw, 0x9000);
    }

    #[test]
    fn raw_simulation_exchange_summary_decodes_command_and_session_state() {
        let summary = raw_simulation_exchange_summary(TransmitRawApduResponse {
            apdu: vec![0x00, 0xA4, 0x04, 0x00, 0x00],
            response: Some(ResponseApduFrame {
                data: Vec::new(),
                sw: 0x9000,
                ..ResponseApduFrame::default()
            }),
            session_state: Some(IsoSessionStateInfo {
                secure_messaging: Some(SecureMessagingStateInfo::default()),
                ..IsoSessionStateInfo::default()
            }),
        })
        .expect("decode raw exchange");

        assert_eq!(summary.command.to_bytes(), [0x00, 0xA4, 0x04, 0x00, 0x00]);
        assert_eq!(summary.response.sw, 0x9000);
    }

    #[test]
    fn simulation_secure_messaging_summary_defaults_missing_session_state() {
        let summary = simulation_secure_messaging_summary(SecureMessagingResponse {
            session_state: None,
        })
        .expect("missing session state should decode to default");
        assert_eq!(summary.session_state, IsoSessionState::default());
    }
}
