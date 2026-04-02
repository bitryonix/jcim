use tonic::{Request, Response, Status};

use jcim_api::v0_3::simulator_service_server::SimulatorService;
use jcim_api::v0_3::{
    GetSimulationResponse, GetSimulationSessionStateResponse, ManageChannelRequest,
    ManageChannelResponse, OpenGpSecureChannelRequest, OpenGpSecureChannelResponse,
    ResetSimulationResponse, SecureMessagingAdvanceRequest, SecureMessagingRequest,
    SecureMessagingResponse, SimulationEvent, SimulationSelector, StartSimulationRequest,
    StartSimulationResponse, StopSimulationResponse, TransmitApduRequest, TransmitApduResponse,
    TransmitRawApduRequest, TransmitRawApduResponse,
};
use jcim_core::apdu::CommandApdu;

use super::{LocalRpc, RpcStream};
use crate::blocking::blocking;
use crate::translate::{
    atr_info, command_apdu_from_proto, gp_secure_channel_info, into_project_selector,
    into_simulation_selector, iso_session_state_info, response_apdu_frame,
    secure_messaging_protocol_from_proto, simulation_info, to_status,
};

#[tonic::async_trait]
impl SimulatorService for LocalRpc {
    type StreamSimulationEventsStream = RpcStream<SimulationEvent>;

    async fn start_simulation(
        &self,
        request: Request<StartSimulationRequest>,
    ) -> Result<Response<StartSimulationResponse>, Status> {
        let request = request.into_inner();
        let project = request.project.ok_or_else(|| {
            Status::invalid_argument("missing simulator input; provide a project selector")
        })?;
        let simulation = self
            .app
            .start_project_simulation(&into_project_selector(project))
            .await
            .map_err(to_status)?;
        Ok(Response::new(StartSimulationResponse {
            simulation: Some(simulation_info(simulation)),
        }))
    }

    async fn stop_simulation(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<StopSimulationResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let simulation = self
            .app
            .stop_simulation(&selector)
            .await
            .map_err(to_status)?;
        Ok(Response::new(StopSimulationResponse {
            simulation: Some(simulation_info(simulation)),
        }))
    }

    async fn get_simulation(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<GetSimulationResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let app = self.app.clone();
        let simulation = blocking(move || app.get_simulation(&selector)).await?;
        Ok(Response::new(GetSimulationResponse {
            simulation: Some(simulation_info(simulation)),
        }))
    }

    async fn stream_simulation_events(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<Self::StreamSimulationEventsStream>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let simulation_id = selector.simulation_id.clone();
        let app = self.app.clone();
        let events = blocking(move || app.simulation_events(&selector)).await?;
        // `tonic` requires `Result<T, Status>` stream items here, so boxing the error would only
        // add noise around a transport-mandated signature.
        #[allow(clippy::result_large_err)]
        let stream = tokio_stream::iter(events.into_iter().map(move |event| {
            Ok(SimulationEvent {
                simulation_id: simulation_id.clone(),
                level: event.level,
                message: event.message,
            })
        }));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn transmit_apdu(
        &self,
        request: Request<TransmitApduRequest>,
    ) -> Result<Response<TransmitApduResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let command = command_apdu_from_proto(request.command)?;
        let exchange = self
            .app
            .transmit_command(&selector, &command)
            .await
            .map_err(to_status)?;
        Ok(Response::new(TransmitApduResponse {
            response: Some(response_apdu_frame(&exchange.response)),
            session_state: Some(iso_session_state_info(&exchange.session_state)),
        }))
    }

    async fn transmit_raw_apdu(
        &self,
        request: Request<TransmitRawApduRequest>,
    ) -> Result<Response<TransmitRawApduResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let command = CommandApdu::parse(&request.apdu).map_err(to_status)?;
        let exchange = self
            .app
            .transmit_command(&selector, &command)
            .await
            .map_err(to_status)?;
        Ok(Response::new(TransmitRawApduResponse {
            apdu: request.apdu,
            response: Some(response_apdu_frame(&exchange.response)),
            session_state: Some(iso_session_state_info(&exchange.session_state)),
        }))
    }

    async fn get_session_state(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<GetSimulationSessionStateResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let session_state = self
            .app
            .simulation_session_state(&selector)
            .map_err(to_status)?;
        Ok(Response::new(GetSimulationSessionStateResponse {
            session_state: Some(iso_session_state_info(&session_state)),
        }))
    }

    async fn manage_channel(
        &self,
        request: Request<ManageChannelRequest>,
    ) -> Result<Response<ManageChannelResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let channel_number = request
            .channel_number
            .map(u8::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("channel number must fit in one byte"))?;
        let summary = self
            .app
            .manage_simulation_channel(&selector, request.open, channel_number)
            .await
            .map_err(to_status)?;
        Ok(Response::new(ManageChannelResponse {
            channel_number: summary.channel_number.map(u32::from),
            response: Some(response_apdu_frame(&summary.response)),
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn open_secure_messaging(
        &self,
        request: Request<SecureMessagingRequest>,
    ) -> Result<Response<SecureMessagingResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let summary = self
            .app
            .open_simulation_secure_messaging(
                &selector,
                secure_messaging_protocol_from_proto(request.protocol, &request.protocol_label),
                request
                    .security_level
                    .map(u8::try_from)
                    .transpose()
                    .map_err(|_| {
                        Status::invalid_argument("secure messaging level must fit in one byte")
                    })?,
                (!request.session_id.is_empty()).then_some(request.session_id),
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(SecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn advance_secure_messaging(
        &self,
        request: Request<SecureMessagingAdvanceRequest>,
    ) -> Result<Response<SecureMessagingResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let summary = self
            .app
            .advance_simulation_secure_messaging(&selector, request.increment_by)
            .await
            .map_err(to_status)?;
        Ok(Response::new(SecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn close_secure_messaging(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<SecureMessagingResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let summary = self
            .app
            .close_simulation_secure_messaging(&selector)
            .await
            .map_err(to_status)?;
        Ok(Response::new(SecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn open_gp_secure_channel(
        &self,
        request: Request<OpenGpSecureChannelRequest>,
    ) -> Result<Response<OpenGpSecureChannelResponse>, Status> {
        let request = request.into_inner();
        let selector = into_simulation_selector(request.simulation.unwrap_or_default());
        let summary = self
            .app
            .open_gp_secure_channel_on_simulation(
                &selector,
                (!request.keyset_name.is_empty()).then_some(request.keyset_name.as_str()),
                request
                    .security_level
                    .map(u8::try_from)
                    .transpose()
                    .map_err(|_| {
                        Status::invalid_argument("GP security level must fit in one byte")
                    })?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(OpenGpSecureChannelResponse {
            secure_channel: Some(gp_secure_channel_info(&summary)),
        }))
    }

    async fn close_gp_secure_channel(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<SecureMessagingResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let summary = self
            .app
            .close_gp_secure_channel_on_simulation(&selector)
            .await
            .map_err(to_status)?;
        Ok(Response::new(SecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn reset_simulation(
        &self,
        request: Request<SimulationSelector>,
    ) -> Result<Response<ResetSimulationResponse>, Status> {
        let selector = into_simulation_selector(request.into_inner());
        let summary = self
            .app
            .reset_simulation_summary(&selector)
            .await
            .map_err(to_status)?;
        Ok(Response::new(ResetSimulationResponse {
            atr: summary.atr.as_ref().map(atr_info),
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }
}

#[cfg(test)]
mod tests {
    use tokio_stream::StreamExt;
    use tonic::Request;

    use jcim_api::v0_3::simulator_service_server::SimulatorService;
    use jcim_core::{aid::Aid, iso7816};

    use super::*;
    use crate::rpc::testsupport::{
        acquire_local_service_lock, create_demo_project, load_rpc, project_selector,
        simulation_selector, temp_root,
    };

    #[tokio::test]
    async fn simulator_rpc_maps_runtime_happy_paths() {
        let _service_lock = acquire_local_service_lock();
        let root = temp_root("simulator");
        let rpc = load_rpc(&root);
        let project_root = create_demo_project(&rpc, &root, "Demo");
        let select_applet =
            iso7816::select_by_name(&Aid::from_hex("F00000000101").expect("default applet aid"))
                .to_bytes();

        let started = SimulatorService::start_simulation(
            &rpc,
            Request::new(StartSimulationRequest {
                project: Some(project_selector(&project_root)),
            }),
        )
        .await
        .expect("start simulation")
        .into_inner();
        let simulation = started.simulation.expect("simulation payload");
        let selector = simulation_selector(&simulation.simulation_id);

        let loaded = SimulatorService::get_simulation(&rpc, Request::new(selector.clone()))
            .await
            .expect("get simulation")
            .into_inner();
        let mut events =
            SimulatorService::stream_simulation_events(&rpc, Request::new(selector.clone()))
                .await
                .expect("stream simulation events")
                .into_inner();
        let first_event = events
            .next()
            .await
            .expect("first event")
            .expect("simulation event");
        let typed = SimulatorService::transmit_apdu(
            &rpc,
            Request::new(TransmitApduRequest {
                simulation: Some(selector.clone()),
                command: Some(jcim_api::v0_3::CommandApduFrame {
                    raw: select_applet.clone(),
                    ..jcim_api::v0_3::CommandApduFrame::default()
                }),
            }),
        )
        .await
        .expect("typed apdu")
        .into_inner();
        let raw = SimulatorService::transmit_raw_apdu(
            &rpc,
            Request::new(TransmitRawApduRequest {
                simulation: Some(selector.clone()),
                apdu: select_applet,
            }),
        )
        .await
        .expect("raw apdu")
        .into_inner();
        let session = SimulatorService::get_session_state(&rpc, Request::new(selector.clone()))
            .await
            .expect("get session state")
            .into_inner();
        let channel = SimulatorService::manage_channel(
            &rpc,
            Request::new(ManageChannelRequest {
                simulation: Some(selector.clone()),
                open: true,
                channel_number: None,
            }),
        )
        .await
        .expect("manage channel")
        .into_inner();
        let opened = SimulatorService::open_secure_messaging(
            &rpc,
            Request::new(SecureMessagingRequest {
                simulation: Some(selector.clone()),
                protocol: jcim_api::v0_3::SecureMessagingProtocol::Scp03 as i32,
                protocol_label: String::new(),
                security_level: Some(0x03),
                session_id: "rpc-sim-session".to_string(),
            }),
        )
        .await
        .expect("open secure messaging")
        .into_inner();
        let advanced = SimulatorService::advance_secure_messaging(
            &rpc,
            Request::new(SecureMessagingAdvanceRequest {
                simulation: Some(selector.clone()),
                increment_by: 1,
            }),
        )
        .await
        .expect("advance secure messaging")
        .into_inner();
        let closed =
            SimulatorService::close_gp_secure_channel(&rpc, Request::new(selector.clone()))
                .await
                .expect("close gp alias")
                .into_inner();
        let reset = SimulatorService::reset_simulation(&rpc, Request::new(selector.clone()))
            .await
            .expect("reset simulation")
            .into_inner();
        let stopped = SimulatorService::stop_simulation(&rpc, Request::new(selector))
            .await
            .expect("stop simulation")
            .into_inner();

        assert_eq!(
            loaded.simulation.expect("loaded simulation").simulation_id,
            simulation.simulation_id
        );
        assert!(!first_event.message.is_empty());
        assert_eq!(typed.response.expect("typed response").sw, 0x9000);
        assert_eq!(raw.response.expect("raw response").sw, 0x9000);
        assert!(session.session_state.is_some());
        assert_eq!(channel.channel_number, Some(1));
        assert!(opened.session_state.is_some());
        assert!(advanced.session_state.is_some());
        assert!(closed.session_state.is_some());
        assert!(reset.atr.is_some());
        assert_eq!(
            stopped.simulation.expect("stopped simulation").status,
            jcim_api::v0_3::SimulationStatus::Stopped as i32
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
