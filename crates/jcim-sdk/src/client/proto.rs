/// APDU encoding and decoding helpers shared across SDK client adapters.
mod apdu;
/// GlobalPlatform summary decoding helpers shared across SDK client adapters.
mod gp;
/// ISO 7816 decoding helpers shared across SDK client adapters.
mod iso;
/// Protobuf selector encoding helpers for project and simulation lookups.
mod selectors;
/// Service-level response decoding helpers shared across SDK client adapters.
mod service;
/// Workspace, build, and simulation summary decoding helpers.
mod summaries;

pub(super) use apdu::{
    command_apdu_frame, response_apdu_from_proto, secure_messaging_protocol_fields,
};
pub(super) use gp::gp_secure_channel_from_proto;
pub(super) use iso::{
    atr_from_proto, iso_capabilities_from_proto, iso_session_state_from_proto,
    protocol_parameters_from_proto,
};
pub(super) use selectors::{project_selector, simulation_selector};
pub(super) use service::{
    reset_summary_from_card_proto, reset_summary_from_simulation_proto, service_status_summary,
    setup_summary,
};
pub(super) use summaries::{artifact_summary, project_summary, simulation_summary};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jcim_api::v0_3::file_selection_info::Selection as FileSelectionSelection;
    use jcim_api::v0_3::{
        AidInfo, ApduEncoding, AtrInfo, CommandApduCase, CommandDomain, CommandKind,
        FileSelectionInfo, GetServiceStatusResponse, IsoSessionStateInfo, LogicalChannelStateInfo,
        PowerState as PowerStateProto, ProjectSelector, ProtocolParametersInfo, ResponseApduFrame,
        RetryCounterInfo, SecureMessagingProtocol as SecureMessagingProtocolProto,
        SecureMessagingStateInfo, SimulationInfo, StatusWordClass, StatusWordInfo,
        TransportProtocol,
    };
    use jcim_core::aid::Aid;
    use jcim_core::apdu::ResponseApdu;
    use jcim_core::iso7816::{
        self, FileSelection, IsoSessionState, PowerState, SecureMessagingProtocol, StatusWord,
    };

    use super::iso::secure_messaging_protocol_from_proto;
    use super::*;

    #[test]
    fn selector_helpers_preserve_path_and_id_shapes() {
        let project = project_selector(&crate::types::ProjectRef::from_path(
            "examples/satochip/workdir",
        ));
        assert_eq!(
            project,
            ProjectSelector {
                project_path: PathBuf::from("examples/satochip/workdir")
                    .display()
                    .to_string(),
                project_id: String::new(),
            }
        );

        let simulation = simulation_selector("sim-42".to_string());
        assert_eq!(simulation.simulation_id, "sim-42");
    }

    #[test]
    fn simulation_summary_maps_unknown_status_to_unknown() {
        let summary = simulation_summary(SimulationInfo {
            simulation_id: "sim-1".to_string(),
            project_id: "proj-1".to_string(),
            project_path: "/tmp/project".to_string(),
            status: 999,
            reader_name: "Reader".to_string(),
            health: "healthy".to_string(),
            atr: None,
            active_protocol: None,
            iso_capabilities: None,
            session_state: None,
            package_count: 1,
            applet_count: 2,
            package_name: "pkg".to_string(),
            package_aid: "A0000001510001".to_string(),
            recent_events: vec!["info: started".to_string()],
        })
        .expect("summary");

        assert_eq!(summary.status, crate::types::SimulationStatus::Unknown);
        assert_eq!(
            summary.iso_capabilities,
            jcim_core::iso7816::IsoCapabilities::default()
        );
        assert_eq!(summary.session_state, IsoSessionState::default());
    }

    #[test]
    fn response_apdu_from_proto_accepts_raw_and_structured_forms() {
        let raw = response_apdu_from_proto(Some(ResponseApduFrame {
            raw: vec![0x01, 0x02, 0x90, 0x00],
            data: vec![0xFF],
            sw: 0x6A82,
            status: None,
        }))
        .expect("raw response");
        assert_eq!(raw, ResponseApdu::success(vec![0x01, 0x02]));

        let structured = response_apdu_from_proto(Some(ResponseApduFrame {
            raw: Vec::new(),
            data: vec![0xCA, 0xFE],
            sw: 0x9000,
            status: None,
        }))
        .expect("structured response");
        assert_eq!(
            structured,
            ResponseApdu {
                data: vec![0xCA, 0xFE],
                sw: 0x9000,
            }
        );
    }

    #[test]
    fn secure_messaging_protocol_helpers_preserve_custom_labels() {
        assert_eq!(
            secure_messaging_protocol_fields(Some(&SecureMessagingProtocol::Other(
                "scp-custom".to_string()
            ))),
            (
                SecureMessagingProtocolProto::Other as i32,
                "scp-custom".to_string()
            )
        );
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Other as i32,
                "scp-custom",
            ),
            Some(SecureMessagingProtocol::Other("scp-custom".to_string()))
        );
        assert_eq!(
            secure_messaging_protocol_from_proto(
                SecureMessagingProtocolProto::Unspecified as i32,
                "",
            ),
            None
        );
    }

    #[test]
    fn service_status_summary_preserves_paths_and_fingerprint() {
        let summary = service_status_summary(GetServiceStatusResponse {
            socket_path: "/tmp/jcim.sock".to_string(),
            running: true,
            known_project_count: 3,
            active_simulation_count: 2,
            service_binary_path: "/tmp/jcimd".to_string(),
            service_binary_fingerprint: "123:456:789".to_string(),
        })
        .expect("service summary");

        assert_eq!(summary.socket_path, PathBuf::from("/tmp/jcim.sock"));
        assert!(summary.running);
        assert_eq!(summary.known_project_count, 3);
        assert_eq!(summary.active_simulation_count, 2);
        assert_eq!(summary.service_binary_path, PathBuf::from("/tmp/jcimd"));
        assert_eq!(summary.service_binary_fingerprint, "123:456:789");
    }

    #[test]
    fn iso_session_state_from_proto_reconstructs_nested_state() {
        let atr_raw = vec![0x3B, 0x80, 0x01, 0x00];
        let state = iso_session_state_from_proto(Some(IsoSessionStateInfo {
            power_state: PowerStateProto::On as i32,
            atr: Some(AtrInfo {
                raw: atr_raw.clone(),
                hex: String::new(),
                convention: jcim_api::v0_3::TransmissionConvention::Direct as i32,
                interface_groups: Vec::new(),
                historical_bytes: Vec::new(),
                checksum_tck: None,
                protocols: vec![TransportProtocol::T1 as i32],
            }),
            active_protocol: Some(ProtocolParametersInfo {
                protocol: TransportProtocol::T1 as i32,
                fi: Some(9),
                di: Some(4),
                waiting_integer: Some(5),
                ifsc: Some(32),
            }),
            selected_aid: Some(AidInfo {
                raw: Aid::from_hex("A000000151000001")
                    .expect("aid")
                    .as_bytes()
                    .to_vec(),
                hex: "A000000151000001".to_string(),
            }),
            current_file: Some(FileSelectionInfo {
                selection: Some(FileSelectionSelection::FileId(0x3F00)),
            }),
            open_channels: vec![LogicalChannelStateInfo {
                channel_number: 1,
                selected_aid: Some(AidInfo {
                    raw: Aid::from_hex("A000000151000002")
                        .expect("aid")
                        .as_bytes()
                        .to_vec(),
                    hex: "A000000151000002".to_string(),
                }),
                current_file: Some(FileSelectionInfo {
                    selection: Some(FileSelectionSelection::ByName(vec![0xA0, 0x00])),
                }),
            }],
            secure_messaging: Some(SecureMessagingStateInfo {
                active: true,
                protocol: SecureMessagingProtocolProto::Other as i32,
                security_level: Some(0x13),
                session_id: "secure-1".to_string(),
                command_counter: 7,
                protocol_label: "scp-custom".to_string(),
            }),
            verified_references: vec![0x81, 0x82],
            retry_counters: vec![RetryCounterInfo {
                reference: 0x81,
                remaining: 3,
            }],
            last_status: Some(StatusWordInfo {
                value: 0x63C2,
                class: StatusWordClass::Warning as i32,
                label: "verify_failed_retries_remaining".to_string(),
                success: false,
                warning: true,
                remaining_response_bytes: None,
                retry_counter: Some(2),
                exact_length_hint: None,
            }),
        }))
        .expect("session state");

        assert_eq!(state.power_state, PowerState::On);
        assert_eq!(state.atr.as_ref().expect("atr").raw, atr_raw);
        assert_eq!(
            state.active_protocol.as_ref().expect("protocol").protocol,
            Some(jcim_core::iso7816::TransportProtocol::T1)
        );
        assert_eq!(state.current_file, Some(FileSelection::FileId(0x3F00)));
        assert_eq!(state.open_channels.len(), 1);
        assert_eq!(
            state.open_channels[0].current_file,
            Some(FileSelection::ByName(vec![0xA0, 0x00]))
        );
        assert_eq!(
            state.secure_messaging.protocol,
            Some(SecureMessagingProtocol::Other("scp-custom".to_string()))
        );
        assert_eq!(state.secure_messaging.command_counter, 7);
        assert_eq!(state.verified_references, vec![0x81, 0x82]);
        assert_eq!(state.retry_counters[0].remaining, 3);
        assert_eq!(state.last_status, Some(StatusWord::new(0x63C2)));
    }

    #[test]
    fn command_apdu_frame_preserves_descriptor_metadata() {
        let aid = Aid::from_hex("A000000151000001").expect("aid");
        let command = iso7816::select_by_name(&aid);
        let frame = command_apdu_frame(&command);

        assert_eq!(frame.encoding, ApduEncoding::Short as i32);
        assert_eq!(
            frame.apdu_case,
            CommandApduCase::CommandApduCase4Short as i32
        );
        assert_eq!(frame.domain, CommandDomain::Iso7816 as i32);
        assert_eq!(frame.kind, CommandKind::Select as i32);
        assert_eq!(frame.logical_channel, 0);
    }
}
