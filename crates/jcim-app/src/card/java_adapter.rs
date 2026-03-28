use super::helper_tool::{reader_arg_list, run_card_helper, run_card_helper_with_env, run_gppro};
use super::inventory_parser::{parse_applet_inventory, parse_package_inventory};
use super::*;

/// Default adapter backed by the bundled card helper and GPPro.
#[derive(Default)]
pub(crate) struct JavaPhysicalCardAdapter;

#[async_trait]
impl PhysicalCardAdapter for JavaPhysicalCardAdapter {
    async fn list_readers(&self, user_config: &UserConfig) -> Result<Vec<CardReaderSummary>> {
        let output = run_card_helper(user_config, "readers", None, &[]).await?;
        let mut readers = Vec::new();
        for line in output.lines() {
            let Some((name, present)) = line.split_once('\t') else {
                continue;
            };
            readers.push(CardReaderSummary {
                name: name.to_string(),
                card_present: present.trim() == "present=1",
            });
        }
        Ok(readers)
    }

    async fn card_status(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardStatusSummary> {
        let output = run_card_helper(user_config, "status", reader_name, &[]).await?;
        let lines = output
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        let mut effective_reader = reader_name.unwrap_or_default().to_string();
        let mut card_present = false;
        let mut protocol_text = String::new();
        let mut atr_hex = String::new();
        for line in &lines {
            if let Some(value) = line.strip_prefix("Reader: ") {
                effective_reader = value.to_string();
            } else if let Some(value) = line.strip_prefix("Card present: ") {
                card_present = value == "yes";
            } else if let Some(value) = line.strip_prefix("Protocol: ") {
                protocol_text = value.to_string();
            } else if let Some(value) = line.strip_prefix("ATR: ") {
                atr_hex = value.to_string();
            }
        }
        let atr = hex::decode(&atr_hex)
            .ok()
            .and_then(|raw| Atr::parse(&raw).ok());
        let active_protocol = atr.as_ref().map(ProtocolParameters::from_atr).or_else(|| {
            TransportProtocol::from_status_text(&protocol_text).map(|protocol| ProtocolParameters {
                protocol: Some(protocol),
                ..ProtocolParameters::default()
            })
        });
        let session_state = if card_present {
            IsoSessionState::reset(atr.clone(), active_protocol.clone())
        } else {
            IsoSessionState::default()
        };
        let iso_capabilities = IsoCapabilities {
            protocols: active_protocol
                .as_ref()
                .and_then(|protocol| protocol.protocol)
                .into_iter()
                .collect(),
            extended_length: false,
            logical_channels: false,
            max_logical_channels: 1,
            secure_messaging: false,
            file_model_visibility: false,
            raw_apdu: true,
        };
        Ok(CardStatusSummary {
            reader_name: effective_reader,
            card_present,
            atr,
            active_protocol,
            iso_capabilities,
            session_state,
            lines,
        })
    }

    async fn install_cap(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        cap_path: &Path,
    ) -> Result<Vec<String>> {
        let mut args = reader_arg_list(reader_name);
        args.push("-install".to_string());
        args.push(cap_path.display().to_string());
        Ok(run_gppro(user_config, &args).await?.output_lines)
    }

    async fn delete_item(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        aid: &str,
    ) -> Result<Vec<String>> {
        let mut args = reader_arg_list(reader_name);
        args.push("-delete".to_string());
        args.push(aid.to_string());
        Ok(run_gppro(user_config, &args).await?.output_lines)
    }

    async fn list_packages(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardPackageInventory> {
        let mut args = reader_arg_list(reader_name);
        args.push("-l".to_string());
        let output = run_gppro(user_config, &args).await?;
        Ok(CardPackageInventory {
            reader_name: reader_name.unwrap_or_default().to_string(),
            packages: parse_package_inventory(&output.output_lines),
            output_lines: output.output_lines,
        })
    }

    async fn list_applets(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardAppletInventory> {
        let mut args = reader_arg_list(reader_name);
        args.push("-l".to_string());
        let output = run_gppro(user_config, &args).await?;
        Ok(CardAppletInventory {
            reader_name: reader_name.unwrap_or_default().to_string(),
            applets: parse_applet_inventory(&output.output_lines),
            output_lines: output.output_lines,
        })
    }

    async fn transmit_apdu(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        apdu_hex: &str,
    ) -> Result<String> {
        let args = vec!["--hex".to_string(), apdu_hex.to_string()];
        let output = run_card_helper(user_config, "apdu", reader_name, &args).await?;
        output
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .ok_or_else(|| {
                JcimError::Unsupported("card helper returned no APDU response".to_string())
            })
    }

    async fn reset_card(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<String> {
        let output = run_card_helper(user_config, "reset", reader_name, &[]).await?;
        output
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .ok_or_else(|| {
                JcimError::Unsupported("card helper returned no ATR after reset".to_string())
            })
    }

    async fn open_gp_secure_channel(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        keyset: &ResolvedGpKeyset,
        security_level: u8,
    ) -> Result<()> {
        let args = vec![
            "--security-level".to_string(),
            format!("0x{security_level:02X}"),
        ];
        let _ = run_card_helper_with_env(
            user_config,
            "gp-auth-open",
            reader_name,
            &args,
            Some(keyset),
        )
        .await?;
        Ok(())
    }

    async fn transmit_gp_secure_command(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        keyset: &ResolvedGpKeyset,
        security_level: u8,
        command: &CommandApdu,
    ) -> Result<ResponseApdu> {
        let args = vec![
            "--security-level".to_string(),
            format!("0x{security_level:02X}"),
            "--hex".to_string(),
            hex::encode_upper(command.to_bytes()),
        ];
        let output = run_card_helper_with_env(
            user_config,
            "gp-secure-apdu",
            reader_name,
            &args,
            Some(keyset),
        )
        .await?;
        let response_hex = output
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .ok_or_else(|| {
                JcimError::Unsupported(
                    "card helper returned no authenticated GP APDU response".to_string(),
                )
            })?;
        ResponseApdu::parse(&hex::decode(&response_hex)?)
    }
}
