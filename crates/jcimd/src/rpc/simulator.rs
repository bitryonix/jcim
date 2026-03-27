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
