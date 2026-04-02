use std::path::Path;

use tonic::{Request, Response, Status};

use jcim_api::v0_3::card_service_server::CardService;
use jcim_api::v0_3::install_cap_request::Input as InstallCapInput;
use jcim_api::v0_3::{
    CardApduRequest, CardApduResponse, CardManageChannelRequest, CardManageChannelResponse,
    CardRawApduRequest, CardRawApduResponse, CardSecureMessagingAdvanceRequest,
    CardSecureMessagingRequest, CardSecureMessagingResponse, CardSelector, CardStatusRequest,
    CardStatusResponse, DeleteItemRequest, DeleteItemResponse, Empty, GetCardSessionStateResponse,
    InstallCapRequest, InstallCapResponse, ListAppletsRequest, ListAppletsResponse,
    ListPackagesRequest, ListPackagesResponse, ListReadersResponse, OpenCardGpSecureChannelRequest,
    OpenCardGpSecureChannelResponse, ReaderInfo, ResetCardRequest, ResetCardResponse,
};
use jcim_core::apdu::CommandApdu;

use super::LocalRpc;
use crate::translate::{
    applet_inventory_response, atr_info, command_apdu_from_proto, delete_item_response,
    gp_secure_channel_info, install_cap_response, iso_capabilities_info, iso_session_state_info,
    package_inventory_response, protocol_parameters_info, response_apdu_frame,
    secure_messaging_protocol_from_proto, to_status,
};

#[tonic::async_trait]
impl CardService for LocalRpc {
    async fn list_readers(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListReadersResponse>, Status> {
        let readers = self.app.list_readers().await.map_err(to_status)?;
        Ok(Response::new(ListReadersResponse {
            readers: readers
                .into_iter()
                .map(|reader| ReaderInfo {
                    name: reader.name,
                    card_present: reader.card_present,
                })
                .collect(),
        }))
    }

    async fn get_card_status(
        &self,
        request: Request<CardStatusRequest>,
    ) -> Result<Response<CardStatusResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let status = self
            .app
            .card_status(reader_name.as_deref())
            .await
            .map_err(to_status)?;
        Ok(Response::new(CardStatusResponse {
            reader_name: status.reader_name,
            card_present: status.card_present,
            atr: status.atr.as_ref().map(atr_info),
            active_protocol: status
                .active_protocol
                .as_ref()
                .map(protocol_parameters_info),
            iso_capabilities: Some(iso_capabilities_info(&status.iso_capabilities)),
            session_state: Some(iso_session_state_info(&status.session_state)),
            lines: status.lines,
        }))
    }

    async fn install_cap(
        &self,
        request: Request<InstallCapRequest>,
    ) -> Result<Response<InstallCapResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = match request.input {
            Some(InstallCapInput::Project(project)) => self
                .app
                .install_project_cap(
                    &crate::translate::into_project_selector(project),
                    reader_name.as_deref(),
                )
                .await
                .map_err(to_status)?,
            Some(InstallCapInput::CapPath(cap_path)) => self
                .app
                .install_cap_from_path(Path::new(&cap_path), reader_name.as_deref(), None)
                .await
                .map_err(to_status)?,
            None => {
                return Err(Status::invalid_argument(
                    "missing card install input; provide a project selector or CAP path",
                ));
            }
        };
        Ok(Response::new(install_cap_response(summary)))
    }

    async fn delete_item(
        &self,
        request: Request<DeleteItemRequest>,
    ) -> Result<Response<DeleteItemResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = self
            .app
            .delete_item(reader_name.as_deref(), &request.aid)
            .await
            .map_err(to_status)?;
        Ok(Response::new(delete_item_response(summary)))
    }

    async fn list_packages(
        &self,
        request: Request<ListPackagesRequest>,
    ) -> Result<Response<ListPackagesResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let inventory = self
            .app
            .list_packages((!reader_name.is_empty()).then_some(reader_name).as_deref())
            .await
            .map_err(to_status)?;
        Ok(Response::new(package_inventory_response(inventory)))
    }

    async fn list_applets(
        &self,
        request: Request<ListAppletsRequest>,
    ) -> Result<Response<ListAppletsResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let inventory = self
            .app
            .list_applets((!reader_name.is_empty()).then_some(reader_name).as_deref())
            .await
            .map_err(to_status)?;
        Ok(Response::new(applet_inventory_response(inventory)))
    }

    async fn transmit_apdu(
        &self,
        request: Request<CardApduRequest>,
    ) -> Result<Response<CardApduResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let command = command_apdu_from_proto(request.command)?;
        let exchange = self
            .app
            .card_command(reader_name.as_deref(), &command)
            .await
            .map_err(to_status)?;
        Ok(Response::new(CardApduResponse {
            response: Some(response_apdu_frame(&exchange.response)),
            session_state: Some(iso_session_state_info(&exchange.session_state)),
        }))
    }

    async fn transmit_raw_apdu(
        &self,
        request: Request<CardRawApduRequest>,
    ) -> Result<Response<CardRawApduResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let command = CommandApdu::parse(&request.apdu).map_err(to_status)?;
        let exchange = self
            .app
            .card_command(reader_name.as_deref(), &command)
            .await
            .map_err(to_status)?;
        Ok(Response::new(CardRawApduResponse {
            apdu: request.apdu,
            response: Some(response_apdu_frame(&exchange.response)),
            session_state: Some(iso_session_state_info(&exchange.session_state)),
        }))
    }

    async fn get_session_state(
        &self,
        request: Request<CardSelector>,
    ) -> Result<Response<GetCardSessionStateResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let session_state = self
            .app
            .card_session_state((!reader_name.is_empty()).then_some(reader_name).as_deref())
            .map_err(to_status)?;
        Ok(Response::new(GetCardSessionStateResponse {
            session_state: Some(iso_session_state_info(&session_state)),
        }))
    }

    async fn manage_channel(
        &self,
        request: Request<CardManageChannelRequest>,
    ) -> Result<Response<CardManageChannelResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let channel_number = request
            .channel_number
            .map(u8::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("channel number must fit in one byte"))?;
        let summary = self
            .app
            .manage_card_channel(reader_name.as_deref(), request.open, channel_number)
            .await
            .map_err(to_status)?;
        Ok(Response::new(CardManageChannelResponse {
            channel_number: summary.channel_number.map(u32::from),
            response: Some(response_apdu_frame(&summary.response)),
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn open_secure_messaging(
        &self,
        request: Request<CardSecureMessagingRequest>,
    ) -> Result<Response<CardSecureMessagingResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = self
            .app
            .open_card_secure_messaging(
                reader_name.as_deref(),
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
            .map_err(to_status)?;
        Ok(Response::new(CardSecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn advance_secure_messaging(
        &self,
        request: Request<CardSecureMessagingAdvanceRequest>,
    ) -> Result<Response<CardSecureMessagingResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = self
            .app
            .advance_card_secure_messaging(reader_name.as_deref(), request.increment_by)
            .map_err(to_status)?;
        Ok(Response::new(CardSecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn close_secure_messaging(
        &self,
        request: Request<CardSelector>,
    ) -> Result<Response<CardSecureMessagingResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let summary = self
            .app
            .close_card_secure_messaging(
                (!reader_name.is_empty()).then_some(reader_name).as_deref(),
            )
            .map_err(to_status)?;
        Ok(Response::new(CardSecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn open_gp_secure_channel(
        &self,
        request: Request<OpenCardGpSecureChannelRequest>,
    ) -> Result<Response<OpenCardGpSecureChannelResponse>, Status> {
        let request = request.into_inner();
        let reader_name = (!request.reader_name.is_empty()).then_some(request.reader_name);
        let summary = self
            .app
            .open_gp_secure_channel_on_card(
                reader_name.as_deref(),
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
        Ok(Response::new(OpenCardGpSecureChannelResponse {
            secure_channel: Some(gp_secure_channel_info(&summary)),
        }))
    }

    async fn close_gp_secure_channel(
        &self,
        request: Request<CardSelector>,
    ) -> Result<Response<CardSecureMessagingResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let summary = self
            .app
            .close_gp_secure_channel_on_card(
                (!reader_name.is_empty()).then_some(reader_name).as_deref(),
            )
            .map_err(to_status)?;
        Ok(Response::new(CardSecureMessagingResponse {
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }

    async fn reset_card(
        &self,
        request: Request<ResetCardRequest>,
    ) -> Result<Response<ResetCardResponse>, Status> {
        let reader_name = request.into_inner().reader_name;
        let reader_name = (!reader_name.is_empty()).then_some(reader_name);
        let summary = self
            .app
            .reset_card_summary(reader_name.as_deref())
            .await
            .map_err(to_status)?;
        Ok(Response::new(ResetCardResponse {
            atr: summary.atr.as_ref().map(atr_info),
            session_state: Some(iso_session_state_info(&summary.session_state)),
        }))
    }
}

#[cfg(test)]
mod tests {
    use tonic::Request;

    use jcim_api::v0_3::card_service_server::CardService;
    use jcim_api::v0_3::install_cap_request::Input as InstallCapInput;
    use jcim_core::globalplatform;

    use super::*;
    use crate::rpc::testsupport::{create_demo_project, load_rpc, project_selector, temp_root};

    #[tokio::test]
    async fn card_rpc_maps_reader_status_install_and_iso_happy_paths() {
        let root = temp_root("card");
        let rpc = load_rpc(&root);
        let project_root = create_demo_project(&rpc, &root, "Demo");
        let select_isd = globalplatform::select_issuer_security_domain().to_bytes();

        let readers = CardService::list_readers(&rpc, Request::new(Empty {}))
            .await
            .expect("list readers")
            .into_inner();
        let status = CardService::get_card_status(
            &rpc,
            Request::new(CardStatusRequest {
                reader_name: String::new(),
            }),
        )
        .await
        .expect("card status")
        .into_inner();
        let install = CardService::install_cap(
            &rpc,
            Request::new(InstallCapRequest {
                input: Some(InstallCapInput::Project(project_selector(&project_root))),
                reader_name: String::new(),
            }),
        )
        .await
        .expect("install cap")
        .into_inner();
        let packages = CardService::list_packages(
            &rpc,
            Request::new(ListPackagesRequest {
                reader_name: String::new(),
            }),
        )
        .await
        .expect("list packages")
        .into_inner();
        let applets = CardService::list_applets(
            &rpc,
            Request::new(ListAppletsRequest {
                reader_name: String::new(),
            }),
        )
        .await
        .expect("list applets")
        .into_inner();
        let session = CardService::get_session_state(
            &rpc,
            Request::new(CardSelector {
                reader_name: String::new(),
            }),
        )
        .await
        .expect("get session state")
        .into_inner();
        let typed = CardService::transmit_apdu(
            &rpc,
            Request::new(CardApduRequest {
                reader_name: String::new(),
                command: Some(jcim_api::v0_3::CommandApduFrame {
                    raw: select_isd.clone(),
                    ..jcim_api::v0_3::CommandApduFrame::default()
                }),
            }),
        )
        .await
        .expect("typed apdu")
        .into_inner();
        let raw = CardService::transmit_raw_apdu(
            &rpc,
            Request::new(CardRawApduRequest {
                reader_name: String::new(),
                apdu: select_isd,
            }),
        )
        .await
        .expect("raw apdu")
        .into_inner();
        let channel = CardService::manage_channel(
            &rpc,
            Request::new(CardManageChannelRequest {
                reader_name: String::new(),
                open: true,
                channel_number: None,
            }),
        )
        .await
        .expect("manage channel")
        .into_inner();
        let opened = CardService::open_secure_messaging(
            &rpc,
            Request::new(CardSecureMessagingRequest {
                reader_name: String::new(),
                protocol: jcim_api::v0_3::SecureMessagingProtocol::Scp03 as i32,
                protocol_label: String::new(),
                security_level: Some(0x03),
                session_id: "rpc-card-session".to_string(),
            }),
        )
        .await
        .expect("open secure messaging")
        .into_inner();
        let advanced = CardService::advance_secure_messaging(
            &rpc,
            Request::new(CardSecureMessagingAdvanceRequest {
                reader_name: String::new(),
                increment_by: 1,
            }),
        )
        .await
        .expect("advance secure messaging")
        .into_inner();
        let closed = CardService::close_gp_secure_channel(
            &rpc,
            Request::new(CardSelector {
                reader_name: String::new(),
            }),
        )
        .await
        .expect("close gp secure channel alias")
        .into_inner();
        let reset = CardService::reset_card(
            &rpc,
            Request::new(ResetCardRequest {
                reader_name: String::new(),
            }),
        )
        .await
        .expect("reset card")
        .into_inner();

        assert_eq!(readers.readers.len(), 1);
        assert!(status.card_present);
        assert!(!install.package_name.is_empty());
        assert!(!packages.packages.is_empty());
        assert!(!applets.applets.is_empty());
        assert!(session.session_state.is_some());
        assert_eq!(typed.response.expect("typed response").sw, 0x9000);
        assert_eq!(raw.response.expect("raw response").sw, 0x9000);
        assert_eq!(channel.channel_number, Some(1));
        assert!(
            opened
                .session_state
                .expect("opened state")
                .secure_messaging
                .is_some()
        );
        assert!(
            advanced
                .session_state
                .expect("advanced state")
                .secure_messaging
                .is_some()
        );
        assert!(closed.session_state.is_some());
        assert!(reset.atr.is_some());

        let _ = std::fs::remove_dir_all(root);
    }
}
