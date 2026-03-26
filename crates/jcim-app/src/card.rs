//! Physical-card adapter boundary for JCIM.

#![allow(clippy::missing_docs_in_private_items)]

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::process::Command;

use jcim_cap::prelude::CapPackage;
use jcim_config::project::UserConfig;
use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::error::{JcimError, Result};
use jcim_core::iso7816::{
    Atr, IsoCapabilities, IsoSessionState, ProtocolParameters, SecureMessagingProtocol,
    TransportProtocol, apply_response_to_session,
};
use jcim_core::{globalplatform, iso7816};

use crate::model::{
    CardAppletInventory, CardAppletSummary, CardPackageInventory, CardPackageSummary,
    CardReaderSummary, CardStatusSummary, ResetSummary,
};

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

/// One env-resolved GlobalPlatform keyset retained only in process memory.
#[derive(Clone)]
pub struct ResolvedGpKeyset {
    pub(crate) name: String,
    pub(crate) mode: globalplatform::ScpMode,
    enc_hex: String,
    mac_hex: String,
    dek_hex: String,
}

impl ResolvedGpKeyset {
    /// Resolve one named GP keyset from environment variables only.
    pub(crate) fn resolve(explicit_name: Option<&str>) -> Result<Self> {
        let name = match explicit_name {
            Some(name) if !name.trim().is_empty() => name.trim().to_string(),
            _ => std::env::var("JCIM_GP_DEFAULT_KEYSET").map_err(|_| {
                JcimError::Unsupported(
                    "missing GP keyset name: pass one explicitly or set JCIM_GP_DEFAULT_KEYSET"
                        .to_string(),
                )
            })?,
        };
        let env_prefix = format!("JCIM_GP_{}", gp_keyset_env_name(&name));
        let mode = match std::env::var(format!("{env_prefix}_MODE"))
            .map_err(|_| {
                JcimError::Unsupported(format!("missing {}_MODE for GP keyset {name}", env_prefix))
            })?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "scp02" => globalplatform::ScpMode::Scp02,
            "scp03" => globalplatform::ScpMode::Scp03,
            other => {
                return Err(JcimError::Unsupported(format!(
                    "unsupported GP mode `{other}` for keyset {name}"
                )));
            }
        };
        let enc_hex = required_gp_key_hex(&env_prefix, "ENC", &name)?;
        let mac_hex = required_gp_key_hex(&env_prefix, "MAC", &name)?;
        let dek_hex = required_gp_key_hex(&env_prefix, "DEK", &name)?;
        Ok(Self {
            name,
            mode,
            enc_hex,
            mac_hex,
            dek_hex,
        })
    }

    pub(crate) fn metadata(&self) -> globalplatform::GpKeysetMetadata {
        globalplatform::GpKeysetMetadata {
            name: self.name.clone(),
            mode: self.mode,
        }
    }

    pub(crate) fn protocol(&self) -> SecureMessagingProtocol {
        match self.mode {
            globalplatform::ScpMode::Scp02 => SecureMessagingProtocol::Scp02,
            globalplatform::ScpMode::Scp03 => SecureMessagingProtocol::Scp03,
        }
    }

    fn apply_helper_env(&self, command: &mut Command) {
        command
            .env("JCIM_GP_MODE", self.mode_label())
            .env("JCIM_GP_ENC", &self.enc_hex)
            .env("JCIM_GP_MAC", &self.mac_hex)
            .env("JCIM_GP_DEK", &self.dek_hex);
    }

    fn mode_label(&self) -> &'static str {
        match self.mode {
            globalplatform::ScpMode::Scp02 => "scp02",
            globalplatform::ScpMode::Scp03 => "scp03",
        }
    }
}

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

/// Deterministic in-memory test adapter for service and SDK integration tests.
#[derive(Clone)]
pub struct MockPhysicalCardAdapter {
    state: Arc<Mutex<MockCardState>>,
}

#[derive(Default)]
struct MockCardState {
    readers: Vec<CardReaderSummary>,
    protocol: String,
    atr_hex: String,
    iso_capabilities: IsoCapabilities,
    session_state: IsoSessionState,
    card_life_cycle: globalplatform::CardLifeCycle,
    packages: Vec<CardPackageSummary>,
    applets: Vec<CardAppletSummary>,
    locked_aids: HashSet<String>,
    pending_response: Vec<u8>,
    pending_get_status: Option<Vec<u8>>,
    binary_files: BTreeMap<u16, Vec<u8>>,
    record_files: BTreeMap<u16, Vec<Vec<u8>>>,
    data_objects: BTreeMap<(u8, u8), Vec<u8>>,
    reference_data: BTreeMap<u8, Vec<u8>>,
    retry_limits: BTreeMap<u8, u8>,
    retry_counters: BTreeMap<u8, u8>,
    challenge_counter: u32,
    pending_gp_auth: Option<PendingGpAuthState>,
}

#[derive(Clone)]
struct PendingGpAuthState {
    protocol: SecureMessagingProtocol,
    session_id: String,
}

impl MockPhysicalCardAdapter {
    /// Build a mock adapter with one default reader and blank card state.
    pub fn new() -> Self {
        let protocol = "T=1".to_string();
        let atr_hex = "3B800100".to_string();
        let iso_capabilities = IsoCapabilities {
            protocols: vec![TransportProtocol::T1],
            extended_length: true,
            logical_channels: true,
            max_logical_channels: 4,
            secure_messaging: true,
            file_model_visibility: true,
            raw_apdu: true,
        };
        Self {
            state: Arc::new(Mutex::new(MockCardState {
                readers: vec![CardReaderSummary {
                    name: "Mock Reader 0".to_string(),
                    card_present: true,
                }],
                session_state: mock_reset_session_state(&atr_hex, &protocol),
                protocol,
                atr_hex,
                iso_capabilities,
                card_life_cycle: globalplatform::CardLifeCycle::Secured,
                packages: Vec::new(),
                applets: Vec::new(),
                locked_aids: HashSet::new(),
                pending_response: Vec::new(),
                pending_get_status: None,
                binary_files: BTreeMap::from([(0x0101, b"JCIM mock EF".to_vec())]),
                record_files: BTreeMap::from([(
                    0x0201,
                    vec![b"record-1".to_vec(), b"record-2".to_vec()],
                )]),
                data_objects: BTreeMap::from([((0x00, 0x42), b"JCIM".to_vec())]),
                reference_data: BTreeMap::from([(0x80, b"1234".to_vec())]),
                retry_limits: BTreeMap::from([(0x80, 3)]),
                retry_counters: BTreeMap::from([(0x80, 3)]),
                challenge_counter: 0,
                pending_gp_auth: None,
            })),
        }
    }
}

impl Default for MockPhysicalCardAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PhysicalCardAdapter for MockPhysicalCardAdapter {
    async fn list_readers(&self, _user_config: &UserConfig) -> Result<Vec<CardReaderSummary>> {
        Ok(self.state.lock().map_err(lock_poisoned)?.readers.clone())
    }

    async fn card_status(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardStatusSummary> {
        let state = self.state.lock().map_err(lock_poisoned)?;
        let reader = reader_name
            .map(str::to_string)
            .or_else(|| state.readers.first().map(|reader| reader.name.clone()))
            .unwrap_or_default();
        Ok(CardStatusSummary {
            reader_name: reader,
            card_present: true,
            atr: state.session_state.atr.clone(),
            active_protocol: state.session_state.active_protocol.clone(),
            iso_capabilities: state.iso_capabilities.clone(),
            session_state: state.session_state.clone(),
            lines: vec![
                format!("Reader: {}", state.readers[0].name),
                "Card present: yes".to_string(),
                format!("Protocol: {}", state.protocol),
                format!("ATR: {}", state.atr_hex),
            ],
        })
    }

    async fn install_cap(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
        cap_path: &Path,
    ) -> Result<Vec<String>> {
        let cap = CapPackage::from_path(cap_path)?;
        let mut state = self.state.lock().map_err(lock_poisoned)?;
        state
            .packages
            .retain(|package| package.aid != cap.package_aid.to_hex());
        state.packages.push(CardPackageSummary {
            aid: cap.package_aid.to_hex(),
            description: format!(
                "{} {}.{}",
                cap.package_name, cap.package_major, cap.package_minor
            ),
        });
        for applet in cap.applets {
            let aid = applet.aid.to_hex();
            state.applets.retain(|existing| existing.aid != aid);
            state.applets.push(CardAppletSummary {
                aid,
                description: applet.name.unwrap_or_else(|| "InstalledApplet".to_string()),
            });
        }
        state.pending_get_status = None;
        Ok(vec![format!(
            "Installed CAP {} on {}",
            cap_path.display(),
            reader_name.unwrap_or("Mock Reader 0")
        )])
    }

    async fn delete_item(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
        aid: &str,
    ) -> Result<Vec<String>> {
        let mut state = self.state.lock().map_err(lock_poisoned)?;
        state.packages.retain(|package| package.aid != aid);
        state.applets.retain(|applet| applet.aid != aid);
        state.locked_aids.remove(aid);
        if state
            .session_state
            .selected_aid
            .as_ref()
            .is_some_and(|selected| selected.to_hex() == aid)
        {
            state.session_state.selected_aid = None;
            state.session_state.current_file = None;
            for channel in &mut state.session_state.open_channels {
                channel.selected_aid = None;
                channel.current_file = None;
            }
        }
        state.pending_get_status = None;
        Ok(vec![format!(
            "Deleted {aid} from {}",
            reader_name.unwrap_or("Mock Reader 0")
        )])
    }

    async fn list_packages(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardPackageInventory> {
        let state = self.state.lock().map_err(lock_poisoned)?;
        Ok(CardPackageInventory {
            reader_name: reader_name.unwrap_or("Mock Reader 0").to_string(),
            packages: state.packages.clone(),
            output_lines: state
                .packages
                .iter()
                .map(|package| format!("PKG: {} {}", package.aid, package.description))
                .collect(),
        })
    }

    async fn list_applets(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardAppletInventory> {
        let state = self.state.lock().map_err(lock_poisoned)?;
        Ok(CardAppletInventory {
            reader_name: reader_name.unwrap_or("Mock Reader 0").to_string(),
            applets: state.applets.clone(),
            output_lines: state
                .applets
                .iter()
                .map(|applet| format!("APP: {} {}", applet.aid, applet.description))
                .collect(),
        })
    }

    async fn transmit_apdu(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
        apdu_hex: &str,
    ) -> Result<String> {
        let apdu = CommandApdu::parse(&hex::decode(apdu_hex)?)?;
        let mut state = self.state.lock().map_err(lock_poisoned)?;
        let response = mock_dispatch_apdu(&mut state, &apdu)?;
        let _ = apply_response_to_session(&mut state.session_state, &apdu, &response);
        if let Some(protocol) = state
            .pending_gp_auth
            .as_ref()
            .map(|auth| auth.protocol.clone())
            && apdu.cla == 0x80
            && apdu.ins == 0x82
            && response.is_success()
        {
            state.session_state.secure_messaging.active = true;
            state.session_state.secure_messaging.protocol = Some(protocol);
            state.session_state.secure_messaging.security_level = Some(apdu.p1);
            state.session_state.secure_messaging.session_id = state
                .pending_gp_auth
                .as_ref()
                .map(|auth| auth.session_id.clone());
            state.session_state.secure_messaging.command_counter = 1;
            state.pending_gp_auth = None;
        }
        Ok(hex::encode_upper(response.to_bytes()))
    }

    async fn reset_card(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
    ) -> Result<String> {
        let mut state = self.state.lock().map_err(lock_poisoned)?;
        state.pending_response.clear();
        state.pending_get_status = None;
        state.pending_gp_auth = None;
        state.retry_counters = state.retry_limits.clone();
        state.session_state = mock_reset_session_state(&state.atr_hex, &state.protocol);
        Ok(state.atr_hex.clone())
    }

    async fn open_gp_secure_channel(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
        keyset: &ResolvedGpKeyset,
        security_level: u8,
    ) -> Result<()> {
        let mut state = self.state.lock().map_err(lock_poisoned)?;
        let session_id = format!("mock-gp-helper-{}", state.challenge_counter);
        let selected_aid = Some(Aid::from_slice(
            &globalplatform::ISSUER_SECURITY_DOMAIN_AID,
        )?);
        state.session_state.selected_aid = selected_aid.clone();
        state.session_state.current_file = None;
        if let Some(channel) = state
            .session_state
            .open_channels
            .iter_mut()
            .find(|channel| channel.channel_number == 0)
        {
            channel.selected_aid = selected_aid;
            channel.current_file = None;
        }
        state.session_state.secure_messaging.active = true;
        state.session_state.secure_messaging.protocol = Some(keyset.protocol());
        state.session_state.secure_messaging.security_level = Some(security_level);
        state.session_state.secure_messaging.session_id = Some(session_id);
        state.session_state.secure_messaging.command_counter = 0;
        state.pending_gp_auth = None;
        Ok(())
    }

    async fn transmit_gp_secure_command(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
        _keyset: &ResolvedGpKeyset,
        _security_level: u8,
        command: &CommandApdu,
    ) -> Result<ResponseApdu> {
        let mut state = self.state.lock().map_err(lock_poisoned)?;
        if !state.session_state.secure_messaging.active {
            return Err(JcimError::Unsupported(
                "mock card GP secure channel is not open".to_string(),
            ));
        }
        let response = mock_dispatch_apdu(&mut state, command)?;
        let _ = apply_response_to_session(&mut state.session_state, command, &response);
        state.session_state.secure_messaging.command_counter = state
            .session_state
            .secure_messaging
            .command_counter
            .saturating_add(1);
        Ok(response)
    }
}

/// Diagnostic output returned by the external card-management tools.
struct CardToolOutput {
    output_lines: Vec<String>,
}

pub(crate) fn helper_jar_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/jcim_card_helper/jcim-card-helper.jar")
}

pub(crate) fn gppro_jar_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third_party/gppro/gp.jar")
}

fn parse_package_inventory(lines: &[String]) -> Vec<CardPackageSummary> {
    lines
        .iter()
        .filter_map(|line| parse_inventory_item(line, "PKG: "))
        .map(|(aid, description)| CardPackageSummary { aid, description })
        .collect()
}

fn parse_applet_inventory(lines: &[String]) -> Vec<CardAppletSummary> {
    lines
        .iter()
        .filter_map(|line| parse_inventory_item(line, "APP: "))
        .map(|(aid, description)| CardAppletSummary { aid, description })
        .collect()
}

fn parse_inventory_item(line: &str, prefix: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix(prefix)?.trim();
    let mut parts = rest.split_whitespace();
    let aid = parts.next()?;
    let parsed = Aid::from_hex(aid).ok()?;
    let description = parts.collect::<Vec<_>>().join(" ");
    Some((parsed.to_hex(), description))
}

fn mock_reset_session_state(atr_hex: &str, protocol: &str) -> IsoSessionState {
    let atr = hex::decode(atr_hex)
        .ok()
        .and_then(|raw| Atr::parse(&raw).ok());
    let active_protocol =
        TransportProtocol::from_status_text(protocol).map(|protocol| ProtocolParameters {
            protocol: Some(protocol),
            ..ProtocolParameters::default()
        });
    IsoSessionState::reset(atr, active_protocol)
}

fn mock_dispatch_apdu(state: &mut MockCardState, apdu: &CommandApdu) -> Result<ResponseApdu> {
    if !mock_supported_cla(apdu.cla) {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::CLASS_NOT_SUPPORTED.as_u16(),
        ));
    }
    let logical_channel = iso7816::logical_channel_from_cla(apdu.cla);
    if apdu.ins != iso7816::INS_MANAGE_CHANNEL
        && logical_channel != 0
        && state
            .session_state
            .open_channels
            .iter()
            .all(|entry| entry.channel_number != logical_channel)
    {
        return Ok(ResponseApdu::status(0x6881));
    }

    match (apdu.cla, apdu.ins) {
        (0x80, 0xF2) => mock_get_status_response(state, apdu.p1, apdu.p2),
        (0x80, 0xF0) => mock_set_status_response(state, apdu),
        (0x80, 0x50) => mock_initialize_update_response(state, apdu),
        (0x80, 0x82) => mock_gp_external_authenticate_response(state, apdu),
        _ => mock_iso_response(state, apdu),
    }
}

fn mock_supported_cla(cla: u8) -> bool {
    cla == 0x80 || cla & 0x80 == 0
}

fn mock_iso_response(state: &mut MockCardState, apdu: &CommandApdu) -> Result<ResponseApdu> {
    Ok(match apdu.ins {
        iso7816::INS_SELECT => mock_select_response(state, apdu)?,
        iso7816::INS_MANAGE_CHANNEL => mock_manage_channel_response(state, apdu),
        iso7816::INS_GET_RESPONSE => mock_get_response_response(state, apdu),
        iso7816::INS_READ_BINARY => mock_read_binary_response(state, apdu),
        iso7816::INS_WRITE_BINARY | iso7816::INS_UPDATE_BINARY => {
            mock_write_binary_response(state, apdu)
        }
        iso7816::INS_ERASE_BINARY => mock_erase_binary_response(state, apdu),
        iso7816::INS_READ_RECORD => mock_read_record_response(state, apdu),
        iso7816::INS_UPDATE_RECORD => mock_update_record_response(state, apdu),
        iso7816::INS_APPEND_RECORD => mock_append_record_response(state, apdu),
        iso7816::INS_SEARCH_RECORD => mock_search_record_response(state, apdu),
        iso7816::INS_GET_DATA => mock_get_data_response(state, apdu),
        iso7816::INS_PUT_DATA => mock_put_data_response(state, apdu),
        iso7816::INS_VERIFY => mock_verify_response(state, apdu),
        iso7816::INS_CHANGE_REFERENCE_DATA => mock_change_reference_data_response(state, apdu),
        iso7816::INS_RESET_RETRY_COUNTER => mock_reset_retry_counter_response(state, apdu),
        iso7816::INS_INTERNAL_AUTHENTICATE => mock_internal_authenticate_response(state, apdu),
        iso7816::INS_EXTERNAL_AUTHENTICATE => mock_external_authenticate_response(apdu),
        iso7816::INS_GET_CHALLENGE => mock_get_challenge_response(state, apdu),
        iso7816::INS_ENVELOPE => mock_envelope_response(state, apdu),
        _ => ResponseApdu::status(iso7816::StatusWord::INSTRUCTION_NOT_SUPPORTED.as_u16()),
    })
}

fn mock_select_response(state: &MockCardState, apdu: &CommandApdu) -> Result<ResponseApdu> {
    match apdu.p1 {
        0x04 => {
            let requested_aid = hex::encode_upper(&apdu.data);
            if requested_aid == hex::encode_upper(globalplatform::ISSUER_SECURITY_DOMAIN_AID) {
                return Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()));
            }
            if matches!(
                state.card_life_cycle,
                globalplatform::CardLifeCycle::CardLocked
                    | globalplatform::CardLifeCycle::Terminated
            ) {
                return Ok(ResponseApdu::status(
                    iso7816::StatusWord::COMMAND_NOT_ALLOWED.as_u16(),
                ));
            }
            if state.locked_aids.contains(&requested_aid) {
                return Ok(ResponseApdu::status(
                    iso7816::StatusWord::WARNING_SELECTED_FILE_INVALIDATED.as_u16(),
                ));
            }
            if state
                .applets
                .iter()
                .any(|applet| applet.aid == requested_aid)
            {
                return Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()));
            }
            Ok(ResponseApdu::status(
                iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16(),
            ))
        }
        0x00 => {
            if apdu.data.len() != 2 {
                return Ok(ResponseApdu::status(
                    iso7816::StatusWord::WRONG_LENGTH.as_u16(),
                ));
            }
            let file_id = u16::from_be_bytes([apdu.data[0], apdu.data[1]]);
            if state.binary_files.contains_key(&file_id)
                || state.record_files.contains_key(&file_id)
            {
                Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()))
            } else {
                Ok(ResponseApdu::status(
                    iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16(),
                ))
            }
        }
        0x08 => {
            if apdu.data.len() < 2 || !apdu.data.len().is_multiple_of(2) {
                return Ok(ResponseApdu::status(
                    iso7816::StatusWord::WRONG_LENGTH.as_u16(),
                ));
            }
            let end = apdu.data.len();
            let file_id = u16::from_be_bytes([apdu.data[end - 2], apdu.data[end - 1]]);
            if state.binary_files.contains_key(&file_id)
                || state.record_files.contains_key(&file_id)
            {
                Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()))
            } else {
                Ok(ResponseApdu::status(
                    iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16(),
                ))
            }
        }
        _ => Ok(ResponseApdu::status(
            iso7816::StatusWord::INCORRECT_P1_P2.as_u16(),
        )),
    }
}

fn mock_manage_channel_response(state: &MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    if !state.iso_capabilities.logical_channels {
        return ResponseApdu::status(0x6881);
    }
    match apdu.p1 {
        0x00 if apdu.p2 == 0 && apdu.data.is_empty() => {
            for candidate in 1..state.iso_capabilities.max_logical_channels {
                if state
                    .session_state
                    .open_channels
                    .iter()
                    .all(|entry| entry.channel_number != candidate)
                {
                    return ResponseApdu::success(vec![candidate]);
                }
            }
            ResponseApdu::status(0x6A81)
        }
        0x80 if apdu.p2 != 0 && apdu.data.is_empty() => {
            if state
                .session_state
                .open_channels
                .iter()
                .any(|entry| entry.channel_number == apdu.p2)
            {
                ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
            } else {
                ResponseApdu::status(0x6881)
            }
        }
        _ => ResponseApdu::status(iso7816::StatusWord::INCORRECT_P1_P2.as_u16()),
    }
}

fn mock_get_response_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    if apdu.p1 != 0x00 || apdu.p2 != 0x00 {
        return ResponseApdu::status(iso7816::StatusWord::INCORRECT_P1_P2.as_u16());
    }
    if state.pending_response.is_empty() {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    }
    let expected_length = apdu.ne.unwrap_or(256);
    let take = expected_length.min(state.pending_response.len());
    let remaining = state.pending_response.split_off(take);
    let current = std::mem::replace(&mut state.pending_response, remaining);
    if state.pending_response.is_empty() {
        ResponseApdu::success(current)
    } else {
        let hinted = state.pending_response.len().min(256);
        ResponseApdu {
            data: current,
            sw: 0x6100 | if hinted == 256 { 0 } else { hinted as u16 },
        }
    }
}

fn mock_read_binary_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let Some(contents) = state.binary_files.get(&file_id) else {
        return ResponseApdu::status(iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16());
    };
    let offset = u16::from_be_bytes([apdu.p1, apdu.p2]) as usize;
    if offset > contents.len() {
        return ResponseApdu::status(iso7816::StatusWord::INCORRECT_P1_P2.as_u16());
    }
    mock_chunk_response(state, contents[offset..].to_vec(), apdu.ne)
}

fn mock_write_binary_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let offset = u16::from_be_bytes([apdu.p1, apdu.p2]) as usize;
    let entry = state.binary_files.entry(file_id).or_default();
    if entry.len() < offset {
        entry.resize(offset, 0x00);
    }
    let required_len = offset.saturating_add(apdu.data.len());
    if entry.len() < required_len {
        entry.resize(required_len, 0x00);
    }
    entry[offset..required_len].copy_from_slice(&apdu.data);
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_erase_binary_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let offset = u16::from_be_bytes([apdu.p1, apdu.p2]) as usize;
    let entry = state.binary_files.entry(file_id).or_default();
    if offset > entry.len() {
        return ResponseApdu::status(iso7816::StatusWord::INCORRECT_P1_P2.as_u16());
    }
    let erase_len = apdu.data.len().max(1);
    let end = offset.saturating_add(erase_len).min(entry.len());
    for byte in &mut entry[offset..end] {
        *byte = 0x00;
    }
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_read_record_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let Some(records) = state.record_files.get(&file_id) else {
        return ResponseApdu::status(iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16());
    };
    let record_number = usize::from(apdu.p1);
    if record_number == 0 || record_number > records.len() {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    mock_chunk_response(state, records[record_number - 1].clone(), apdu.ne)
}

fn mock_update_record_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let Some(records) = state.record_files.get_mut(&file_id) else {
        return ResponseApdu::status(iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16());
    };
    let record_number = usize::from(apdu.p1);
    if record_number == 0 || record_number > records.len() {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    records[record_number - 1] = apdu.data.clone();
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_append_record_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    state
        .record_files
        .entry(file_id)
        .or_default()
        .push(apdu.data.clone());
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_search_record_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let Some(records) = state.record_files.get(&file_id) else {
        return ResponseApdu::status(iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16());
    };
    let matches = records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            record
                .windows(apdu.data.len())
                .any(|window| window == apdu.data)
                .then_some((index + 1) as u8)
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    mock_chunk_response(state, matches, apdu.ne)
}

fn mock_get_data_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let key = (apdu.p1, apdu.p2);
    if let Some(data) = state.data_objects.get(&key).cloned() {
        return mock_chunk_response(state, data, apdu.ne);
    }
    ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16())
}

fn mock_put_data_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    state
        .data_objects
        .insert((apdu.p1, apdu.p2), apdu.data.clone());
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_verify_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let reference = apdu.p2;
    let Some(expected) = state.reference_data.get(&reference) else {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    };
    let remaining = *state.retry_counters.get(&reference).unwrap_or(&0);
    if remaining == 0 {
        return ResponseApdu::status(iso7816::StatusWord::AUTH_METHOD_BLOCKED.as_u16());
    }
    if apdu.data.is_empty() {
        return ResponseApdu::status(0x63C0 | u16::from(remaining));
    }
    if &apdu.data == expected {
        if let Some(limit) = state.retry_limits.get(&reference).copied() {
            state.retry_counters.insert(reference, limit);
        }
        ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
    } else {
        let updated = remaining.saturating_sub(1);
        state.retry_counters.insert(reference, updated);
        if updated == 0 {
            ResponseApdu::status(iso7816::StatusWord::AUTH_METHOD_BLOCKED.as_u16())
        } else {
            ResponseApdu::status(0x63C0 | u16::from(updated))
        }
    }
}

fn mock_change_reference_data_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> ResponseApdu {
    let reference = apdu.p2;
    if !state.reference_data.contains_key(&reference) {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    if !state.session_state.verified_references.contains(&reference) {
        return ResponseApdu::status(iso7816::StatusWord::SECURITY_STATUS_NOT_SATISFIED.as_u16());
    }
    if apdu.data.is_empty() {
        return ResponseApdu::status(iso7816::StatusWord::WRONG_LENGTH.as_u16());
    }
    state.reference_data.insert(reference, apdu.data.clone());
    if let Some(limit) = state.retry_limits.get(&reference).copied() {
        state.retry_counters.insert(reference, limit);
    }
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_reset_retry_counter_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> ResponseApdu {
    let reference = apdu.p2;
    if !state.reference_data.contains_key(&reference) {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    let isd_selected = state
        .session_state
        .selected_aid
        .as_ref()
        .is_some_and(|aid| aid.as_bytes() == globalplatform::ISSUER_SECURITY_DOMAIN_AID);
    if !isd_selected && !state.session_state.verified_references.contains(&reference) {
        return ResponseApdu::status(iso7816::StatusWord::SECURITY_STATUS_NOT_SATISFIED.as_u16());
    }
    if apdu.data.is_empty() {
        return ResponseApdu::status(iso7816::StatusWord::WRONG_LENGTH.as_u16());
    }
    state.reference_data.insert(reference, apdu.data.clone());
    if let Some(limit) = state.retry_limits.get(&reference).copied() {
        state.retry_counters.insert(reference, limit);
    }
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_get_challenge_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let expected_length = apdu.ne.unwrap_or(8).clamp(1, 32);
    let challenge = mock_deterministic_bytes(&mut state.challenge_counter, expected_length);
    mock_chunk_response(state, challenge, apdu.ne)
}

fn mock_internal_authenticate_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> ResponseApdu {
    let expected_length = apdu.ne.unwrap_or(apdu.data.len().max(8));
    let mut output = apdu.data.iter().rev().copied().collect::<Vec<_>>();
    while output.len() < expected_length {
        output.extend_from_slice(&apdu.data);
        if apdu.data.is_empty() {
            output.extend_from_slice(&mock_deterministic_bytes(&mut state.challenge_counter, 8));
        }
    }
    output.truncate(expected_length);
    mock_chunk_response(state, output, apdu.ne)
}

fn mock_external_authenticate_response(apdu: &CommandApdu) -> ResponseApdu {
    if apdu.data.is_empty() {
        ResponseApdu::status(iso7816::StatusWord::WRONG_LENGTH.as_u16())
    } else {
        ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
    }
}

fn mock_envelope_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let payload = apdu.data.iter().rev().copied().collect::<Vec<_>>();
    mock_chunk_response(state, payload, apdu.ne)
}

fn mock_get_status_response(state: &mut MockCardState, p1: u8, p2: u8) -> Result<ResponseApdu> {
    if p2 == 0x03 {
        if let Some(remaining) = state.pending_get_status.take() {
            return Ok(mock_chunk_registry_response(state, remaining));
        }
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16(),
        ));
    }
    if p2 != 0x02 {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::INCORRECT_P1_P2.as_u16(),
        ));
    }

    let mut data = Vec::new();
    match p1 {
        0x80 => {
            data.extend(mock_registry_entry(
                &Aid::from_slice(&globalplatform::ISSUER_SECURITY_DOMAIN_AID)?,
                mock_card_life_cycle_state(state.card_life_cycle),
                Some([0x9E, 0x00, 0x00]),
            )?);
        }
        0x40 => {
            for applet in &state.applets {
                let aid = Aid::from_hex(&applet.aid)?;
                let life_cycle_state = if state.locked_aids.contains(&applet.aid) {
                    0x83
                } else {
                    0x07
                };
                data.extend(mock_registry_entry(&aid, life_cycle_state, None)?);
            }
        }
        0x20 | 0x10 => {
            for package in &state.packages {
                let aid = Aid::from_hex(&package.aid)?;
                data.extend(mock_registry_entry(&aid, 0x01, None)?);
            }
        }
        _ => {
            return Ok(ResponseApdu::status(
                iso7816::StatusWord::INCORRECT_P1_P2.as_u16(),
            ));
        }
    }
    Ok(mock_chunk_registry_response(state, data))
}

fn mock_registry_entry(
    aid: &Aid,
    life_cycle_state: u8,
    privileges: Option<[u8; 3]>,
) -> Result<Vec<u8>> {
    let mut nested = vec![0x4F, aid.as_bytes().len() as u8];
    nested.extend_from_slice(aid.as_bytes());
    nested.extend_from_slice(&[0x9F, 0x70, 0x01, life_cycle_state]);
    if let Some(privileges) = privileges {
        nested.extend_from_slice(&[0xC5, 0x03]);
        nested.extend_from_slice(&privileges);
    }

    let mut entry = vec![0xE3];
    if nested.len() > usize::from(u8::MAX) {
        return Err(JcimError::Gp(
            "mock registry entry exceeded short-form BER-TLV length".to_string(),
        ));
    }
    entry.push(nested.len() as u8);
    entry.extend(nested);
    Ok(entry)
}

fn mock_set_status_response(state: &mut MockCardState, apdu: &CommandApdu) -> Result<ResponseApdu> {
    mock_apply_set_status(state, apdu)?;
    Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()))
}

fn mock_apply_set_status(state: &mut MockCardState, apdu: &CommandApdu) -> Result<()> {
    match apdu.p1 {
        0x80 => {
            state.card_life_cycle = match apdu.p2 {
                0x01 => globalplatform::CardLifeCycle::OpReady,
                0x07 => globalplatform::CardLifeCycle::Initialized,
                0x0F => globalplatform::CardLifeCycle::Secured,
                0x7F => globalplatform::CardLifeCycle::CardLocked,
                0xFF => globalplatform::CardLifeCycle::Terminated,
                other => {
                    return Err(JcimError::Gp(format!(
                        "unsupported mock card life cycle transition {:02X}",
                        other
                    )));
                }
            };
        }
        0x40 | 0x60 => {
            let aid = Aid::from_slice(&apdu.data)?.to_hex();
            match apdu.p2 {
                0x80 => {
                    state.locked_aids.insert(aid);
                }
                0x00 => {
                    state.locked_aids.remove(&aid);
                }
                other => {
                    return Err(JcimError::Gp(format!(
                        "unsupported mock application/security-domain state control {:02X}",
                        other
                    )));
                }
            }
        }
        other => {
            return Err(JcimError::Gp(format!(
                "unsupported mock SET STATUS target {:02X}",
                other
            )));
        }
    }
    state.pending_get_status = None;
    Ok(())
}

fn mock_initialize_update_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> Result<ResponseApdu> {
    if apdu.data.len() != 8 {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::WRONG_LENGTH.as_u16(),
        ));
    }
    let protocol = SecureMessagingProtocol::Scp03;
    let sequence = state.challenge_counter as u16;
    let card_challenge = mock_deterministic_bytes(&mut state.challenge_counter, 6);
    let mut data = vec![0x00; 10];
    data.push(0x01);
    data.push(0x03);
    data.extend_from_slice(&sequence.to_be_bytes());
    data.extend_from_slice(&card_challenge);
    data.extend_from_slice(&[0x00; 8]);
    state.pending_gp_auth = Some(PendingGpAuthState {
        protocol,
        session_id: format!("mock-gp-{}", sequence),
    });
    Ok(ResponseApdu::success(data))
}

fn mock_gp_external_authenticate_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> Result<ResponseApdu> {
    if state.pending_gp_auth.is_none() {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16(),
        ));
    }
    if apdu.data.len() != 8 {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::WRONG_LENGTH.as_u16(),
        ));
    }
    Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()))
}

fn mock_selected_file_id(state: &MockCardState) -> Option<u16> {
    match state.session_state.current_file.clone() {
        Some(iso7816::FileSelection::FileId(file_id)) => Some(file_id),
        Some(iso7816::FileSelection::Path(path)) if path.len() >= 2 => {
            let end = path.len();
            Some(u16::from_be_bytes([path[end - 2], path[end - 1]]))
        }
        _ => None,
    }
}

fn mock_chunk_registry_response(state: &mut MockCardState, data: Vec<u8>) -> ResponseApdu {
    const PAGE_BYTES: usize = 96;
    if data.len() > PAGE_BYTES {
        state.pending_get_status = Some(data[PAGE_BYTES..].to_vec());
        ResponseApdu {
            data: data[..PAGE_BYTES].to_vec(),
            sw: iso7816::StatusWord::MORE_DATA_AVAILABLE.as_u16(),
        }
    } else {
        state.pending_get_status = None;
        ResponseApdu::success(data)
    }
}

fn mock_chunk_response(
    state: &mut MockCardState,
    data: Vec<u8>,
    expected_length: Option<usize>,
) -> ResponseApdu {
    let Some(expected_length) = expected_length else {
        state.pending_response.clear();
        return ResponseApdu::success(data);
    };
    if data.len() <= expected_length {
        state.pending_response.clear();
        return ResponseApdu::success(data);
    }
    state.pending_response = data[expected_length..].to_vec();
    let hinted = state.pending_response.len().min(256);
    ResponseApdu {
        data: data[..expected_length].to_vec(),
        sw: 0x6100 | if hinted == 256 { 0 } else { hinted as u16 },
    }
}

fn mock_deterministic_bytes(counter: &mut u32, len: usize) -> Vec<u8> {
    let seed = *counter;
    *counter = counter.saturating_add(1);
    (0..len)
        .map(|offset| seed.wrapping_add(offset as u32) as u8)
        .collect()
}

fn mock_card_life_cycle_state(state: globalplatform::CardLifeCycle) -> u8 {
    match state {
        globalplatform::CardLifeCycle::OpReady => 0x01,
        globalplatform::CardLifeCycle::Initialized => 0x07,
        globalplatform::CardLifeCycle::Secured => 0x0F,
        globalplatform::CardLifeCycle::CardLocked => 0x7F,
        globalplatform::CardLifeCycle::Terminated => 0xFF,
    }
}

fn reader_arg_list(reader_name: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(reader_name) = reader_name {
        args.push("-r".to_string());
        args.push(reader_name.to_string());
    }
    args
}

async fn run_card_helper(
    user_config: &UserConfig,
    action: &str,
    reader_name: Option<&str>,
    extra_args: &[String],
) -> Result<String> {
    run_card_helper_with_env(user_config, action, reader_name, extra_args, None).await
}

async fn run_card_helper_with_env(
    user_config: &UserConfig,
    action: &str,
    reader_name: Option<&str>,
    extra_args: &[String],
    gp_keyset: Option<&ResolvedGpKeyset>,
) -> Result<String> {
    let mut command = Command::new(&user_config.java_bin);
    command
        .arg("-cp")
        .arg(helper_jar_path())
        .arg("jcim.cardhelper.Main")
        .arg(action)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(keyset) = gp_keyset {
        keyset.apply_helper_env(&mut command);
    }
    if let Some(reader_name) = reader_name {
        command.arg("--reader").arg(reader_name);
    }
    command.args(extra_args);
    run_command_to_string(command, format!("card helper {action}")).await
}

async fn run_gppro(user_config: &UserConfig, args: &[String]) -> Result<CardToolOutput> {
    let mut command = Command::new(&user_config.java_bin);
    command
        .arg("-jar")
        .arg(gppro_jar_path())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_command_to_string(command, "GPPro command".to_string()).await?;
    Ok(CardToolOutput {
        output_lines: output.lines().map(|line| line.to_string()).collect(),
    })
}

async fn run_command_to_string(mut command: Command, description: String) -> Result<String> {
    let output = command
        .output()
        .await
        .map_err(|error| JcimError::Unsupported(format!("unable to run {description}: {error}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stderr.trim().is_empty() {
        stdout.to_string()
    } else if stdout.trim().is_empty() {
        stderr.to_string()
    } else {
        format!("{stdout}{stderr}")
    };
    if output.status.success() {
        Ok(combined)
    } else {
        Err(JcimError::Unsupported(format!(
            "{description} failed with status {}: {}",
            output.status,
            combined.trim()
        )))
    }
}

fn lock_poisoned<T>(_: T) -> JcimError {
    JcimError::Unsupported("physical-card adapter state lock was poisoned".to_string())
}

fn gp_keyset_env_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn required_gp_key_hex(env_prefix: &str, suffix: &str, keyset_name: &str) -> Result<String> {
    let variable = format!("{env_prefix}_{suffix}");
    let value = std::env::var(&variable).map_err(|_| {
        JcimError::Unsupported(format!("missing {variable} for GP keyset {keyset_name}"))
    })?;
    let normalized = value.trim().to_string();
    let bytes = hex::decode(&normalized).map_err(|error| {
        JcimError::Unsupported(format!(
            "invalid {variable} for GP keyset {keyset_name}: {error}"
        ))
    })?;
    if !matches!(bytes.len(), 16 | 24 | 32) {
        return Err(JcimError::Unsupported(format!(
            "{variable} for GP keyset {keyset_name} must be 16, 24, or 32 bytes"
        )));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use jcim_config::project::UserConfig;
    use jcim_core::apdu::ResponseApdu;
    use jcim_core::{globalplatform, iso7816};

    use super::{
        MockPhysicalCardAdapter, PhysicalCardAdapter, ResolvedGpKeyset, parse_applet_inventory,
        parse_package_inventory,
    };

    #[test]
    fn parses_package_inventory_lines() {
        let packages = parse_package_inventory(&[
            "PKG: A000000151000000 demo.package 1.0".to_string(),
            "ISD: A000000003000000".to_string(),
        ]);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].aid, "A000000151000000");
        assert_eq!(packages[0].description, "demo.package 1.0");
    }

    #[test]
    fn parses_applet_inventory_lines() {
        let applets = parse_applet_inventory(&[
            "APP: A000000151000001 DemoApplet".to_string(),
            "APP: invalid broken".to_string(),
        ]);
        assert_eq!(applets.len(), 1);
        assert_eq!(applets[0].aid, "A000000151000001");
        assert_eq!(applets[0].description, "DemoApplet");
    }

    #[tokio::test]
    async fn mock_card_tracks_get_response_and_logical_channels() {
        let adapter = MockPhysicalCardAdapter::new();
        let user = UserConfig::default();

        let select = iso7816::select_file(0x0101);
        let select_response = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(select.to_bytes()))
            .await
            .expect("select response");
        assert_eq!(
            ResponseApdu::parse(&hex::decode(select_response).expect("hex"))
                .expect("response")
                .sw,
            0x9000
        );

        let read = iso7816::read_binary(0, 4);
        let read_response = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(read.to_bytes()))
            .await
            .expect("read response");
        let read_response =
            ResponseApdu::parse(&hex::decode(read_response).expect("hex")).expect("response");
        assert_eq!(read_response.data, b"JCIM");
        assert_eq!(read_response.sw, 0x6108);

        let get_response = iso7816::get_response(8);
        let get_response = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(get_response.to_bytes()))
            .await
            .expect("get response");
        let get_response =
            ResponseApdu::parse(&hex::decode(get_response).expect("hex")).expect("response");
        assert_eq!(get_response.data, b" mock EF");
        assert_eq!(get_response.sw, 0x9000);

        let open_channel = iso7816::manage_channel_open();
        let open_response = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(open_channel.to_bytes()))
            .await
            .expect("open channel");
        let open_response =
            ResponseApdu::parse(&hex::decode(open_response).expect("hex")).expect("response");
        assert_eq!(open_response.data, vec![1]);

        let status = adapter.card_status(&user, None).await.expect("status");
        assert!(
            status
                .session_state
                .open_channels
                .iter()
                .any(|entry| entry.channel_number == 1)
        );

        let close_channel = iso7816::manage_channel_close(1);
        let close_response = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(close_channel.to_bytes()))
            .await
            .expect("close channel");
        let close_response =
            ResponseApdu::parse(&hex::decode(close_response).expect("hex")).expect("response");
        assert_eq!(close_response.sw, 0x9000);
    }

    #[tokio::test]
    async fn mock_card_enforces_retry_counters_and_blocking() {
        let adapter = MockPhysicalCardAdapter::new();
        let user = UserConfig::default();

        let wrong = iso7816::verify(0x80, b"0000");
        let first = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(wrong.to_bytes()))
            .await
            .expect("first verify");
        let first = ResponseApdu::parse(&hex::decode(first).expect("hex")).expect("response");
        assert_eq!(first.sw, 0x63C2);

        let second = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(wrong.to_bytes()))
            .await
            .expect("second verify");
        let second = ResponseApdu::parse(&hex::decode(second).expect("hex")).expect("response");
        assert_eq!(second.sw, 0x63C1);

        let third = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(wrong.to_bytes()))
            .await
            .expect("third verify");
        let third = ResponseApdu::parse(&hex::decode(third).expect("hex")).expect("response");
        assert_eq!(third.sw, iso7816::StatusWord::AUTH_METHOD_BLOCKED.as_u16());
    }

    #[tokio::test]
    async fn mock_gp_get_status_supports_pagination() {
        let adapter = MockPhysicalCardAdapter::new();
        let user = UserConfig::default();
        {
            let mut state = adapter.state.lock().expect("lock");
            for suffix in 0u8..12 {
                state.applets.push(super::CardAppletSummary {
                    aid: format!("A0000001510000{:02X}", suffix),
                    description: format!("Applet {suffix}"),
                });
            }
        }

        let first = globalplatform::get_status(
            globalplatform::RegistryKind::Applications,
            globalplatform::GetStatusOccurrence::FirstOrAll,
        );
        let first = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(first.to_bytes()))
            .await
            .expect("first page");
        let first = ResponseApdu::parse(&hex::decode(first).expect("hex")).expect("response");
        assert_eq!(first.sw, iso7816::StatusWord::MORE_DATA_AVAILABLE.as_u16());

        let next = globalplatform::get_status(
            globalplatform::RegistryKind::Applications,
            globalplatform::GetStatusOccurrence::Next,
        );
        let next = adapter
            .transmit_apdu(&user, None, &hex::encode_upper(next.to_bytes()))
            .await
            .expect("next page");
        let next = ResponseApdu::parse(&hex::decode(next).expect("hex")).expect("response");
        assert!(matches!(next.sw, 0x9000 | 0x6310));
    }

    #[tokio::test]
    async fn mock_card_supports_helper_style_gp_auth_flow() {
        let adapter = MockPhysicalCardAdapter::new();
        let user = UserConfig::default();
        let keyset = ResolvedGpKeyset {
            name: "mock".to_string(),
            mode: globalplatform::ScpMode::Scp03,
            enc_hex: "404142434445464748494A4B4C4D4E4F".to_string(),
            mac_hex: "505152535455565758595A5B5C5D5E5F".to_string(),
            dek_hex: "606162636465666768696A6B6C6D6E6F".to_string(),
        };

        adapter
            .open_gp_secure_channel(&user, None, &keyset, 0x03)
            .await
            .expect("open gp secure channel");

        let summary = adapter.card_status(&user, None).await.expect("status");
        assert!(summary.session_state.secure_messaging.active);
        assert_eq!(
            summary.session_state.secure_messaging.protocol,
            Some(iso7816::SecureMessagingProtocol::Scp03)
        );

        let response = adapter
            .transmit_gp_secure_command(
                &user,
                None,
                &keyset,
                0x03,
                &globalplatform::get_status(
                    globalplatform::RegistryKind::Applications,
                    globalplatform::GetStatusOccurrence::FirstOrAll,
                ),
            )
            .await
            .expect("authenticated get status");
        assert!(matches!(response.sw, 0x9000 | 0x6310));

        let updated = adapter
            .card_status(&user, None)
            .await
            .expect("updated status");
        assert!(updated.session_state.secure_messaging.command_counter >= 1);
    }
}
