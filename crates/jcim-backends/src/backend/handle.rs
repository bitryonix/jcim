//! Public backend trait and async actor handle.

use jcim_config::config::RuntimeConfig;
use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::error::{JcimError, Result};
use jcim_core::iso7816::{Atr, IsoSessionState, SecureMessagingProtocol};
use jcim_core::model::{
    BackendHealth, InstallRequest, InstallResult, PackageSummary, PowerAction, ProtocolHandshake,
    ProtocolVersion, RuntimeSnapshot, VirtualAppletMetadata,
};
use tokio::sync::{mpsc, oneshot};

use super::actor::backend_actor_loop;
use super::external::ExternalBackend;

/// Synchronous backend behavior that the local service and embedded callers rely on.
///
/// # Why this exists
/// The local service and embedded readers need one shared capability contract regardless of
/// whether the implementation is in-process or managed as an external child process.
///
/// # Role in the system
/// Implemented by the maintained simulator adapter, then wrapped by [`BackendHandle`] for async
/// callers.
pub trait CardBackend: Send {
    /// Negotiate protocol compatibility and return the backend capability summary.
    fn handshake(&mut self, client_protocol: ProtocolVersion) -> Result<ProtocolHandshake>;
    /// Return the current backend health without sending APDU traffic.
    fn backend_health(&mut self) -> Result<BackendHealth>;
    /// Return the backend-owned ISO/IEC 7816 session state.
    fn get_session_state(&mut self) -> Result<IsoSessionState>;
    /// Send a typed APDU to the backend and return the updated session state.
    fn transmit_typed_apdu(&mut self, command: &CommandApdu) -> Result<BackendApduExchange>;
    /// Send one raw APDU byte sequence to the backend and return the updated session state.
    fn transmit_raw_apdu(&mut self, apdu: &[u8]) -> Result<BackendApduExchange>;
    /// Reset the card and return the typed reset result.
    fn reset(&mut self) -> Result<BackendResetResult>;
    /// Change power state and return the updated session state.
    fn set_power(&mut self, action: PowerAction) -> Result<BackendPowerResult>;
    /// Open or close one logical channel.
    fn manage_channel(
        &mut self,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<BackendApduExchange>;
    /// Mark secure messaging as active for later transmissions.
    fn open_secure_messaging(
        &mut self,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<BackendSecureMessagingSummary>;
    /// Advance the secure-messaging command counter.
    fn advance_secure_messaging(
        &mut self,
        increment_by: u32,
    ) -> Result<BackendSecureMessagingSummary>;
    /// Clear secure-messaging state.
    fn close_secure_messaging(&mut self) -> Result<BackendSecureMessagingSummary>;
    /// Install a CAP payload using the maintained typed request surface.
    fn install(&mut self, request: InstallRequest) -> Result<InstallResult>;
    /// Delete a package by package AID.
    fn delete_package(&mut self, aid: &Aid) -> Result<bool>;
    /// List applets visible to the backend.
    fn list_applets(&mut self) -> Result<Vec<VirtualAppletMetadata>>;
    /// List packages visible to the backend.
    fn list_packages(&mut self) -> Result<Vec<PackageSummary>>;
    /// Return a runtime snapshot for diagnostics and tooling.
    fn snapshot(&mut self) -> Result<RuntimeSnapshot>;
    /// Shut the backend down gracefully when possible.
    fn shutdown(&mut self) -> Result<()>;
}

/// Result of one backend-owned APDU exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendApduExchange {
    /// Response returned by the backend after any secure-message unwrap.
    pub response: ResponseApdu,
    /// Backend-owned ISO/IEC 7816 session state after the exchange.
    pub session_state: IsoSessionState,
}

/// Result of one backend reset operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendResetResult {
    /// Parsed ATR when the backend reported one.
    pub atr: Option<Atr>,
    /// Backend-owned ISO/IEC 7816 session state after the reset.
    pub session_state: IsoSessionState,
}

/// Result of one backend power-control operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendPowerResult {
    /// Parsed ATR when the backend powered on successfully.
    pub atr: Option<Atr>,
    /// Backend-owned ISO/IEC 7816 session state after the power transition.
    pub session_state: IsoSessionState,
}

/// Result of one backend secure-messaging state transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendSecureMessagingSummary {
    /// Backend-owned ISO/IEC 7816 session state after the transition.
    pub session_state: IsoSessionState,
}

/// Async façade over a backend actor thread.
///
/// # Why this exists
/// The local service and embedded readers speak async APIs, but the external simulator backend is
/// simpler to drive behind a single-threaded actor. This handle bridges those two models.
#[derive(Clone)]
pub struct BackendHandle {
    /// Sender used to forward commands to the actor thread.
    tx: mpsc::Sender<BackendCommand>,
}

/// Commands sent from async callers into the backend actor thread.
pub(super) enum BackendCommand {
    /// Request a backend handshake.
    Handshake {
        /// Protocol version expected by the caller.
        client_protocol: ProtocolVersion,
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<ProtocolHandshake>>,
    },
    /// Request a backend health probe.
    BackendHealth {
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<BackendHealth>>,
    },
    /// Request the current ISO session state.
    GetSessionState {
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<IsoSessionState>>,
    },
    /// Request typed APDU transmission.
    TransmitTypedApdu {
        /// Typed APDU payload to send.
        command: CommandApdu,
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<BackendApduExchange>>,
    },
    /// Request raw APDU transmission.
    TransmitRawApdu {
        /// Raw APDU payload to send.
        apdu: Vec<u8>,
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<BackendApduExchange>>,
    },
    /// Request a card reset.
    Reset {
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<BackendResetResult>>,
    },
    /// Request a power transition.
    SetPower {
        /// Requested power action.
        action: PowerAction,
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<BackendPowerResult>>,
    },
    /// Request logical-channel management.
    ManageChannel {
        /// Whether the request opens or closes a channel.
        open: bool,
        /// Optional channel number when closing.
        channel_number: Option<u8>,
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<BackendApduExchange>>,
    },
    /// Request secure-messaging activation.
    OpenSecureMessaging {
        /// Requested protocol family.
        protocol: Option<SecureMessagingProtocol>,
        /// Optional requested security level.
        security_level: Option<u8>,
        /// Optional session identifier.
        session_id: Option<String>,
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<BackendSecureMessagingSummary>>,
    },
    /// Request a secure-messaging counter advance.
    AdvanceSecureMessaging {
        /// Counter increment.
        increment_by: u32,
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<BackendSecureMessagingSummary>>,
    },
    /// Request secure-messaging shutdown.
    CloseSecureMessaging {
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<BackendSecureMessagingSummary>>,
    },
    /// Request a CAP install.
    Install {
        /// Install request payload.
        request: InstallRequest,
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<InstallResult>>,
    },
    /// Request package deletion.
    DeletePackage {
        /// Package AID to remove.
        aid: Aid,
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<bool>>,
    },
    /// Request an applet inventory.
    ListApplets {
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<Vec<VirtualAppletMetadata>>>,
    },
    /// Request a package inventory.
    ListPackages {
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<Vec<PackageSummary>>>,
    },
    /// Request a runtime snapshot.
    Snapshot {
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<RuntimeSnapshot>>,
    },
    /// Request backend shutdown.
    Shutdown {
        /// One-shot reply channel for the result.
        reply: oneshot::Sender<Result<()>>,
    },
}

impl BackendHandle {
    /// Build a backend handle from the maintained runtime configuration surface.
    pub fn from_config(config: RuntimeConfig) -> Result<Self> {
        match config.backend.kind {
            jcim_core::model::BackendKind::Simulator => {
                Self::spawn(ExternalBackend::spawn(config)?)
            }
            _ => Err(JcimError::Unsupported(
                "unsupported backend kind for backend handle".to_string(),
            )),
        }
    }

    /// Start a backend actor thread around the given backend implementation.
    fn spawn<B>(backend: B) -> Result<Self>
    where
        B: CardBackend + 'static,
    {
        let (tx, mut rx) = mpsc::channel(32);
        std::thread::Builder::new()
            .name("jcim-backend".to_string())
            .spawn(move || backend_actor_loop(Box::new(backend), &mut rx))
            .map_err(JcimError::from)?;
        Ok(Self { tx })
    }

    /// Negotiate protocol compatibility with the configured backend.
    pub async fn handshake(&self, client_protocol: ProtocolVersion) -> Result<ProtocolHandshake> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::Handshake {
            client_protocol,
            reply,
        })
        .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Query backend health without sending APDU traffic.
    pub async fn backend_health(&self) -> Result<BackendHealth> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::BackendHealth { reply }).await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Return the backend-owned ISO/IEC 7816 session state.
    pub async fn get_session_state(&self) -> Result<IsoSessionState> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::GetSessionState { reply }).await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Send one typed APDU to the selected backend.
    pub async fn transmit_typed_apdu(&self, command: CommandApdu) -> Result<BackendApduExchange> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::TransmitTypedApdu { command, reply })
            .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Send one raw APDU byte sequence to the selected backend.
    pub async fn transmit_raw_apdu(&self, apdu: Vec<u8>) -> Result<BackendApduExchange> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::TransmitRawApdu { apdu, reply })
            .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Reset the backend card and return the typed reset result.
    pub async fn reset(&self) -> Result<BackendResetResult> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::Reset { reply }).await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Change card power state and return the updated session result.
    pub async fn set_power(&self, action: PowerAction) -> Result<BackendPowerResult> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::SetPower { action, reply })
            .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Open or close one logical channel on the selected backend.
    pub async fn manage_channel(
        &self,
        open: bool,
        channel_number: Option<u8>,
    ) -> Result<BackendApduExchange> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::ManageChannel {
            open,
            channel_number,
            reply,
        })
        .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Mark secure messaging as active for later backend transmissions.
    pub async fn open_secure_messaging(
        &self,
        protocol: Option<SecureMessagingProtocol>,
        security_level: Option<u8>,
        session_id: Option<String>,
    ) -> Result<BackendSecureMessagingSummary> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::OpenSecureMessaging {
            protocol,
            security_level,
            session_id,
            reply,
        })
        .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Advance the backend secure-messaging command counter.
    pub async fn advance_secure_messaging(
        &self,
        increment_by: u32,
    ) -> Result<BackendSecureMessagingSummary> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::AdvanceSecureMessaging {
            increment_by,
            reply,
        })
        .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Clear backend secure-messaging state.
    pub async fn close_secure_messaging(&self) -> Result<BackendSecureMessagingSummary> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::CloseSecureMessaging { reply })
            .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Install a CAP payload through the selected backend.
    pub async fn install(&self, request: InstallRequest) -> Result<InstallResult> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::Install { request, reply })
            .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Delete a package by AID through the selected backend.
    pub async fn delete_package(&self, aid: Aid) -> Result<bool> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::DeletePackage { aid, reply })
            .await?;
        rx.await.map_err(actor_canceled)?
    }

    /// List applets visible to the selected backend.
    pub async fn list_applets(&self) -> Result<Vec<VirtualAppletMetadata>> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::ListApplets { reply }).await?;
        rx.await.map_err(actor_canceled)?
    }

    /// List packages visible to the selected backend.
    pub async fn list_packages(&self) -> Result<Vec<PackageSummary>> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::ListPackages { reply }).await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Return a runtime snapshot for diagnostics and tooling.
    pub async fn snapshot(&self) -> Result<RuntimeSnapshot> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::Snapshot { reply }).await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Shut the backend down and stop the actor thread.
    pub async fn shutdown(&self) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        self.send(BackendCommand::Shutdown { reply }).await?;
        rx.await.map_err(actor_canceled)?
    }

    /// Forward a command to the actor thread.
    async fn send(&self, command: BackendCommand) -> Result<()> {
        self.tx.send(command).await.map_err(|_| {
            JcimError::BackendUnavailable("backend actor is no longer running".to_string())
        })
    }
}

/// Convert a dropped actor reply channel into a stable backend-unavailable error.
pub(super) fn actor_canceled(_: oneshot::error::RecvError) -> JcimError {
    JcimError::BackendUnavailable("backend actor terminated before replying".to_string())
}
