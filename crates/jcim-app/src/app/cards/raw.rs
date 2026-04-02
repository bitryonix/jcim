use super::*;

impl JcimApp {
    /// Send one APDU to a physical card.
    pub async fn card_apdu(&self, reader_name: Option<&str>, apdu_hex: &str) -> Result<String> {
        let command = CommandApdu::parse(&hex::decode(apdu_hex)?)?;
        let exchange = self.card_command(reader_name, &command).await?;
        Ok(hex::encode_upper(exchange.response.to_bytes()))
    }

    /// Send one typed command APDU to a physical card.
    pub async fn card_command(
        &self,
        reader_name: Option<&str>,
        command: &CommandApdu,
    ) -> Result<ApduExchangeSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let reader_key = effective_reader.clone().unwrap_or_default();
        // Clone tracked secure-channel state before awaiting transport; session updates commit
        // afterward through the state store in one bounded synchronous step.
        let gp_secure_channel = self.state.card_gp_secure_channel(&reader_key)?;
        let response = self
            .transmit_card_command_with_optional_gp_auth(
                &user_config,
                effective_reader.as_deref(),
                gp_secure_channel.as_ref(),
                command,
            )
            .await?;
        let session_state = match self
            .state
            .record_card_command(&reader_key, command, &response)
        {
            Ok(session_state) => session_state,
            Err(_) => self.state.card_session_state_or_default(&reader_key)?,
        };
        Ok(ApduExchangeSummary {
            command: command.clone(),
            response,
            session_state,
        })
    }

    /// Route one card command through GP secure transport when a tracked GP session requires it.
    async fn transmit_card_command_with_optional_gp_auth(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        secure_channel: Option<&globalplatform::EstablishedSecureChannel>,
        command: &CommandApdu,
    ) -> Result<jcim_core::apdu::ResponseApdu> {
        if should_use_gp_secure_transport(command, secure_channel) {
            let secure_channel = secure_channel.expect("checked above");
            let keyset = ResolvedGpKeyset::resolve(Some(&secure_channel.keyset.name))?;
            match self
                .state
                .card_adapter
                .transmit_gp_secure_command(
                    user_config,
                    reader_name,
                    &keyset,
                    secure_channel.security_level.as_byte(),
                    command,
                )
                .await
            {
                Ok(response) => return Ok(response),
                Err(JcimError::Unsupported(_)) => {
                    return Err(JcimError::Unsupported(
                        "tracked GP secure channel requires authenticated GP transport support from the active physical-card adapter".to_string(),
                    ))
                }
                Err(error) => return Err(error),
            }
        }

        self.state
            .card_adapter
            .transmit_command(user_config, reader_name, command)
            .await
    }
}

/// Return whether the tracked GP session should wrap this command in secure transport.
fn should_use_gp_secure_transport(
    command: &CommandApdu,
    secure_channel: Option<&globalplatform::EstablishedSecureChannel>,
) -> bool {
    secure_channel.is_some()
        && matches!(
            iso7816::describe_command(command).domain,
            iso7816::CommandDomain::GlobalPlatform
        )
}

#[cfg(test)]
mod tests {
    use jcim_core::aid::Aid;
    use jcim_core::globalplatform::{self, GpKeysetMetadata, ScpMode, SecurityLevel};

    use super::should_use_gp_secure_transport;

    #[test]
    fn gp_transport_is_only_used_for_gp_commands_when_channel_is_tracked() {
        let channel = globalplatform::EstablishedSecureChannel {
            keyset: GpKeysetMetadata {
                name: "default".to_string(),
                mode: ScpMode::Scp02,
            },
            security_level: SecurityLevel::CommandMac,
            session_id: "session".to_string(),
        };

        assert!(should_use_gp_secure_transport(
            &globalplatform::initialize_update([0x00; 8]),
            Some(&channel)
        ));
        assert!(!should_use_gp_secure_transport(
            &jcim_core::iso7816::select_by_name(&Aid::new(vec![0xA0, 0x00, 0x00]).unwrap()),
            Some(&channel)
        ));
        assert!(!should_use_gp_secure_transport(
            &globalplatform::initialize_update([0x00; 8]),
            None
        ));
    }
}
