use jcim_api::v0_3::card_service_client::CardServiceClient;
use jcim_api::v0_3::install_cap_request::Input as InstallCapInput;
use jcim_api::v0_3::{
    CardApduRequest, CardManageChannelRequest, CardRawApduRequest,
    CardSecureMessagingAdvanceRequest, CardSecureMessagingRequest, CardSelector, CardStatusRequest,
    Empty, InstallCapRequest, ListAppletsRequest, ListPackagesRequest,
    OpenCardGpSecureChannelRequest, ResetCardRequest,
};

use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::iso7816::{IsoSessionState, SecureMessagingProtocol};
use jcim_core::{globalplatform, iso7816};

use crate::error::Result;
use crate::types::{
    ApduExchangeSummary, AppletSummary, CardAppletInventory, CardAppletSummary, CardDeleteSummary,
    CardInstallSource, CardInstallSummary, CardPackageInventory, CardPackageSummary,
    CardReaderSummary, CardStatusSummary, ManageChannelSummary, ReaderRef, ResetSummary,
    SecureMessagingSummary,
};

use super::JcimClient;
use super::bootstrap::invalid_connection_target;
use super::proto::{
    command_apdu_frame, gp_secure_channel_from_proto, iso_session_state_from_proto,
    project_selector, reset_summary_from_card_proto, response_apdu_from_proto,
    secure_messaging_protocol_fields,
};

impl JcimClient {
    /// List visible physical readers.
    pub async fn list_readers(&self) -> Result<Vec<CardReaderSummary>> {
        let response = CardServiceClient::new(self.channel.clone())
            .list_readers(Empty {})
            .await?
            .into_inner();
        Ok(response
            .readers
            .into_iter()
            .map(|reader| CardReaderSummary {
                name: reader.name,
                card_present: reader.card_present,
            })
            .collect())
    }

    /// Fetch physical-card status using the configured default reader.
    pub async fn get_card_status(&self) -> Result<CardStatusSummary> {
        self.get_card_status_on(ReaderRef::Default).await
    }

    /// Fetch physical-card status using one explicit reader selector.
    pub async fn get_card_status_on(&self, reader: ReaderRef) -> Result<CardStatusSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .get_card_status(CardStatusRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(CardStatusSummary {
            reader_name: response.reader_name,
            card_present: response.card_present,
            atr: super::proto::atr_from_proto(response.atr)?,
            active_protocol: super::proto::protocol_parameters_from_proto(response.active_protocol),
            iso_capabilities: super::proto::iso_capabilities_from_proto(response.iso_capabilities),
            session_state: iso_session_state_from_proto(response.session_state)?,
            lines: response.lines,
        })
    }

    /// Install a CAP onto a physical card using the configured default reader.
    pub async fn install_cap(&self, source: CardInstallSource) -> Result<CardInstallSummary> {
        self.install_cap_on(source, ReaderRef::Default).await
    }

    /// Install a CAP onto a physical card using one explicit reader selector.
    pub async fn install_cap_on(
        &self,
        source: CardInstallSource,
        reader: ReaderRef,
    ) -> Result<CardInstallSummary> {
        let request = InstallCapRequest {
            input: Some(match source {
                CardInstallSource::Project(project) => {
                    InstallCapInput::Project(project_selector(&project))
                }
                CardInstallSource::Cap(cap_path) => {
                    InstallCapInput::CapPath(cap_path.display().to_string())
                }
            }),
            reader_name: reader.as_deref().unwrap_or_default().to_string(),
        };
        let response = CardServiceClient::new(self.channel.clone())
            .install_cap(request)
            .await?
            .into_inner();
        Ok(CardInstallSummary {
            reader_name: response.reader_name,
            cap_path: crate::types::owned_path(response.cap_path),
            package_name: response.package_name,
            package_aid: response.package_aid,
            applets: response
                .applets
                .into_iter()
                .map(|applet| AppletSummary {
                    class_name: applet.class_name,
                    aid: applet.aid,
                })
                .collect(),
            output_lines: response.output_lines,
        })
    }

    /// Delete one item using the configured default reader.
    pub async fn delete_item(&self, aid: &str) -> Result<CardDeleteSummary> {
        self.delete_item_on(aid, ReaderRef::Default).await
    }

    /// Delete one item using one explicit reader selector.
    pub async fn delete_item_on(&self, aid: &str, reader: ReaderRef) -> Result<CardDeleteSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .delete_item(jcim_api::v0_3::DeleteItemRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                aid: aid.to_string(),
            })
            .await?
            .into_inner();
        Ok(CardDeleteSummary {
            reader_name: response.reader_name,
            aid: response.aid,
            deleted: response.deleted,
            output_lines: response.output_lines,
        })
    }

    /// List packages using the configured default reader.
    pub async fn list_packages(&self) -> Result<CardPackageInventory> {
        self.list_packages_on(ReaderRef::Default).await
    }

    /// List packages using one explicit reader selector.
    pub async fn list_packages_on(&self, reader: ReaderRef) -> Result<CardPackageInventory> {
        let response = CardServiceClient::new(self.channel.clone())
            .list_packages(ListPackagesRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(CardPackageInventory {
            reader_name: response.reader_name,
            packages: response
                .packages
                .into_iter()
                .map(|package| CardPackageSummary {
                    aid: package.aid,
                    description: package.description,
                })
                .collect(),
            output_lines: response.output_lines,
        })
    }

    /// List applets using the configured default reader.
    pub async fn list_applets(&self) -> Result<CardAppletInventory> {
        self.list_applets_on(ReaderRef::Default).await
    }

    /// List applets using one explicit reader selector.
    pub async fn list_applets_on(&self, reader: ReaderRef) -> Result<CardAppletInventory> {
        let response = CardServiceClient::new(self.channel.clone())
            .list_applets(ListAppletsRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(CardAppletInventory {
            reader_name: response.reader_name,
            applets: response
                .applets
                .into_iter()
                .map(|applet| CardAppletSummary {
                    aid: applet.aid,
                    description: applet.description,
                })
                .collect(),
            output_lines: response.output_lines,
        })
    }

    /// Send one APDU using the configured default reader.
    pub async fn transmit_card_apdu(&self, apdu: &CommandApdu) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(apdu, ReaderRef::Default).await
    }

    /// Send one APDU using one explicit reader selector.
    pub async fn transmit_card_apdu_on(
        &self,
        apdu: &CommandApdu,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        let response = CardServiceClient::new(self.channel.clone())
            .transmit_apdu(CardApduRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                command: Some(command_apdu_frame(apdu)),
            })
            .await?
            .into_inner()
            .response;
        response_apdu_from_proto(response)
    }

    /// Fetch the current tracked ISO/IEC 7816 session state using the configured default reader.
    pub async fn get_card_session_state(&self) -> Result<IsoSessionState> {
        self.get_card_session_state_on(ReaderRef::Default).await
    }

    /// Fetch the current tracked ISO/IEC 7816 session state using one explicit reader.
    pub async fn get_card_session_state_on(&self, reader: ReaderRef) -> Result<IsoSessionState> {
        let response = CardServiceClient::new(self.channel.clone())
            .get_session_state(CardSelector {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        iso_session_state_from_proto(response.session_state)
    }

    /// Send one raw APDU byte sequence using the configured default reader.
    pub async fn transmit_raw_card_apdu(&self, apdu: &[u8]) -> Result<ApduExchangeSummary> {
        self.transmit_raw_card_apdu_on(apdu, ReaderRef::Default)
            .await
    }

    /// Send one raw APDU byte sequence using one explicit reader.
    pub async fn transmit_raw_card_apdu_on(
        &self,
        apdu: &[u8],
        reader: ReaderRef,
    ) -> Result<ApduExchangeSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .transmit_raw_apdu(CardRawApduRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                apdu: apdu.to_vec(),
            })
            .await?
            .into_inner();
        Ok(ApduExchangeSummary {
            command: CommandApdu::parse(&response.apdu)?,
            response: response_apdu_from_proto(response.response)?,
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Open or close one logical channel using the configured default reader.
    pub async fn manage_card_channel(
        &self,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<ManageChannelSummary> {
        self.manage_card_channel_on(open, channel_number, ReaderRef::Default)
            .await
    }

    /// Open or close one logical channel using one explicit reader.
    pub async fn manage_card_channel_on(
        &self,
        open: bool,
        channel_number: Option<u8>,
        reader: ReaderRef,
    ) -> Result<ManageChannelSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .manage_channel(CardManageChannelRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                open,
                channel_number: channel_number.map(u32::from),
            })
            .await?
            .into_inner();
        Ok(ManageChannelSummary {
            channel_number: response.channel_number.map(|value| value as u8),
            response: response_apdu_from_proto(response.response)?,
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Mark secure messaging as active using the configured default reader.
    pub async fn open_card_secure_messaging(
        &self,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<SecureMessagingSummary> {
        self.open_card_secure_messaging_on(protocol, security_level, session_id, ReaderRef::Default)
            .await
    }

    /// Mark secure messaging as active using one explicit reader.
    pub async fn open_card_secure_messaging_on(
        &self,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
        reader: ReaderRef,
    ) -> Result<SecureMessagingSummary> {
        let (protocol, protocol_label) = secure_messaging_protocol_fields(protocol.as_ref());
        let response = CardServiceClient::new(self.channel.clone())
            .open_secure_messaging(CardSecureMessagingRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                protocol,
                security_level: security_level.map(u32::from),
                session_id: session_id.unwrap_or_default(),
                protocol_label,
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Advance the secure-messaging command counter using the configured default reader.
    pub async fn advance_card_secure_messaging(
        &self,
        increment_by: u32,
    ) -> Result<SecureMessagingSummary> {
        self.advance_card_secure_messaging_on(increment_by, ReaderRef::Default)
            .await
    }

    /// Advance the secure-messaging command counter using one explicit reader.
    pub async fn advance_card_secure_messaging_on(
        &self,
        increment_by: u32,
        reader: ReaderRef,
    ) -> Result<SecureMessagingSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .advance_secure_messaging(CardSecureMessagingAdvanceRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                increment_by,
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Clear the tracked secure-messaging state using the configured default reader.
    pub async fn close_card_secure_messaging(&self) -> Result<SecureMessagingSummary> {
        self.close_card_secure_messaging_on(ReaderRef::Default)
            .await
    }

    /// Clear the tracked secure-messaging state using one explicit reader.
    pub async fn close_card_secure_messaging_on(
        &self,
        reader: ReaderRef,
    ) -> Result<SecureMessagingSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .close_secure_messaging(CardSelector {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Open one typed GP secure channel using the configured default reader.
    pub async fn open_gp_secure_channel_on_card(
        &self,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
    ) -> Result<crate::types::GpSecureChannelSummary> {
        self.open_gp_secure_channel_on_card_with_reader(
            keyset_name,
            security_level,
            ReaderRef::Default,
        )
        .await
    }

    /// Open one typed GP secure channel using one explicit reader.
    pub async fn open_gp_secure_channel_on_card_with_reader(
        &self,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
        reader: ReaderRef,
    ) -> Result<crate::types::GpSecureChannelSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .open_gp_secure_channel(OpenCardGpSecureChannelRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
                keyset_name: keyset_name.unwrap_or_default().to_string(),
                security_level: security_level.map(u32::from),
            })
            .await?
            .into_inner();
        gp_secure_channel_from_proto(response.secure_channel)
    }

    /// Close one typed GP secure channel using the configured default reader.
    pub async fn close_gp_secure_channel_on_card(&self) -> Result<SecureMessagingSummary> {
        self.close_gp_secure_channel_on_card_with_reader(ReaderRef::Default)
            .await
    }

    /// Close one typed GP secure channel using one explicit reader.
    pub async fn close_gp_secure_channel_on_card_with_reader(
        &self,
        reader: ReaderRef,
    ) -> Result<SecureMessagingSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .close_gp_secure_channel(CardSelector {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        Ok(SecureMessagingSummary {
            session_state: iso_session_state_from_proto(response.session_state)?,
        })
    }

    /// Send one ISO/IEC 7816 `SELECT` by application identifier using the configured default reader.
    pub async fn iso_select_application_on_card(&self, aid: &Aid) -> Result<ResponseApdu> {
        self.iso_select_application_on_card_with_reader(aid, ReaderRef::Default)
            .await
    }

    /// Send one ISO/IEC 7816 `SELECT` by application identifier using one explicit reader.
    pub async fn iso_select_application_on_card_with_reader(
        &self,
        aid: &Aid,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(&iso7816::select_by_name(aid), reader)
            .await
    }

    /// Send one GlobalPlatform `SELECT` for the issuer security domain using the configured default reader.
    pub async fn gp_select_issuer_security_domain_on_card(&self) -> Result<ResponseApdu> {
        self.gp_select_issuer_security_domain_on_card_with_reader(ReaderRef::Default)
            .await
    }

    /// Send one GlobalPlatform `SELECT` for the issuer security domain using one explicit reader.
    pub async fn gp_select_issuer_security_domain_on_card_with_reader(
        &self,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(&globalplatform::select_issuer_security_domain(), reader)
            .await
    }

    /// Run one typed GlobalPlatform `GET STATUS` request using the configured default reader.
    pub async fn gp_get_status_on_card(
        &self,
        kind: globalplatform::RegistryKind,
        occurrence: globalplatform::GetStatusOccurrence,
    ) -> Result<globalplatform::GetStatusResponse> {
        self.gp_get_status_on_card_with_reader(kind, occurrence, ReaderRef::Default)
            .await
    }

    /// Run one typed GlobalPlatform `GET STATUS` request using one explicit reader.
    pub async fn gp_get_status_on_card_with_reader(
        &self,
        kind: globalplatform::RegistryKind,
        occurrence: globalplatform::GetStatusOccurrence,
        reader: ReaderRef,
    ) -> Result<globalplatform::GetStatusResponse> {
        let response = self
            .transmit_card_apdu_on(&globalplatform::get_status(kind, occurrence), reader)
            .await?;
        Ok(globalplatform::parse_get_status(kind, &response)?)
    }

    /// Set one GlobalPlatform card life cycle state using the configured default reader.
    pub async fn gp_set_card_status_on_card(
        &self,
        state: globalplatform::CardLifeCycle,
    ) -> Result<ResponseApdu> {
        self.gp_set_card_status_on_card_with_reader(state, ReaderRef::Default)
            .await
    }

    /// Set one GlobalPlatform card life cycle state using one explicit reader.
    pub async fn gp_set_card_status_on_card_with_reader(
        &self,
        state: globalplatform::CardLifeCycle,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(&globalplatform::set_card_status(state), reader)
            .await
    }

    /// Lock or unlock one application using the configured default reader.
    pub async fn gp_set_application_status_on_card(
        &self,
        aid: &Aid,
        transition: globalplatform::LockTransition,
    ) -> Result<ResponseApdu> {
        self.gp_set_application_status_on_card_with_reader(aid, transition, ReaderRef::Default)
            .await
    }

    /// Lock or unlock one application using one explicit reader.
    pub async fn gp_set_application_status_on_card_with_reader(
        &self,
        aid: &Aid,
        transition: globalplatform::LockTransition,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(
            &globalplatform::set_application_status(aid, transition),
            reader,
        )
        .await
    }

    /// Lock or unlock one security domain and its applications using the configured default reader.
    pub async fn gp_set_security_domain_status_on_card(
        &self,
        aid: &Aid,
        transition: globalplatform::LockTransition,
    ) -> Result<ResponseApdu> {
        self.gp_set_security_domain_status_on_card_with_reader(aid, transition, ReaderRef::Default)
            .await
    }

    /// Lock or unlock one security domain and its applications using one explicit reader.
    pub async fn gp_set_security_domain_status_on_card_with_reader(
        &self,
        aid: &Aid,
        transition: globalplatform::LockTransition,
        reader: ReaderRef,
    ) -> Result<ResponseApdu> {
        self.transmit_card_apdu_on(
            &globalplatform::set_security_domain_status(aid, transition),
            reader,
        )
        .await
    }

    /// Reset the configured default reader and return the typed reset summary.
    pub async fn reset_card_summary(&self) -> Result<ResetSummary> {
        self.reset_card_summary_on(ReaderRef::Default).await
    }

    /// Reset one explicit reader and return the typed reset summary.
    pub async fn reset_card_summary_on(&self, reader: ReaderRef) -> Result<ResetSummary> {
        let response = CardServiceClient::new(self.channel.clone())
            .reset_card(ResetCardRequest {
                reader_name: reader.as_deref().unwrap_or_default().to_string(),
            })
            .await?
            .into_inner();
        reset_summary_from_card_proto(response)
    }

    pub(super) async fn validated_card_status_for_connection(
        &self,
        reader: ReaderRef,
    ) -> Result<CardStatusSummary> {
        if let ReaderRef::Named(reader_name) = &reader
            && reader_name.trim().is_empty()
        {
            return Err(invalid_connection_target(
                "reader connection requires a non-empty reader name".to_string(),
            ));
        }
        let status = self.get_card_status_on(reader).await?;
        if !status.card_present {
            let reader_name = status.reader_name.trim();
            let message = if reader_name.is_empty() {
                "reader connection requires a present card".to_string()
            } else {
                format!("reader `{reader_name}` has no present card")
            };
            return Err(invalid_connection_target(message));
        }
        Ok(status)
    }
}
