//! Integration coverage for fail-closed RPC request validation.

#![forbid(unsafe_code)]

#[path = "../../../tests/support/socket.rs"]
mod socket_support;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use jcim_api::v0_3::card_service_client::CardServiceClient;
use jcim_api::v0_3::install_cap_request::Input as InstallCapInput;
use jcim_api::v0_3::simulator_service_client::SimulatorServiceClient;
use jcim_api::v0_3::{
    CardApduRequest, CardManageChannelRequest, CardRawApduRequest, CardSecureMessagingRequest,
    CommandApduCase, CommandApduFrame, CommandDomain, CommandKind, InstallCapRequest,
    ManageChannelRequest, SecureMessagingRequest, SimulationSelector, TransmitApduRequest,
    TransmitRawApduRequest,
};
use jcim_app::JcimApp;
use jcim_config::project::ManagedPaths;
use jcim_core::apdu::CommandApdu;
use jcim_core::iso7816;

#[tokio::test]
async fn rpc_validation_paths_fail_closed_with_invalid_argument() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "rpc_validation_paths_fail_closed_with_invalid_argument",
    ) {
        return;
    }

    let root = temp_root("rpc-validation");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let socket_path = managed_paths.service_socket_path.clone();
    let app = JcimApp::load_with_paths(managed_paths.clone()).expect("load app");

    let mut server =
        tokio::spawn(async move { jcimd::serve_local_service(app, &socket_path).await });
    socket_support::wait_for_socket_or_server_exit(&managed_paths.service_socket_path, &mut server)
        .await;

    let channel = connect_channel(&managed_paths.service_socket_path)
        .await
        .expect("connect");
    let mut card = CardServiceClient::new(channel.clone());
    let mut simulator = SimulatorServiceClient::new(channel);

    expect_invalid_argument(
        card.install_cap(InstallCapRequest {
            reader_name: String::new(),
            input: None,
        })
        .await
        .expect_err("missing card install input should fail"),
        "provide a project selector or CAP path",
    );

    expect_invalid_argument(
        card.manage_channel(CardManageChannelRequest {
            reader_name: String::new(),
            open: true,
            channel_number: Some(300),
        })
        .await
        .expect_err("out-of-range card channel should fail"),
        "channel number must fit in one byte",
    );

    expect_invalid_argument(
        card.open_secure_messaging(CardSecureMessagingRequest {
            reader_name: String::new(),
            protocol: jcim_api::v0_3::SecureMessagingProtocol::Iso7816 as i32,
            security_level: Some(300),
            session_id: String::new(),
            protocol_label: String::new(),
        })
        .await
        .expect_err("out-of-range card secure messaging level should fail"),
        "secure messaging level must fit in one byte",
    );

    expect_invalid_argument(
        card.transmit_apdu(CardApduRequest {
            reader_name: String::new(),
            command: Some(mismatched_select_command()),
        })
        .await
        .expect_err("card APDU metadata mismatch should fail"),
        "command kind",
    );

    expect_invalid_argument(
        card.transmit_raw_apdu(CardRawApduRequest {
            reader_name: String::new(),
            apdu: vec![0x00],
        })
        .await
        .expect_err("invalid raw card APDU should fail"),
        "at least 4 bytes",
    );

    expect_invalid_argument(
        simulator
            .manage_channel(ManageChannelRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: "sim-1".to_string(),
                }),
                open: true,
                channel_number: Some(300),
            })
            .await
            .expect_err("out-of-range simulation channel should fail"),
        "channel number must fit in one byte",
    );

    expect_invalid_argument(
        simulator
            .open_secure_messaging(SecureMessagingRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: "sim-1".to_string(),
                }),
                protocol: jcim_api::v0_3::SecureMessagingProtocol::Iso7816 as i32,
                security_level: Some(300),
                session_id: String::new(),
                protocol_label: String::new(),
            })
            .await
            .expect_err("out-of-range simulation secure messaging level should fail"),
        "secure messaging level must fit in one byte",
    );

    expect_invalid_argument(
        simulator
            .transmit_apdu(TransmitApduRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: "sim-1".to_string(),
                }),
                command: Some(mismatched_select_command()),
            })
            .await
            .expect_err("simulation APDU metadata mismatch should fail"),
        "command kind",
    );

    expect_invalid_argument(
        simulator
            .transmit_raw_apdu(TransmitRawApduRequest {
                simulation: Some(SimulationSelector {
                    simulation_id: "sim-1".to_string(),
                }),
                apdu: vec![0x00],
            })
            .await
            .expect_err("invalid raw simulation APDU should fail"),
        "at least 4 bytes",
    );

    // This keeps the import path anchored to the generated request enum so future contract drift
    // shows up as a compile error in this validation suite.
    let _ = std::mem::size_of::<InstallCapInput>();

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

fn expect_invalid_argument(error: tonic::Status, message_fragment: &str) {
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    assert!(
        error.message().contains(message_fragment),
        "expected `{}` in `{}`",
        message_fragment,
        error.message()
    );
}

fn mismatched_select_command() -> CommandApduFrame {
    let command = CommandApdu::parse(&[
        0x00, 0xA4, 0x04, 0x00, 0x09, 0x53, 0x61, 0x74, 0x6F, 0x43, 0x68, 0x69, 0x70, 0x00,
    ])
    .expect("parse select");
    let descriptor = iso7816::describe_command(&command);

    CommandApduFrame {
        raw: command.to_bytes(),
        cla: u32::from(command.cla),
        ins: u32::from(command.ins),
        p1: u32::from(command.p1),
        p2: u32::from(command.p2),
        data: command.data.clone(),
        ne: command.ne.map(|value| value as u32),
        encoding: jcim_api::v0_3::ApduEncoding::Short as i32,
        apdu_case: match command.apdu_case() {
            jcim_core::apdu::CommandApduCase::Case1 => CommandApduCase::CommandApduCase1 as i32,
            jcim_core::apdu::CommandApduCase::Case2Short => {
                CommandApduCase::CommandApduCase2Short as i32
            }
            jcim_core::apdu::CommandApduCase::Case3Short => {
                CommandApduCase::CommandApduCase3Short as i32
            }
            jcim_core::apdu::CommandApduCase::Case4Short => {
                CommandApduCase::CommandApduCase4Short as i32
            }
            jcim_core::apdu::CommandApduCase::Case2Extended => {
                CommandApduCase::CommandApduCase2Extended as i32
            }
            jcim_core::apdu::CommandApduCase::Case3Extended => {
                CommandApduCase::CommandApduCase3Extended as i32
            }
            jcim_core::apdu::CommandApduCase::Case4Extended => {
                CommandApduCase::CommandApduCase4Extended as i32
            }
        },
        domain: match descriptor.domain {
            jcim_core::iso7816::CommandDomain::Iso7816 => CommandDomain::Iso7816 as i32,
            jcim_core::iso7816::CommandDomain::GlobalPlatform => {
                CommandDomain::GlobalPlatform as i32
            }
            jcim_core::iso7816::CommandDomain::Opaque => CommandDomain::Opaque as i32,
        },
        kind: CommandKind::GetResponse as i32,
        logical_channel: u32::from(descriptor.logical_channel),
    }
}

async fn connect_channel(socket_path: &Path) -> Result<Channel, tonic::transport::Error> {
    let socket_path = socket_path.to_path_buf();
    Endpoint::try_from("http://[::]:50051")
        .expect("endpoint")
        .connect_with_connector(service_fn(move |_| {
            let socket_path = socket_path.clone();
            async move { UnixStream::connect(socket_path).await.map(TokioIo::new) }
        }))
        .await
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcimd-rpc-validation-{label}-{unique:x}"))
}
