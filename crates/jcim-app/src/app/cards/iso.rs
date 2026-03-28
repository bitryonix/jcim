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
