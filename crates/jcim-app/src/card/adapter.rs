use super::*;

/// Adapter contract for real-card operations.
#[async_trait]
pub trait PhysicalCardAdapter: Send + Sync {
    /// List visible PC/SC readers.
    async fn list_readers(&self, user_config: &UserConfig) -> Result<Vec<CardReaderSummary>>;

    /// Query status for one reader.
    async fn card_status(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardStatusSummary>;

    /// Install one CAP onto a physical card.
    async fn install_cap(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        cap_path: &Path,
    ) -> Result<Vec<String>>;

    /// Delete one item from a physical card.
    async fn delete_item(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        aid: &str,
    ) -> Result<Vec<String>>;

    /// List packages visible on a physical card.
    async fn list_packages(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardPackageInventory>;

    /// List applets visible on a physical card.
    async fn list_applets(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardAppletInventory>;

    /// Send one APDU to a physical card.
    async fn transmit_apdu(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        apdu_hex: &str,
    ) -> Result<String>;

    /// Send one typed APDU to a physical card.
    async fn transmit_command(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        command: &CommandApdu,
    ) -> Result<ResponseApdu> {
        let response_hex = self
            .transmit_apdu(
                user_config,
                reader_name,
                &hex::encode_upper(command.to_bytes()),
            )
            .await?;
        ResponseApdu::parse(&hex::decode(&response_hex)?)
    }

    /// Reset a physical card and return the ATR.
    async fn reset_card(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<String>;

    /// Reset a physical card and return the typed reset summary.
    async fn reset_card_summary(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<ResetSummary> {
        let atr_hex = self.reset_card(user_config, reader_name).await?;
        let atr = (!atr_hex.trim().is_empty())
            .then(|| Atr::parse(&hex::decode(&atr_hex)?))
            .transpose()?;
        let active_protocol = atr.as_ref().map(ProtocolParameters::from_atr);
        Ok(ResetSummary {
            atr: atr.clone(),
            session_state: if atr.is_some() {
                IsoSessionState::reset(atr, active_protocol)
            } else {
                IsoSessionState::default()
            },
        })
    }

    /// Open one authenticated GP secure channel on a real card when the adapter supports it.
    async fn open_gp_secure_channel(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
        _keyset: &ResolvedGpKeyset,
        _security_level: u8,
    ) -> Result<()> {
        Err(JcimError::Unsupported(
            "physical-card GP secure-channel automation is unavailable for this adapter"
                .to_string(),
        ))
    }

    /// Send one authenticated GP APDU when the adapter supports real secure-channel execution.
    async fn transmit_gp_secure_command(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
        _keyset: &ResolvedGpKeyset,
        _security_level: u8,
        _command: &CommandApdu,
    ) -> Result<ResponseApdu> {
        Err(JcimError::Unsupported(
            "authenticated GP APDU transport is unavailable for this adapter".to_string(),
        ))
    }
}
