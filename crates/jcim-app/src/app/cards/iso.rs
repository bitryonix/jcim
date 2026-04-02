use super::*;

impl JcimApp {
    /// Return the current tracked ISO/IEC 7816 session state for one physical card reader.
    pub fn card_session_state(&self, reader_name: Option<&str>) -> Result<IsoSessionState> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let key = effective_reader.unwrap_or_default();
        self.state.card_session_state_or_default(&key)
    }

    /// Open or close one logical channel on a physical card.
    pub async fn manage_card_channel(
        &self,
        reader_name: Option<&str>,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<ManageChannelSummary> {
        let command = if open {
            iso7816::manage_channel_open()
        } else {
            iso7816::manage_channel_close(channel_number.unwrap_or_default())
        };
        let exchange = self.card_command(reader_name, &command).await?;
        let channel_number = if open {
            exchange.response.data.first().copied().or(channel_number)
        } else {
            channel_number
        };
        Ok(ManageChannelSummary {
            channel_number,
            response: exchange.response,
            session_state: exchange.session_state,
        })
    }

    /// Mark one secure-messaging session as open for one physical card reader.
    pub fn open_card_secure_messaging(
        &self,
        reader_name: Option<&str>,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<SecureMessagingSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let key = effective_reader.unwrap_or_default();
        let session_state =
            self.state
                .open_card_secure_messaging(&key, protocol, security_level, session_id)?;
        Ok(SecureMessagingSummary { session_state })
    }

    /// Advance the tracked secure-messaging command counter for one physical card reader.
    pub fn advance_card_secure_messaging(
        &self,
        reader_name: Option<&str>,
        increment_by: u32,
    ) -> Result<SecureMessagingSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let key = effective_reader.unwrap_or_default();
        let session_state = self
            .state
            .advance_card_secure_messaging(&key, increment_by)?;
        Ok(SecureMessagingSummary { session_state })
    }

    /// Close the tracked secure-messaging session for one physical card reader.
    pub fn close_card_secure_messaging(
        &self,
        reader_name: Option<&str>,
    ) -> Result<SecureMessagingSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let key = effective_reader.unwrap_or_default();
        let session_state = self.state.close_card_secure_messaging(&key)?;
        Ok(SecureMessagingSummary { session_state })
    }
}

#[cfg(test)]
mod tests {
    use jcim_core::iso7816::SecureMessagingProtocol;

    use crate::app::testsupport::{load_test_app, temp_root};

    #[tokio::test]
    async fn card_iso_helpers_use_default_reader_and_track_channel_and_secure_messaging() {
        let root = temp_root("card-iso");
        let app = load_test_app(&root);
        app.state
            .persist_user_config(|config| {
                config.default_reader = Some("Configured Reader".to_string());
            })
            .expect("persist user config");

        let opened = app
            .manage_card_channel(None, true, None)
            .await
            .expect("open logical channel");
        assert_eq!(opened.channel_number, Some(1));
        assert!(opened.response.is_success());
        assert!(
            opened
                .session_state
                .open_channels
                .iter()
                .any(|channel| channel.channel_number == 1)
        );

        let via_default = app
            .card_session_state(None)
            .expect("session state by default");
        let via_explicit = app
            .card_session_state(Some("Configured Reader"))
            .expect("session state by explicit reader");
        assert_eq!(via_default, via_explicit);

        let opened_sm = app
            .open_card_secure_messaging(
                None,
                Some(SecureMessagingProtocol::Iso7816),
                Some(0x01),
                Some("session-1".to_string()),
            )
            .expect("open secure messaging");
        assert!(opened_sm.session_state.secure_messaging.active);
        assert_eq!(
            opened_sm.session_state.secure_messaging.protocol,
            Some(SecureMessagingProtocol::Iso7816)
        );
        assert_eq!(
            opened_sm
                .session_state
                .secure_messaging
                .session_id
                .as_deref(),
            Some("session-1")
        );

        let advanced = app
            .advance_card_secure_messaging(None, 2)
            .expect("advance secure messaging");
        assert_eq!(advanced.session_state.secure_messaging.command_counter, 2);

        let closed_sm = app
            .close_card_secure_messaging(None)
            .expect("close secure messaging");
        assert!(!closed_sm.session_state.secure_messaging.active);
        assert_eq!(
            app.card_session_state(None)
                .expect("session state after secure messaging close")
                .secure_messaging,
            closed_sm.session_state.secure_messaging
        );

        let closed_channel = app
            .manage_card_channel(None, false, Some(1))
            .await
            .expect("close logical channel");
        assert_eq!(closed_channel.channel_number, Some(1));
        assert!(closed_channel.response.is_success());
        assert!(
            closed_channel
                .session_state
                .open_channels
                .iter()
                .all(|channel| channel.channel_number != 1)
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
