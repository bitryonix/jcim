use super::*;

impl AppState {
    /// Return the tracked card session state for one reader or a blank default session.
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

    /// Return the tracked GP secure-channel summary for one reader when one is active.
    pub(crate) fn card_gp_secure_channel(
        &self,
        reader_key: &str,
    ) -> Result<Option<globalplatform::EstablishedSecureChannel>> {
        let sessions = self.card_sessions.lock().map_err(lock_poisoned)?;
        Ok(sessions
            .get(reader_key)
            .and_then(|record| record.gp_secure_channel.clone()))
    }

    /// Replace the authoritative tracked session state after a fresh card-status snapshot.
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

    /// Apply one APDU exchange to the tracked card session and return the updated state.
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

    /// Mark secure messaging as open for one tracked card session.
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

    /// Advance the tracked secure-messaging counter for one card session.
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

    /// Clear the tracked secure-messaging and GP secure-channel state for one reader.
    pub(crate) fn close_card_secure_messaging(&self, reader_key: &str) -> Result<IsoSessionState> {
        self.with_card_session_mut(reader_key, |entry| {
            entry.session_state.secure_messaging = SecureMessagingState::default();
            entry.gp_secure_channel = None;
            entry.session_state.clone()
        })
    }

    /// Record a newly established GP secure channel on the basic card channel.
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

    /// Replace the entire tracked card session after a reset or authoritative card snapshot.
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

    /// Run one mutation against the tracked card session for a reader, creating defaults first.
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

/// Tracked physical-card session state plus any active GP secure-channel metadata.
#[derive(Clone, Debug, Default)]
pub(crate) struct CardSessionRecord {
    /// Current ISO/IEC 7816 session state mirrored from the reader.
    pub(crate) session_state: IsoSessionState,
    /// Active GP secure-channel metadata, when JCIM has opened one for this reader.
    pub(crate) gp_secure_channel: Option<globalplatform::EstablishedSecureChannel>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use jcim_config::project::{ManagedPaths, UserConfig};
    use jcim_core::aid::Aid;
    use jcim_core::apdu::ResponseApdu;
    use jcim_core::globalplatform::{
        EstablishedSecureChannel, GpKeysetMetadata, ScpMode, SecurityLevel,
    };
    use jcim_core::iso7816::{self, IsoSessionState, SecureMessagingProtocol};

    use super::*;
    use crate::card::JavaPhysicalCardAdapter;
    use crate::registry::ProjectRegistry;

    #[test]
    fn card_sessions_bootstrap_defaults_and_open_then_close_secure_messaging() {
        let root = temp_root("secure-messaging");
        let state = test_state(&root);

        assert_eq!(
            state
                .card_session_state_or_default("Reader One")
                .expect("default session"),
            IsoSessionState::default()
        );

        let opened = state
            .open_card_secure_messaging(
                "Reader One",
                Some(SecureMessagingProtocol::Scp03),
                Some(0x13),
                Some("session-1".to_string()),
            )
            .expect("open secure messaging");
        assert!(opened.secure_messaging.active);
        assert_eq!(
            opened.secure_messaging.protocol,
            Some(SecureMessagingProtocol::Scp03)
        );
        assert_eq!(opened.secure_messaging.security_level, Some(0x13));
        assert_eq!(
            opened.secure_messaging.session_id.as_deref(),
            Some("session-1")
        );

        let closed = state
            .close_card_secure_messaging("Reader One")
            .expect("close secure messaging");
        assert_eq!(closed.secure_messaging, SecureMessagingState::default());
        assert_eq!(
            state
                .card_gp_secure_channel("Reader One")
                .expect("secure channel"),
            None
        );
    }

    #[test]
    fn secure_messaging_counters_increment_only_when_active_and_saturate() {
        let root = temp_root("counters");
        let state = test_state(&root);
        let aid = Aid::from_hex("A000000151000001").expect("aid");
        let command = iso7816::select_by_name(&aid);
        let response = ResponseApdu::status(0x9000);

        state
            .reset_card_session("Reader Two", &IsoSessionState::reset(None, None))
            .expect("reset session");
        let without_secure_messaging = state
            .record_card_command("Reader Two", &command, &response)
            .expect("record command without secure messaging");
        assert_eq!(without_secure_messaging.secure_messaging.command_counter, 0);

        let opened = state
            .open_card_secure_messaging(
                "Reader Two",
                Some(SecureMessagingProtocol::Iso7816),
                Some(0x01),
                Some("session-2".to_string()),
            )
            .expect("open secure messaging");
        assert_eq!(opened.secure_messaging.command_counter, 0);

        let incremented = state
            .record_card_command("Reader Two", &command, &response)
            .expect("record command with secure messaging");
        assert_eq!(incremented.secure_messaging.command_counter, 1);

        state
            .with_card_session_mut("Reader Two", |entry| {
                entry.session_state.secure_messaging.command_counter = u32::MAX - 1;
            })
            .expect("set high counter");
        let saturated = state
            .advance_card_secure_messaging("Reader Two", 10)
            .expect("advance secure messaging");
        assert_eq!(saturated.secure_messaging.command_counter, u32::MAX);
    }

    #[test]
    fn gp_secure_channel_open_updates_selected_aid_and_basic_channel() {
        let root = temp_root("gp-open");
        let state = test_state(&root);
        let secure_channel = EstablishedSecureChannel {
            keyset: GpKeysetMetadata {
                name: "lab".to_string(),
                mode: ScpMode::Scp03,
            },
            security_level: SecurityLevel::CommandAndResponseMacWithEncryption,
            session_id: "gp-session".to_string(),
        };
        let selected_aid = Aid::from_hex("A000000151000000").expect("aid");

        state
            .reset_card_session("Reader Three", &IsoSessionState::reset(None, None))
            .expect("reset session");
        let session = state
            .open_card_gp_secure_channel(
                "Reader Three",
                secure_channel.clone(),
                selected_aid.clone(),
                SecureMessagingProtocol::Scp03,
                0x13,
            )
            .expect("open GP secure channel");

        assert_eq!(session.selected_aid, Some(selected_aid.clone()));
        let channel_zero = session
            .open_channels
            .iter()
            .find(|channel| channel.channel_number == 0)
            .expect("basic channel");
        assert_eq!(channel_zero.selected_aid, Some(selected_aid));
        assert_eq!(
            state
                .card_gp_secure_channel("Reader Three")
                .expect("stored secure channel"),
            Some(secure_channel)
        );
    }

    fn test_state(root: &Path) -> AppState {
        AppState::new(
            ManagedPaths::for_root(root.join("managed")),
            PathBuf::from("/tmp/jcimd-test"),
            "fingerprint".to_string(),
            UserConfig::default(),
            ProjectRegistry::default(),
            Arc::new(JavaPhysicalCardAdapter),
            1,
        )
    }

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from("/tmp").join(format!("jcim-card-session-{label}-{unique:x}"))
    }
}
