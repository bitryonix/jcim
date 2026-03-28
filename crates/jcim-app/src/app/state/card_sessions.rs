use super::*;

impl AppState {
    pub(crate) fn card_session_state_or_default(
        &self,
        reader_key: &str,
    ) -> Result<IsoSessionState> {
        let sessions = self.card_sessions.lock().map_err(lock_poisoned)?;
        Ok(sessions
            .get(reader_key)
            .map(|record| record.session_state.clone())
            .unwrap_or_default())
    }

    pub(crate) fn card_gp_secure_channel(
        &self,
        reader_key: &str,
    ) -> Result<Option<globalplatform::EstablishedSecureChannel>> {
        let sessions = self.card_sessions.lock().map_err(lock_poisoned)?;
        Ok(sessions
            .get(reader_key)
            .and_then(|record| record.gp_secure_channel.clone()))
    }

    pub(crate) fn sync_card_status(
        &self,
        reader_key: &str,
        session_state: &IsoSessionState,
    ) -> Result<()> {
        self.with_card_session_mut(reader_key, |entry| {
            let gp_secure_channel = entry.gp_secure_channel.clone();
            *entry = CardSessionRecord {
                session_state: session_state.clone(),
                gp_secure_channel,
            };
        })
    }

    pub(crate) fn record_card_command(
        &self,
        reader_key: &str,
        command: &CommandApdu,
        response: &ResponseApdu,
    ) -> Result<IsoSessionState> {
        self.with_card_session_mut(reader_key, |entry| {
            let _ = apply_response_to_session(&mut entry.session_state, command, response);
            if entry.session_state.secure_messaging.active {
                entry.session_state.secure_messaging.command_counter = entry
                    .session_state
                    .secure_messaging
                    .command_counter
                    .saturating_add(1);
            }
            entry.session_state.clone()
        })
    }

    pub(crate) fn open_card_secure_messaging(
        &self,
        reader_key: &str,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<IsoSessionState> {
        self.with_card_session_mut(reader_key, |entry| {
            entry.session_state.secure_messaging = SecureMessagingState {
                active: true,
                protocol,
                security_level,
                session_id,
                command_counter: 0,
            };
            entry.gp_secure_channel = None;
            entry.session_state.clone()
        })
    }

    pub(crate) fn advance_card_secure_messaging(
        &self,
        reader_key: &str,
        increment_by: u32,
    ) -> Result<IsoSessionState> {
        self.with_card_session_mut(reader_key, |entry| {
            entry.session_state.secure_messaging.command_counter = entry
                .session_state
                .secure_messaging
                .command_counter
                .saturating_add(increment_by.max(1));
            entry.session_state.clone()
        })
    }

    pub(crate) fn close_card_secure_messaging(&self, reader_key: &str) -> Result<IsoSessionState> {
        self.with_card_session_mut(reader_key, |entry| {
            entry.session_state.secure_messaging = SecureMessagingState::default();
            entry.gp_secure_channel = None;
            entry.session_state.clone()
        })
    }

    pub(crate) fn open_card_gp_secure_channel(
        &self,
        reader_key: &str,
        secure_channel: globalplatform::EstablishedSecureChannel,
        selected_aid: Aid,
        protocol: SecureMessagingProtocol,
        security_level: u8,
    ) -> Result<IsoSessionState> {
        self.with_card_session_mut(reader_key, |entry| {
            entry.session_state.selected_aid = Some(selected_aid.clone());
            entry.session_state.current_file = None;
            if let Some(channel) = entry
                .session_state
                .open_channels
                .iter_mut()
                .find(|channel| channel.channel_number == 0)
            {
                channel.selected_aid = Some(selected_aid.clone());
                channel.current_file = None;
            }
            entry.session_state.secure_messaging = SecureMessagingState {
                active: true,
                protocol: Some(protocol),
                security_level: Some(security_level),
                session_id: Some(secure_channel.session_id.clone()),
                command_counter: 0,
            };
            entry.gp_secure_channel = Some(secure_channel);
            entry.session_state.clone()
        })
    }

    pub(crate) fn reset_card_session(
        &self,
        reader_key: &str,
        session_state: &IsoSessionState,
    ) -> Result<()> {
        self.card_sessions.lock().map_err(lock_poisoned)?.insert(
            reader_key.to_string(),
            CardSessionRecord {
                session_state: session_state.clone(),
                gp_secure_channel: None,
            },
        );
        Ok(())
    }

    fn with_card_session_mut<T>(
        &self,
        reader_key: &str,
        op: impl FnOnce(&mut CardSessionRecord) -> T,
    ) -> Result<T> {
        let mut sessions = self.card_sessions.lock().map_err(lock_poisoned)?;
        let entry = sessions
            .entry(reader_key.to_string())
            .or_insert_with(CardSessionRecord::default);
        Ok(op(entry))
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CardSessionRecord {
    pub(crate) session_state: IsoSessionState,
    pub(crate) gp_secure_channel: Option<globalplatform::EstablishedSecureChannel>,
}
