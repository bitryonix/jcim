use super::*;

impl JcimApp {
    /// Send one APDU to the selected simulation.
    pub async fn transmit_apdu(
        &self,
        selector: &SimulationSelectorInput,
        apdu_hex: &str,
    ) -> Result<String> {
        let command = CommandApdu::parse(&hex::decode(apdu_hex)?)?;
        let exchange = self.transmit_command(selector, &command).await?;
        Ok(hex::encode_upper(exchange.response.to_bytes()))
    }

    /// Send one typed command APDU to the selected simulation.
    pub async fn transmit_command(
        &self,
        selector: &SimulationSelectorInput,
        command: &CommandApdu,
    ) -> Result<ApduExchangeSummary> {
        let handle = self.simulation_handle(selector)?;
        let exchange = handle.transmit_typed_apdu(command.clone()).await?;
        let response = exchange.response;
        let session_state = exchange.session_state;
        let _ = self.state.update_simulation_session(
            &selector.simulation_id,
            &session_state,
            "info",
            format!(
                "apdu exchange {}",
                truncate_hex(&hex::encode_upper(command.to_bytes()))
            ),
        );
        Ok(ApduExchangeSummary {
            command: command.clone(),
            response,
            session_state,
        })
    }

    /// Open or close one logical channel on a running simulation.
    pub async fn manage_simulation_channel(
        &self,
        selector: &SimulationSelectorInput,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<ManageChannelSummary> {
        let handle = self.simulation_handle(selector)?;
        let exchange = handle.manage_channel(open, channel_number).await?;
        let channel_number = if open {
            exchange.response.data.first().copied().or(channel_number)
        } else {
            channel_number
        };
        let _ = self.state.update_simulation_session(
            &selector.simulation_id,
            &exchange.session_state,
            "info",
            if open {
                format!(
                    "opened logical channel {}",
                    channel_number.map_or_else(|| "?".to_string(), |value| value.to_string())
                )
            } else {
                format!(
                    "closed logical channel {}",
                    channel_number.map_or_else(|| "?".to_string(), |value| value.to_string())
                )
            },
        );
        Ok(ManageChannelSummary {
            channel_number,
            response: exchange.response,
            session_state: exchange.session_state,
        })
    }

    /// Mark one secure-messaging session as open for one running simulation.
    pub async fn open_simulation_secure_messaging(
        &self,
        selector: &SimulationSelectorInput,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<SecureMessagingSummary> {
        let handle = self.simulation_handle(selector)?;
        let summary = handle
            .open_secure_messaging(protocol, security_level, session_id)
            .await?;
        self.state.update_simulation_session(
            &selector.simulation_id,
            &summary.session_state,
            "info",
            "simulation secure messaging opened",
        )?;
        Ok(SecureMessagingSummary {
            session_state: summary.session_state,
        })
    }

    /// Advance the tracked secure-messaging command counter for one simulation.
    pub async fn advance_simulation_secure_messaging(
        &self,
        selector: &SimulationSelectorInput,
        increment_by: u32,
    ) -> Result<SecureMessagingSummary> {
        let handle = self.simulation_handle(selector)?;
        let summary = handle.advance_secure_messaging(increment_by).await?;
        self.state.update_simulation_session(
            &selector.simulation_id,
            &summary.session_state,
            "info",
            "simulation secure messaging advanced",
        )?;
        Ok(SecureMessagingSummary {
            session_state: summary.session_state,
        })
    }

    /// Close the tracked secure-messaging session for one simulation.
    pub async fn close_simulation_secure_messaging(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<SecureMessagingSummary> {
        let handle = self.simulation_handle(selector)?;
        let summary = handle.close_secure_messaging().await?;
        self.state.update_simulation_session(
            &selector.simulation_id,
            &summary.session_state,
            "info",
            "simulation secure messaging closed",
        )?;
        Ok(SecureMessagingSummary {
            session_state: summary.session_state,
        })
    }

    /// Open one typed GP secure channel on a running simulation.
    pub async fn open_gp_secure_channel_on_simulation(
        &self,
        selector: &SimulationSelectorInput,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
    ) -> Result<GpSecureChannelSummary> {
        let keyset = ResolvedGpKeyset::resolve(keyset_name)?;
        let security_level = gp_security_level(security_level.unwrap_or(0x01));
        let selected_aid = Aid::from_slice(&globalplatform::ISSUER_SECURITY_DOMAIN_AID)?;
        self.transmit_command(selector, &globalplatform::select_issuer_security_domain())
            .await?;
        let host_challenge = gp_host_challenge();
        let initialize_update = self
            .transmit_command(selector, &globalplatform::initialize_update(host_challenge))
            .await?;
        let initialize_update =
            globalplatform::parse_initialize_update(keyset.mode, &initialize_update.response)?;
        let derived = globalplatform::derive_session_context(
            keyset.metadata(),
            security_level,
            host_challenge,
            initialize_update,
        );
        let secure_channel = globalplatform::establish_secure_channel(
            &derived,
            format!("sim-gp-{}", selector.simulation_id),
        );
        self.transmit_command(
            selector,
            &globalplatform::external_authenticate(security_level, [0x00; 8]),
        )
        .await?;
        let summary = self
            .open_simulation_secure_messaging(
                selector,
                Some(keyset.protocol()),
                Some(security_level.as_byte()),
                Some(secure_channel.session_id.clone()),
            )
            .await?;
        Ok(GpSecureChannelSummary {
            secure_channel,
            selected_aid,
            session_state: summary.session_state,
        })
    }

    /// Close one typed GP secure channel on a running simulation.
    pub async fn close_gp_secure_channel_on_simulation(
        &self,
        selector: &SimulationSelectorInput,
    ) -> Result<SecureMessagingSummary> {
        self.close_simulation_secure_messaging(selector).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::testsupport::{
        acquire_local_service_lock, load_test_app, project_selector, simulation_selector, temp_root,
    };

    #[tokio::test]
    async fn apdu_exchange_and_channel_management_record_simulation_events() {
        let _service_lock = acquire_local_service_lock();
        let root = temp_root("sim-session-events");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");

        let simulation = app
            .start_project_simulation(&project_selector(&project_root))
            .await
            .expect("start simulation");
        let selector = simulation_selector(simulation.simulation_id.clone());
        let applet_aid = Aid::from_hex("F00000000101").expect("applet aid");

        let exchange = app
            .transmit_command(&selector, &iso7816::select_by_name(&applet_aid))
            .await
            .expect("transmit command");
        let channel = app
            .manage_simulation_channel(&selector, true, None)
            .await
            .expect("open channel");
        let events = app.simulation_events(&selector).expect("simulation events");

        assert_eq!(exchange.response.sw, 0x9000);
        assert_eq!(channel.channel_number, Some(1));
        assert!(
            events
                .iter()
                .any(|event| event.message.contains("apdu exchange"))
        );
        assert!(
            events
                .iter()
                .any(|event| event.message.contains("opened logical channel 1"))
        );

        let _ = app.stop_simulation(&selector).await;
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn secure_messaging_lifecycle_on_simulation_updates_tracked_session_state() {
        let _service_lock = acquire_local_service_lock();
        let root = temp_root("sim-gp");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");

        let simulation = app
            .start_project_simulation(&project_selector(&project_root))
            .await
            .expect("start simulation");
        let selector = simulation_selector(simulation.simulation_id.clone());

        let summary = app
            .open_simulation_secure_messaging(
                &selector,
                Some(SecureMessagingProtocol::Scp03),
                Some(0x03),
                Some("sim-session".to_string()),
            )
            .await
            .expect("open secure messaging");
        let session_state = app
            .simulation_session_state(&selector)
            .expect("session state");

        assert!(summary.session_state.secure_messaging.active);
        assert_eq!(session_state, summary.session_state);
        assert_eq!(
            session_state.secure_messaging.session_id,
            Some("sim-session".to_string())
        );

        let closed = app
            .close_gp_secure_channel_on_simulation(&selector)
            .await
            .expect("close secure messaging through gp alias");
        assert!(!closed.session_state.secure_messaging.active);

        let _ = app.stop_simulation(&selector).await;
        let _ = std::fs::remove_dir_all(root);
    }
}
