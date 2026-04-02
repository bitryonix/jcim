//! Message round-trip coverage for the `jcim-api` public contract.

#![forbid(unsafe_code)]

use prost::Message;

use jcim_api::v0_3::{
    BuildProjectRequest, ListSimulationsResponse, ProjectSelector, SimulationInfo, SimulationStatus,
};

#[test]
fn stable_request_messages_round_trip_through_prost() {
    let request = BuildProjectRequest {
        project: Some(ProjectSelector {
            project_path: "/tmp/demo".to_string(),
            project_id: "project-123".to_string(),
        }),
    };

    let encoded = request.encode_to_vec();
    let decoded = BuildProjectRequest::decode(encoded.as_slice()).expect("decode build request");
    let selector = decoded.project.expect("project selector");
    assert_eq!(selector.project_path, "/tmp/demo");
    assert_eq!(selector.project_id, "project-123");
}

#[test]
fn stable_response_messages_round_trip_enums_and_nested_payloads() {
    let response = ListSimulationsResponse {
        simulations: vec![SimulationInfo {
            simulation_id: "sim-1".to_string(),
            project_id: "project-123".to_string(),
            project_path: "/tmp/demo".to_string(),
            status: SimulationStatus::Running as i32,
            reader_name: "JCIM Reader".to_string(),
            health: "ready".to_string(),
            atr: None,
            active_protocol: None,
            iso_capabilities: None,
            session_state: None,
            package_count: 1,
            applet_count: 2,
            package_name: "Demo".to_string(),
            package_aid: "A000000151000000".to_string(),
            recent_events: vec!["simulation started".to_string()],
        }],
    };

    let encoded = response.encode_to_vec();
    let decoded =
        ListSimulationsResponse::decode(encoded.as_slice()).expect("decode simulation list");
    assert_eq!(decoded.simulations.len(), 1);
    assert_eq!(decoded.simulations[0].status(), SimulationStatus::Running);
    assert_eq!(decoded.simulations[0].package_count, 1);
    assert_eq!(decoded.simulations[0].applet_count, 2);
}
