use super::*;

impl JcimApp {
    /// Open one typed GP secure channel on a physical card.
    pub async fn open_gp_secure_channel_on_card(
        &self,
        reader_name: Option<&str>,
        keyset_name: Option<&str>,
        security_level: Option<u8>,
    ) -> Result<GpSecureChannelSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let keyset = ResolvedGpKeyset::resolve(keyset_name)?;
        let security_level_byte = security_level.unwrap_or(0x01);
        let security_level = gp_security_level(security_level_byte);
        let selected_aid = Aid::from_slice(&globalplatform::ISSUER_SECURITY_DOMAIN_AID)?;
        let secure_channel = globalplatform::EstablishedSecureChannel {
            keyset: keyset.metadata(),
            security_level,
            session_id: format!(
                "card-gp-{}",
                effective_reader
                    .clone()
                    .unwrap_or_else(|| "default".to_string())
            ),
        };

        self.state
            .card_adapter
            .open_gp_secure_channel(
                &user_config,
                effective_reader.as_deref(),
                &keyset,
                security_level_byte,
            )
            .await?;
        self.card_status(effective_reader.as_deref()).await?;

        let reader_key = effective_reader.clone().unwrap_or_default();
        let session_state = self.state.open_card_gp_secure_channel(
            &reader_key,
            secure_channel.clone(),
            selected_aid.clone(),
            keyset.protocol(),
            security_level.as_byte(),
        )?;
        Ok(GpSecureChannelSummary {
            secure_channel,
            selected_aid,
            session_state,
        })
    }

    /// Close one typed GP secure channel on a physical card.
    pub fn close_gp_secure_channel_on_card(
        &self,
        reader_name: Option<&str>,
    ) -> Result<SecureMessagingSummary> {
        self.close_card_secure_messaging(reader_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::testsupport::{load_test_app, temp_root};

    #[tokio::test]
    async fn opening_gp_secure_channel_updates_tracked_card_session_state() {
        let root = temp_root("card-gp");
        let app = load_test_app(&root);

        let summary = app
            .open_gp_secure_channel_on_card(None, Some("__test__"), Some(0x03))
            .await
            .expect("open gp secure channel");
        let session_state = app.card_session_state(None).expect("card session state");
        let selected_aid = summary.selected_aid.clone();

        assert!(summary.session_state.secure_messaging.active);
        assert_eq!(
            selected_aid,
            Aid::from_slice(&globalplatform::ISSUER_SECURITY_DOMAIN_AID).expect("isd aid")
        );
        assert_eq!(session_state, summary.session_state);
        assert_eq!(session_state.selected_aid, Some(selected_aid.clone()));

        let closed = app
            .close_gp_secure_channel_on_card(None)
            .expect("close gp secure channel");
        assert!(!closed.session_state.secure_messaging.active);

        let _ = std::fs::remove_dir_all(root);
    }
}
