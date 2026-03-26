//! Regression tests for backend manifests and JSON reply parsing.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_config::config::RuntimeConfig;
use jcim_core::aid::Aid;
use jcim_core::iso7816::{IsoCapabilities, PowerState};
use jcim_core::model::{
    BackendCapabilities, BackendHealth, BackendKind, CardProfileId, JavaCardClassicVersion,
    MemoryLimits, MemoryStatus, ProtocolHandshake, ProtocolVersion,
};

use super::external::parse_runtime_snapshot;
use super::manifest::{
    BackendBundleManifest, default_protocol_version, resolve_classpath, validate_external_config,
};
use super::reply::{
    BackendOperation, BackendReply, BackendSessionStateWire, RuntimeSnapshotWire, ensure_reply_ok,
    ensure_reply_operation, parse_reply_line, validate_protocol,
};

fn temp_path(suffix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "jcim-backend-test-{suffix}-{unique}-{}",
        std::process::id()
    ))
}

#[test]
fn validate_external_config_requires_classes_and_metadata_inputs() {
    let manifest = BackendBundleManifest {
        protocol_version: default_protocol_version(),
        main_class: "example.Main".to_string(),
        classpath: vec!["libs/*".to_string()],
        args: Vec::new(),
        env: BTreeMap::new(),
        startup_timeout_ms: 1_000,
        supported_profiles: vec![CardProfileId::Classic221, CardProfileId::Classic222],
    };

    let config = RuntimeConfig {
        profile_id: CardProfileId::Classic221,
        cap_path: Some(PathBuf::from("/tmp/mock.cap")),
        classes_path: Some(PathBuf::from("/tmp/mock-classes")),
        simulator_metadata_path: Some(PathBuf::from("/tmp/mock-simulator.properties")),
        ..RuntimeConfig::default()
    };
    validate_external_config(&config, &manifest).expect("simulator config");

    let missing_classes = RuntimeConfig {
        profile_id: CardProfileId::Classic221,
        simulator_metadata_path: Some(PathBuf::from("/tmp/mock-simulator.properties")),
        ..RuntimeConfig::default()
    };
    assert!(validate_external_config(&missing_classes, &manifest).is_err());

    let missing_metadata = RuntimeConfig {
        profile_id: CardProfileId::Classic221,
        classes_path: Some(PathBuf::from("/tmp/mock-classes")),
        ..RuntimeConfig::default()
    };
    assert!(validate_external_config(&missing_metadata, &manifest).is_err());
}

#[test]
fn validate_external_config_rejects_unsupported_profile() {
    let manifest = BackendBundleManifest {
        protocol_version: default_protocol_version(),
        main_class: "example.Main".to_string(),
        classpath: vec!["libs/*".to_string()],
        args: Vec::new(),
        env: BTreeMap::new(),
        startup_timeout_ms: 1_000,
        supported_profiles: vec![CardProfileId::Classic221],
    };
    let config = RuntimeConfig {
        profile_id: CardProfileId::Classic305,
        classes_path: Some(PathBuf::from("/tmp/mock-classes")),
        simulator_metadata_path: Some(PathBuf::from("/tmp/mock-simulator.properties")),
        ..RuntimeConfig::default()
    };
    let error = validate_external_config(&config, &manifest).expect_err("profile error");
    assert!(error.to_string().contains("does not support profile"));
}

#[test]
fn resolve_classpath_expands_wildcards_and_existing_files_only() {
    let bundle_dir = temp_path("classpath");
    std::fs::create_dir_all(bundle_dir.join("libs")).expect("mkdir");
    std::fs::write(bundle_dir.join("classes"), b"x").expect("classes");
    std::fs::write(bundle_dir.join("libs/a.jar"), b"a").expect("jar a");
    std::fs::write(bundle_dir.join("libs/b.jar"), b"b").expect("jar b");

    let resolved = resolve_classpath(
        &bundle_dir,
        &[
            "classes".to_string(),
            "libs/*".to_string(),
            "missing".to_string(),
        ],
    )
    .expect("resolve");

    assert_eq!(resolved.len(), 3);
    assert!(resolved.iter().any(|entry| entry.ends_with("classes")));
    assert!(resolved.iter().any(|entry| entry.ends_with("libs/a.jar")));
    assert!(resolved.iter().any(|entry| entry.ends_with("libs/b.jar")));

    let _ = std::fs::remove_dir_all(bundle_dir);
}

#[test]
fn parse_reply_lines_and_operation_checks_enforce_json_contract() {
    let reply = parse_reply_line(
        r#"{"op":"health","ok":true,"health":{"backend_kind":"simulator","status":"ready","message":"healthy","protocol_version":"1.0"}}"#,
    )
    .expect("reply");
    ensure_reply_operation(&reply, BackendOperation::Health).expect("kind");
    ensure_reply_ok(&reply).expect("ok");
    let BackendReply::Health {
        health: Some(BackendHealth { message, .. }),
        ..
    } = reply
    else {
        panic!("health reply");
    };
    assert_eq!(message, "healthy");

    assert!(parse_reply_line("{").is_err());
    let mismatch = parse_reply_line(r#"{"op":"snapshot","ok":false,"error":"bad state"}"#)
        .expect("snapshot error");
    let error = ensure_reply_ok(&mismatch).expect_err("backend failure");
    assert!(error.to_string().contains("bad state"));
}

#[test]
fn parse_handshake_reply_extracts_capabilities() {
    let reply = parse_reply_line(
        r#"{"op":"handshake","ok":true,"handshake":{"protocol_version":"1.0","backend_kind":"simulator","reader_name":"Mock","backend_capabilities":{"protocol_version":"1.0","iso_capabilities":{"protocols":["T1"],"extended_length":true,"logical_channels":true,"max_logical_channels":4,"secure_messaging":true,"file_model_visibility":false,"raw_apdu":true},"supports_typed_apdu":true,"supports_raw_apdu":true,"supports_apdu":true,"supports_reset":true,"supports_power_control":true,"supports_get_session_state":true,"supports_manage_channel":true,"supports_secure_messaging":true,"supports_snapshot":true,"supports_install":false,"supports_delete":false,"supports_backend_health":true,"executes_real_methods":true,"wire_compatible_scp02":false,"wire_compatible_scp03":false,"supported_profiles":["classic221","classic222"]}}}"#,
    )
    .expect("handshake");
    ensure_reply_operation(&reply, BackendOperation::Handshake).expect("handshake op");
    let BackendReply::Handshake {
        handshake:
            Some(ProtocolHandshake {
                backend_kind,
                reader_name,
                backend_capabilities,
                ..
            }),
        ..
    } = reply
    else {
        panic!("handshake reply");
    };
    assert_eq!(backend_kind, BackendKind::Simulator);
    assert_eq!(reader_name, "Mock");
    assert_eq!(
        backend_capabilities.supported_profiles,
        vec![CardProfileId::Classic221, CardProfileId::Classic222]
    );
    assert!(backend_capabilities.supports_typed_apdu);
    assert!(backend_capabilities.supports_manage_channel);
    assert!(!backend_capabilities.supports_install);
    assert!(!backend_capabilities.wire_compatible_scp02);
}

#[test]
fn parse_runtime_snapshot_preserves_selected_aid_in_session_state() {
    let snapshot = parse_runtime_snapshot(RuntimeSnapshotWire {
        backend_kind: BackendKind::Simulator,
        profile_id: CardProfileId::Classic222,
        version: JavaCardClassicVersion::V2_2_2,
        backend_capabilities: BackendCapabilities {
            protocol_version: ProtocolVersion::current(),
            iso_capabilities: IsoCapabilities {
                protocols: vec![jcim_core::iso7816::TransportProtocol::T1],
                extended_length: true,
                logical_channels: true,
                max_logical_channels: 4,
                secure_messaging: true,
                file_model_visibility: false,
                raw_apdu: true,
            },
            supports_typed_apdu: true,
            supports_raw_apdu: true,
            supports_apdu: true,
            supports_reset: true,
            supports_power_control: true,
            supports_get_session_state: true,
            supports_manage_channel: true,
            supports_secure_messaging: true,
            supports_snapshot: true,
            supports_install: false,
            supports_delete: false,
            supports_backend_health: true,
            executes_real_methods: true,
            wire_compatible_scp02: false,
            wire_compatible_scp03: false,
            supported_profiles: vec![CardProfileId::Classic222],
        },
        atr_hex: "3B800100".to_string(),
        reader_name: "Mock".to_string(),
        iso_capabilities: IsoCapabilities {
            protocols: vec![jcim_core::iso7816::TransportProtocol::T1],
            extended_length: true,
            logical_channels: true,
            max_logical_channels: 4,
            secure_messaging: true,
            file_model_visibility: false,
            raw_apdu: true,
        },
        power_on: true,
        selected_aid: Some(Aid::from_hex("A000000151000001").expect("aid")),
        session_state: BackendSessionStateWire {
            power_state: PowerState::On,
            atr_hex: Some("3B800100".to_string()),
            active_protocol: Some(jcim_core::iso7816::ProtocolParameters {
                protocol: Some(jcim_core::iso7816::TransportProtocol::T1),
                ..jcim_core::iso7816::ProtocolParameters::default()
            }),
            selected_aid: Some(Aid::from_hex("A000000151000001").expect("aid")),
            current_file: None,
            open_channels: Vec::new(),
            secure_messaging: jcim_core::iso7816::SecureMessagingState::default(),
            verified_references: Vec::new(),
            retry_counters: Vec::new(),
            last_status: Some(0x9000),
        },
        memory_limits: MemoryLimits {
            persistent_bytes: 1,
            transient_reset_bytes: 1,
            transient_deselect_bytes: 1,
            apdu_buffer_bytes: 1,
            commit_buffer_bytes: 1,
            install_scratch_bytes: 1,
            stack_bytes: 1,
            page_bytes: 1,
            erase_block_bytes: 1,
            journal_bytes: 1,
            wear_limit: None,
        },
        memory_status: MemoryStatus::default(),
    })
    .expect("snapshot");

    assert_eq!(
        snapshot
            .selected_aid
            .as_ref()
            .expect("selected aid")
            .to_hex(),
        "A000000151000001"
    );
    assert_eq!(
        snapshot
            .session_state
            .selected_aid
            .as_ref()
            .expect("session selected aid")
            .to_hex(),
        "A000000151000001"
    );
    assert_eq!(snapshot.session_state.open_channels.len(), 1);
    assert_eq!(snapshot.session_state.open_channels[0].channel_number, 0);
    assert_eq!(
        snapshot.session_state.open_channels[0]
            .selected_aid
            .as_ref()
            .expect("basic channel selection")
            .to_hex(),
        "A000000151000001"
    );
    assert_eq!(
        snapshot.session_state.last_status.expect("status").as_u16(),
        0x9000
    );
}

#[test]
fn protocol_validation_rejects_major_version_mismatch() {
    validate_protocol(ProtocolVersion::current(), ProtocolVersion::new(1, 9)).expect("compatible");
    let error = validate_protocol(ProtocolVersion::current(), ProtocolVersion::new(2, 0))
        .expect_err("mismatch");
    assert!(error.to_string().contains("protocol mismatch"));
}
