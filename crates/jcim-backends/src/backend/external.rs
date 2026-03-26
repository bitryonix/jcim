//! External backend process supervision.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use jcim_config::config::RuntimeConfig;
use jcim_core::aid::Aid;
use jcim_core::apdu::ResponseApdu;
use jcim_core::error::{JcimError, Result};
use jcim_core::iso7816::{Atr, IsoSessionState};
use jcim_core::model::{
    BackendHealth, InstallRequest, InstallResult, PackageSummary, PowerAction, ProtocolHandshake,
    ProtocolVersion, RuntimeSnapshot, VirtualAppletMetadata,
};

use super::handle::{
    BackendApduExchange, BackendPowerResult, BackendResetResult, BackendSecureMessagingSummary,
    CardBackend,
};
use super::manifest::{BackendBundleManifest, resolve_classpath, validate_external_config};
use super::reply::{
    BackendApduExchangeWire, BackendOperation, BackendPowerResultWire, BackendReply,
    BackendRequest, BackendResetResultWire, BackendSecureMessagingSummaryWire, InstallRequestWire,
    RuntimeSnapshotWire, child_exit_error, ensure_reply_ok, ensure_reply_operation,
    read_reply_with_timeout, validate_protocol, write_request_line,
};

/// Running external backend child process plus cached handshake state.
pub(super) struct ExternalBackend {
    /// Child process launched from the backend bundle.
    child: Child,
    /// Stdin used for JSON-line control commands.
    stdin: ChildStdin,
    /// Buffered stdout used for reply parsing.
    stdout: BufReader<ChildStdout>,
    /// Handshake cached at startup so later requests can reuse capability data.
    handshake: ProtocolHandshake,
}

impl Drop for ExternalBackend {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl ExternalBackend {
    /// Launch an external backend bundle and complete its startup handshake.
    pub(super) fn spawn(config: RuntimeConfig) -> Result<Self> {
        let bundle_dir = config.backend_bundle_dir();
        let manifest_path = bundle_dir.join("manifest.toml");
        if !manifest_path.exists() {
            return Err(JcimError::BackendStartup(format!(
                "backend bundle manifest not found: {}",
                manifest_path.display()
            )));
        }

        let manifest: BackendBundleManifest =
            toml::from_str(&std::fs::read_to_string(&manifest_path)?).map_err(|error| {
                JcimError::BackendStartup(format!("invalid backend manifest: {error}"))
            })?;
        validate_external_config(&config, &manifest)?;

        let profile = config.resolve_profile();
        let mut command = build_launch_command(&config, &manifest, &bundle_dir, &profile)?;
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        for (key, value) in &manifest.env {
            command.env(key, value);
        }
        command.env("JCIM_BUNDLE_DIR", &bundle_dir);

        if let Some(path) = &config.cap_path {
            command.arg("--cap-path").arg(path);
        }
        if let Some(path) = &config.simulator_metadata_path {
            command.arg("--simulator-metadata").arg(path);
        }
        let mut child = command.spawn().map_err(|error| {
            JcimError::BackendStartup(format!("unable to spawn backend process: {error}"))
        })?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            JcimError::BackendStartup("backend process did not expose stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            JcimError::BackendStartup("backend process did not expose stdout".to_string())
        })?;
        let stdout = BufReader::new(stdout);

        write_request_line(
            &mut stdin,
            &BackendRequest::Handshake {
                client_protocol: ProtocolVersion::current(),
            },
        )?;
        let (stdout, reply) =
            read_reply_with_timeout(stdout, Duration::from_millis(manifest.startup_timeout_ms))?;
        ensure_reply_operation(&reply, BackendOperation::Handshake)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::Handshake {
            handshake: Some(handshake),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "backend handshake reply omitted handshake payload".to_string(),
            ));
        };

        validate_protocol(ProtocolVersion::current(), handshake.protocol_version)?;
        validate_protocol(manifest.protocol_version, handshake.protocol_version)?;
        if handshake.backend_kind != config.backend.kind {
            return Err(JcimError::BackendStartup(format!(
                "backend bundle reported {} but {} was requested",
                handshake.backend_kind, config.backend.kind
            )));
        }
        if !handshake.backend_capabilities.supported_profiles.is_empty()
            && !handshake
                .backend_capabilities
                .supported_profiles
                .contains(&config.profile_id)
        {
            return Err(JcimError::Unsupported(format!(
                "backend bundle does not support profile {}",
                config.profile_id
            )));
        }

        Ok(Self {
            child,
            stdin,
            stdout,
            handshake,
        })
    }

    /// Send one command and synchronously wait for its reply line.
    fn command(&mut self, request: BackendRequest) -> Result<BackendReply> {
        write_request_line(&mut self.stdin, &request)?;
        self.read_reply()
    }

    /// Read one reply line from the backend control stream.
    fn read_reply(&mut self) -> Result<BackendReply> {
        let mut line = String::new();
        let bytes = self.stdout.read_line(&mut line)?;
        if bytes == 0 {
            return Err(child_exit_error(
                &mut self.child,
                "backend process closed the control stream",
            ));
        }
        super::reply::parse_reply_line(&line)
    }
}

/// Build the OS process command for the selected external backend launcher.
fn build_launch_command(
    config: &RuntimeConfig,
    manifest: &BackendBundleManifest,
    bundle_dir: &Path,
    profile: &jcim_core::model::CardProfile,
) -> Result<Command> {
    let mut command = build_java_command(config, manifest, bundle_dir)?;

    command
        .args(&manifest.args)
        .arg("--backend-kind")
        .arg(config.backend.kind.display_name())
        .arg("--profile-id")
        .arg(profile.id.to_string())
        .arg("--version")
        .arg(profile.version.display_name())
        .arg("--reader-name")
        .arg(&profile.reader_name)
        .arg("--atr")
        .arg(hex::encode_upper(&profile.hardware.atr));
    Ok(command)
}

/// Build the Java-specific command-line for a manifest that launches through the JVM.
fn build_java_command(
    config: &RuntimeConfig,
    manifest: &BackendBundleManifest,
    bundle_dir: &Path,
) -> Result<Command> {
    let classpath = resolve_classpath(bundle_dir, &manifest.classpath)?;
    if classpath.is_empty() {
        return Err(JcimError::BackendStartup(format!(
            "no backend classpath entries resolved from {}",
            bundle_dir.display()
        )));
    }

    let classpath_sep = if cfg!(windows) { ";" } else { ":" };
    let mut command = Command::new(&config.backend.java_bin);
    command
        .arg("-cp")
        .arg(classpath.join(classpath_sep))
        .arg(&manifest.main_class);
    Ok(command)
}

impl CardBackend for ExternalBackend {
    fn handshake(&mut self, client_protocol: ProtocolVersion) -> Result<ProtocolHandshake> {
        validate_protocol(client_protocol, self.handshake.protocol_version)?;
        Ok(self.handshake.clone())
    }

    fn backend_health(&mut self) -> Result<BackendHealth> {
        let reply = self.command(BackendRequest::Health)?;
        ensure_reply_operation(&reply, BackendOperation::Health)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::Health {
            health: Some(health),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "backend health reply omitted payload".to_string(),
            ));
        };
        Ok(health)
    }

    fn get_session_state(&mut self) -> Result<IsoSessionState> {
        let reply = self.command(BackendRequest::GetSessionState)?;
        ensure_reply_operation(&reply, BackendOperation::GetSessionState)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::GetSessionState {
            session_state: Some(session_state),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "backend session-state reply omitted payload".to_string(),
            ));
        };
        session_state.try_into()
    }

    fn transmit_typed_apdu(
        &mut self,
        command: &jcim_core::apdu::CommandApdu,
    ) -> Result<BackendApduExchange> {
        let reply = self.command(BackendRequest::TransmitTyped {
            raw_hex: hex::encode_upper(command.to_bytes()),
            command: command.clone(),
        })?;
        ensure_reply_operation(&reply, BackendOperation::TransmitTyped)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::TransmitTyped {
            exchange: Some(exchange),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "typed APDU reply omitted exchange payload".to_string(),
            ));
        };
        parse_apdu_exchange(exchange)
    }

    fn transmit_raw_apdu(&mut self, apdu: &[u8]) -> Result<BackendApduExchange> {
        let reply = self.command(BackendRequest::TransmitRaw {
            apdu_hex: hex::encode_upper(apdu),
        })?;
        ensure_reply_operation(&reply, BackendOperation::TransmitRaw)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::TransmitRaw {
            exchange: Some(exchange),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "raw APDU reply omitted exchange payload".to_string(),
            ));
        };
        parse_apdu_exchange(exchange)
    }

    fn reset(&mut self) -> Result<BackendResetResult> {
        let reply = self.command(BackendRequest::Reset)?;
        ensure_reply_operation(&reply, BackendOperation::Reset)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::Reset {
            reset: Some(reset), ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "reset reply omitted payload".to_string(),
            ));
        };
        parse_reset_result(reset)
    }

    fn set_power(&mut self, action: PowerAction) -> Result<BackendPowerResult> {
        let reply = self.command(BackendRequest::Power { action })?;
        ensure_reply_operation(&reply, BackendOperation::Power)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::Power {
            power: Some(power), ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "power reply omitted payload".to_string(),
            ));
        };
        parse_power_result(power)
    }

    fn manage_channel(
        &mut self,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<BackendApduExchange> {
        let reply = self.command(BackendRequest::ManageChannel {
            open,
            channel_number,
        })?;
        ensure_reply_operation(&reply, BackendOperation::ManageChannel)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::ManageChannel {
            exchange: Some(exchange),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "manage-channel reply omitted payload".to_string(),
            ));
        };
        parse_apdu_exchange(exchange)
    }

    fn open_secure_messaging(
        &mut self,
        protocol: Option<jcim_core::iso7816::SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<BackendSecureMessagingSummary> {
        let reply = self.command(BackendRequest::OpenSecureMessaging {
            protocol,
            security_level,
            session_id,
        })?;
        ensure_reply_operation(&reply, BackendOperation::OpenSecureMessaging)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::OpenSecureMessaging {
            secure_messaging: Some(summary),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "open secure-messaging reply omitted payload".to_string(),
            ));
        };
        parse_secure_messaging_summary(summary)
    }

    fn advance_secure_messaging(
        &mut self,
        increment_by: u32,
    ) -> Result<BackendSecureMessagingSummary> {
        let reply = self.command(BackendRequest::AdvanceSecureMessaging { increment_by })?;
        ensure_reply_operation(&reply, BackendOperation::AdvanceSecureMessaging)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::AdvanceSecureMessaging {
            secure_messaging: Some(summary),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "advance secure-messaging reply omitted payload".to_string(),
            ));
        };
        parse_secure_messaging_summary(summary)
    }

    fn close_secure_messaging(&mut self) -> Result<BackendSecureMessagingSummary> {
        let reply = self.command(BackendRequest::CloseSecureMessaging)?;
        ensure_reply_operation(&reply, BackendOperation::CloseSecureMessaging)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::CloseSecureMessaging {
            secure_messaging: Some(summary),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "close secure-messaging reply omitted payload".to_string(),
            ));
        };
        parse_secure_messaging_summary(summary)
    }

    fn install(&mut self, request: InstallRequest) -> Result<InstallResult> {
        let reply = self.command(BackendRequest::Install {
            request: InstallRequestWire::from(&request),
        })?;
        ensure_reply_operation(&reply, BackendOperation::Install)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::Install {
            install: Some(install),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "install reply omitted payload".to_string(),
            ));
        };
        Ok(install)
    }

    fn delete_package(&mut self, aid: &Aid) -> Result<bool> {
        let reply = self.command(BackendRequest::DeletePackage { aid: aid.clone() })?;
        ensure_reply_operation(&reply, BackendOperation::DeletePackage)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::DeletePackage { deleted, .. } = reply else {
            return Err(JcimError::MalformedBackendReply(
                "delete-package reply had the wrong shape".to_string(),
            ));
        };
        Ok(deleted)
    }

    fn list_applets(&mut self) -> Result<Vec<VirtualAppletMetadata>> {
        let reply = self.command(BackendRequest::ListApplets)?;
        ensure_reply_operation(&reply, BackendOperation::ListApplets)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::ListApplets { applets, .. } = reply else {
            return Err(JcimError::MalformedBackendReply(
                "list-applets reply had the wrong shape".to_string(),
            ));
        };
        Ok(applets)
    }

    fn list_packages(&mut self) -> Result<Vec<PackageSummary>> {
        let reply = self.command(BackendRequest::ListPackages)?;
        ensure_reply_operation(&reply, BackendOperation::ListPackages)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::ListPackages { packages, .. } = reply else {
            return Err(JcimError::MalformedBackendReply(
                "list-packages reply had the wrong shape".to_string(),
            ));
        };
        Ok(packages)
    }

    fn snapshot(&mut self) -> Result<RuntimeSnapshot> {
        let reply = self.command(BackendRequest::Snapshot)?;
        ensure_reply_operation(&reply, BackendOperation::Snapshot)?;
        ensure_reply_ok(&reply)?;
        let BackendReply::Snapshot {
            snapshot: Some(snapshot),
            ..
        } = reply
        else {
            return Err(JcimError::MalformedBackendReply(
                "snapshot reply omitted payload".to_string(),
            ));
        };
        parse_runtime_snapshot(snapshot)
    }

    fn shutdown(&mut self) -> Result<()> {
        let _ = self.command(BackendRequest::Shutdown);
        match self.child.try_wait()? {
            Some(_) => Ok(()),
            None => {
                let _ = self.child.kill();
                let _ = self.child.wait();
                Ok(())
            }
        }
    }
}

/// Convert one backend-wire APDU exchange into the maintained adapter result type.
fn parse_apdu_exchange(exchange: BackendApduExchangeWire) -> Result<BackendApduExchange> {
    Ok(BackendApduExchange {
        response: ResponseApdu::parse(&hex::decode(&exchange.response_hex)?)?,
        session_state: exchange.session_state.try_into()?,
    })
}

/// Parse one optional ATR hex string returned by the backend control stream.
fn parse_optional_atr(atr_hex: Option<String>) -> Result<Option<Atr>> {
    atr_hex
        .map(|value| {
            let raw = hex::decode(&value)?;
            Atr::parse(&raw)
        })
        .transpose()
}

/// Convert one backend-wire reset payload into the maintained typed reset summary.
fn parse_reset_result(reset: BackendResetResultWire) -> Result<BackendResetResult> {
    let session_state: IsoSessionState = reset.session_state.try_into()?;
    Ok(BackendResetResult {
        atr: parse_optional_atr(reset.atr_hex)?.or_else(|| session_state.atr.clone()),
        session_state,
    })
}

/// Convert one backend-wire power payload into the maintained typed power summary.
fn parse_power_result(power: BackendPowerResultWire) -> Result<BackendPowerResult> {
    let session_state: IsoSessionState = power.session_state.try_into()?;
    Ok(BackendPowerResult {
        atr: parse_optional_atr(power.atr_hex)?.or_else(|| session_state.atr.clone()),
        session_state,
    })
}

/// Convert one backend-wire secure-messaging payload into the maintained summary type.
fn parse_secure_messaging_summary(
    summary: BackendSecureMessagingSummaryWire,
) -> Result<BackendSecureMessagingSummary> {
    Ok(BackendSecureMessagingSummary {
        session_state: summary.session_state.try_into()?,
    })
}

/// Convert one backend-wire snapshot into the maintained runtime snapshot model.
pub(super) fn parse_runtime_snapshot(snapshot: RuntimeSnapshotWire) -> Result<RuntimeSnapshot> {
    snapshot.try_into()
}
