//! Descriptor-backed compatibility checks for the maintained JCIM gRPC contract.

#![forbid(unsafe_code)]

use prost::Message;
use prost_types::{DescriptorProto, FileDescriptorSet};

#[test]
fn jcim_v0_3_descriptor_preserves_the_maintained_package_and_services() {
    let descriptor = FileDescriptorSet::decode(jcim_api::JCIM_V0_3_DESCRIPTOR_SET)
        .expect("decode file descriptor set");
    let file = descriptor
        .file
        .iter()
        .find(|file| file.package.as_deref() == Some("jcim.v0_3"))
        .expect("jcim.v0_3 descriptor file");

    let services = file
        .service
        .iter()
        .map(|service| service.name.as_deref().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        services,
        vec![
            "WorkspaceService",
            "ProjectService",
            "BuildService",
            "SimulatorService",
            "CardService",
            "SystemService",
        ]
    );
}

#[test]
fn jcim_v0_3_descriptor_preserves_selected_stable_field_numbers() {
    let descriptor = FileDescriptorSet::decode(jcim_api::JCIM_V0_3_DESCRIPTOR_SET)
        .expect("decode file descriptor set");
    let file = descriptor
        .file
        .iter()
        .find(|file| file.package.as_deref() == Some("jcim.v0_3"))
        .expect("jcim.v0_3 descriptor file");

    assert_field_numbers(
        message(file, "ProjectSelector"),
        &[("project_path", 1), ("project_id", 2)],
    );
    assert_field_numbers(message(file, "SimulationSelector"), &[("simulation_id", 1)]);
    assert_field_numbers(message(file, "CardSelector"), &[("reader_name", 1)]);
    assert_field_numbers(
        message(file, "ProjectInfo"),
        &[
            ("project_id", 1),
            ("name", 2),
            ("project_path", 3),
            ("profile", 4),
            ("build_kind", 5),
            ("package_name", 6),
            ("package_aid", 7),
            ("applets", 8),
        ],
    );
    assert_field_numbers(
        message(file, "SimulationInfo"),
        &[
            ("simulation_id", 1),
            ("project_id", 2),
            ("project_path", 3),
            ("status", 4),
            ("reader_name", 5),
            ("health", 6),
        ],
    );
    assert_field_numbers(
        message(file, "GetServiceStatusResponse"),
        &[
            ("socket_path", 1),
            ("running", 2),
            ("known_project_count", 3),
            ("active_simulation_count", 4),
            ("service_binary_path", 5),
            ("service_binary_fingerprint", 6),
        ],
    );
}

fn message<'a>(file: &'a prost_types::FileDescriptorProto, name: &str) -> &'a DescriptorProto {
    file.message_type
        .iter()
        .find(|message| message.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("missing descriptor message `{name}`"))
}

fn assert_field_numbers(message: &DescriptorProto, expected: &[(&str, i32)]) {
    let actual = message
        .field
        .iter()
        .map(|field| {
            (
                field.name.as_deref().unwrap_or_default().to_string(),
                field.number.unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    for (name, number) in expected {
        assert!(
            actual
                .iter()
                .any(|(field_name, field_number)| field_name == name && field_number == number),
            "message `{}` is missing expected field `{name} = {number}`; actual fields: {actual:?}",
            message.name.as_deref().unwrap_or("<unknown>")
        );
    }
}
