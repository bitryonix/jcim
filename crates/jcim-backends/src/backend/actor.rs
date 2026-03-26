//! Actor loop that serializes access to a backend implementation.

use tokio::sync::mpsc;

use super::handle::{BackendCommand, CardBackend};

/// Run the backend actor loop until the command channel closes or shutdown is requested.
pub(super) fn backend_actor_loop(
    mut backend: Box<dyn CardBackend>,
    receiver: &mut mpsc::Receiver<BackendCommand>,
) {
    while let Some(command) = receiver.blocking_recv() {
        match command {
            BackendCommand::Handshake {
                client_protocol,
                reply,
            } => {
                let _ = reply.send(backend.handshake(client_protocol));
            }
            BackendCommand::BackendHealth { reply } => {
                let _ = reply.send(backend.backend_health());
            }
            BackendCommand::GetSessionState { reply } => {
                let _ = reply.send(backend.get_session_state());
            }
            BackendCommand::TransmitTypedApdu { command, reply } => {
                let _ = reply.send(backend.transmit_typed_apdu(&command));
            }
            BackendCommand::TransmitRawApdu { apdu, reply } => {
                let _ = reply.send(backend.transmit_raw_apdu(&apdu));
            }
            BackendCommand::Reset { reply } => {
                let _ = reply.send(backend.reset());
            }
            BackendCommand::SetPower { action, reply } => {
                let _ = reply.send(backend.set_power(action));
            }
            BackendCommand::ManageChannel {
                open,
                channel_number,
                reply,
            } => {
                let _ = reply.send(backend.manage_channel(open, channel_number));
            }
            BackendCommand::OpenSecureMessaging {
                protocol,
                security_level,
                session_id,
                reply,
            } => {
                let _ =
                    reply.send(backend.open_secure_messaging(protocol, security_level, session_id));
            }
            BackendCommand::AdvanceSecureMessaging {
                increment_by,
                reply,
            } => {
                let _ = reply.send(backend.advance_secure_messaging(increment_by));
            }
            BackendCommand::CloseSecureMessaging { reply } => {
                let _ = reply.send(backend.close_secure_messaging());
            }
            BackendCommand::Install { request, reply } => {
                let _ = reply.send(backend.install(request));
            }
            BackendCommand::DeletePackage { aid, reply } => {
                let _ = reply.send(backend.delete_package(&aid));
            }
            BackendCommand::ListApplets { reply } => {
                let _ = reply.send(backend.list_applets());
            }
            BackendCommand::ListPackages { reply } => {
                let _ = reply.send(backend.list_packages());
            }
            BackendCommand::Snapshot { reply } => {
                let _ = reply.send(backend.snapshot());
            }
            BackendCommand::Shutdown { reply } => {
                let result = backend.shutdown();
                let _ = reply.send(result);
                return;
            }
        }
    }

    let _ = backend.shutdown();
}
