//! Integration coverage for simulator-oriented gRPC flows.

#![forbid(unsafe_code)]

#[path = "../../../tests/support/socket.rs"]
mod socket_support;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use jcim_api::v0_3::build_service_client::BuildServiceClient;
use jcim_api::v0_3::simulator_service_client::SimulatorServiceClient;
use jcim_api::v0_3::{
    BuildProjectRequest, CommandApduCase, CommandApduFrame, CommandDomain, CommandKind,
    ProjectSelector, SimulationSelector, StartSimulationRequest, TransmitApduRequest,
};
use jcim_app::JcimApp;
use jcim_config::project::ManagedPaths;
use jcim_core::apdu::CommandApdu;
use jcim_core::iso7816;

#[tokio::test]
async fn service_builds_the_source_backed_satochip_example() {
    if !socket_support::unix_domain_sockets_supported(
        "service_builds_the_source_backed_satochip_example",
    ) {
        return;
    }

    let root = temp_root("service-build");
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
    let response = BuildServiceClient::new(channel)
        .build_project(BuildProjectRequest {
            project: Some(ProjectSelector {
                project_path: satochip_project_root().display().to_string(),
                project_id: String::new(),
            }),
        })
        .await
        .expect("build")
        .into_inner();

    assert_eq!(response.artifacts.len(), 1);
    assert_eq!(response.artifacts[0].kind, "cap");
    assert!(Path::new(&response.artifacts[0].path).exists());

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn project_backed_simulation_starts_and_exchanges_apdus() {
    if !socket_support::unix_domain_sockets_supported(
        "project_backed_simulation_starts_and_exchanges_apdus",
    ) {
        return;
    }

    let root = temp_root("project-sim");
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
    let simulation = SimulatorServiceClient::new(channel.clone())
        .start_simulation(StartSimulationRequest {
            project: Some(ProjectSelector {
                project_path: satochip_project_root().display().to_string(),
                project_id: String::new(),
            }),
        })
        .await
        .expect("start simulation")
        .into_inner()
        .simulation
        .expect("simulation");

    let exchange = SimulatorServiceClient::new(channel.clone())
        .transmit_apdu(TransmitApduRequest {
            simulation: Some(SimulationSelector {
                simulation_id: simulation.simulation_id.clone(),
            }),
            command: Some(select_satochip_command()),
        })
        .await
        .expect("transmit select")
        .into_inner();
    assert_eq!(exchange.response.expect("response").sw, 0x9000);
    assert!(exchange.session_state.is_some());

    let stopped = SimulatorServiceClient::new(channel)
        .stop_simulation(SimulationSelector {
            simulation_id: simulation.simulation_id,
        })
        .await
        .expect("stop simulation")
        .into_inner()
        .simulation
        .expect("stopped simulation");
    assert_eq!(stopped.status(), jcim_api::v0_3::SimulationStatus::Stopped);

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn missing_project_selector_fails_closed() {
    if !socket_support::unix_domain_sockets_supported("missing_project_selector_fails_closed") {
        return;
    }

    let root = temp_root("missing-project");
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
    let error = SimulatorServiceClient::new(channel)
        .start_simulation(StartSimulationRequest { project: None })
        .await
        .expect_err("missing project selector should fail");

    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    assert!(error.message().contains("project selector"));

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(root);
}

fn select_satochip_command() -> CommandApduFrame {
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
        kind: match descriptor.kind {
            jcim_core::iso7816::CommandKind::Select => CommandKind::Select as i32,
            jcim_core::iso7816::CommandKind::ManageChannel => CommandKind::ManageChannel as i32,
            jcim_core::iso7816::CommandKind::GetResponse => CommandKind::GetResponse as i32,
            jcim_core::iso7816::CommandKind::ReadBinary => CommandKind::ReadBinary as i32,
            jcim_core::iso7816::CommandKind::WriteBinary => CommandKind::WriteBinary as i32,
            jcim_core::iso7816::CommandKind::UpdateBinary => CommandKind::UpdateBinary as i32,
            jcim_core::iso7816::CommandKind::EraseBinary => CommandKind::EraseBinary as i32,
            jcim_core::iso7816::CommandKind::ReadRecord => CommandKind::ReadRecord as i32,
            jcim_core::iso7816::CommandKind::UpdateRecord => CommandKind::UpdateRecord as i32,
            jcim_core::iso7816::CommandKind::AppendRecord => CommandKind::AppendRecord as i32,
            jcim_core::iso7816::CommandKind::SearchRecord => CommandKind::SearchRecord as i32,
            jcim_core::iso7816::CommandKind::GetData => CommandKind::GetData as i32,
            jcim_core::iso7816::CommandKind::PutData => CommandKind::PutData as i32,
            jcim_core::iso7816::CommandKind::Verify => CommandKind::Verify as i32,
            jcim_core::iso7816::CommandKind::ChangeReferenceData => {
                CommandKind::ChangeReferenceData as i32
            }
            jcim_core::iso7816::CommandKind::ResetRetryCounter => {
                CommandKind::ResetRetryCounter as i32
            }
            jcim_core::iso7816::CommandKind::GetChallenge => CommandKind::GetChallenge as i32,
            jcim_core::iso7816::CommandKind::InternalAuthenticate => {
                CommandKind::InternalAuthenticate as i32
            }
            jcim_core::iso7816::CommandKind::ExternalAuthenticate => {
                CommandKind::ExternalAuthenticate as i32
            }
            jcim_core::iso7816::CommandKind::Envelope => CommandKind::Envelope as i32,
            jcim_core::iso7816::CommandKind::GpGetStatus => CommandKind::GpGetStatus as i32,
            jcim_core::iso7816::CommandKind::GpSetStatus => CommandKind::GpSetStatus as i32,
            jcim_core::iso7816::CommandKind::GpInitializeUpdate => {
                CommandKind::GpInitializeUpdate as i32
            }
            jcim_core::iso7816::CommandKind::GpExternalAuthenticate => {
                CommandKind::GpExternalAuthenticate as i32
            }
            jcim_core::iso7816::CommandKind::Opaque => CommandKind::Opaque as i32,
        },
        logical_channel: u32::from(descriptor.logical_channel),
    }
}

fn satochip_project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/satochip/workdir")
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
    PathBuf::from("/tmp").join(format!("jd-{label}-{unique}"))
}
