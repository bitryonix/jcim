//! Regression tests for the shared JCIM model layer.

use super::{
    BackendCapabilities, BackendHealth, BackendHealthStatus, BackendKind, CardProfile,
    CardProfileId, InstallDisposition, InstallRequest, JavaCardClassicVersion, PowerAction,
    ProtocolHandshake, ProtocolVersion, ScpMode,
};
use crate::iso7816::IsoCapabilities;

/// Ensure backend kinds preserve their string contract.
#[test]
fn backend_kind_names_and_parse_roundtrip() {
    assert_eq!(BackendKind::Simulator.to_string(), "simulator");
    assert_eq!(
        "simulator".parse::<BackendKind>().expect("kind"),
        BackendKind::Simulator
    );
}

/// Ensure Java Card versions report the expected names and CAP compatibility.
#[test]
fn java_card_versions_report_names_and_cap_support() {
    assert_eq!(JavaCardClassicVersion::V2_1.display_name(), "2.1");
    assert!(JavaCardClassicVersion::V2_1.supports_cap_minor(1));
    assert!(!JavaCardClassicVersion::V2_1.supports_cap_minor(2));
    assert_eq!(
        "3.0.5".parse::<JavaCardClassicVersion>().expect("version"),
        JavaCardClassicVersion::V3_0_5
    );
}

/// Ensure profile identifiers resolve versions and parse from their stable strings.
#[test]
fn profile_ids_resolve_versions_and_names() {
    assert_eq!(
        CardProfileId::Classic221.version(),
        JavaCardClassicVersion::V2_2_1
    );
    assert_eq!(CardProfileId::Classic305.display_name(), "3.0.5");
    assert_eq!(
        "classic304".parse::<CardProfileId>().expect("profile"),
        CardProfileId::Classic304
    );
}

/// Ensure maintained profiles preserve the expected hardware-family defaults.
#[test]
fn builtin_profiles_use_expected_hardware_families() {
    let classic21 = CardProfile::builtin(CardProfileId::Classic21);
    assert_eq!(classic21.version, JavaCardClassicVersion::V2_1);
    assert_eq!(classic21.hardware.memory.persistent_bytes, 180 * 1024);
    assert!(!classic21.hardware.supports_scp03);

    let classic305 = CardProfile::builtin(CardProfileId::Classic305);
    assert_eq!(classic305.hardware.memory.persistent_bytes, 512 * 1024);
    assert_eq!(classic305.hardware.max_apdu_size, 2048);
    assert!(classic305.supports_cap_minor(2));
}

/// Ensure typed power and install helpers preserve the compact boolean API semantics.
#[test]
fn typed_power_and_install_requests_preserve_boolean_flag_behavior() {
    assert_eq!(PowerAction::from(true), PowerAction::On);
    assert_eq!(PowerAction::from(false), PowerAction::Off);

    let install = InstallRequest::from_selectable_flag(vec![0xCA, 0xFE], true);
    assert!(install.make_selectable());
    assert_eq!(install.disposition, InstallDisposition::MakeSelectable);
}

/// Ensure SCP mode defaults stay stable for configuration loading.
#[test]
fn scp_mode_defaults_to_any() {
    assert_eq!(ScpMode::default(), ScpMode::Any);
}

/// Ensure protocol versions preserve display, parse, and compatibility behavior.
#[test]
fn protocol_versions_parse_display_and_negotiate() {
    let version = "1.4".parse::<ProtocolVersion>().expect("version");
    assert_eq!(version.to_string(), "1.4");
    assert!(ProtocolVersion::current().is_compatible_with(ProtocolVersion::new(1, 9)));
    assert!(!ProtocolVersion::current().is_compatible_with(ProtocolVersion::new(2, 0)));
}

/// Ensure the backend-facing capability structs remain easy to construct in tests and callers.
#[test]
fn backend_capability_structs_are_buildable() {
    let capabilities = BackendCapabilities {
        protocol_version: ProtocolVersion::current(),
        iso_capabilities: IsoCapabilities::default(),
        accepts_cap: true,
        supports_typed_apdu: true,
        supports_raw_apdu: true,
        supports_apdu: true,
        supports_reset: true,
        supports_power_control: true,
        supports_get_session_state: true,
        supports_manage_channel: true,
        supports_secure_messaging: true,
        supports_snapshot: true,
        supports_install: true,
        supports_delete: true,
        supports_backend_health: true,
        executes_real_methods: false,
        wire_compatible_scp02: false,
        wire_compatible_scp03: false,
        supported_profiles: vec![CardProfileId::Classic305],
    };
    let handshake = ProtocolHandshake {
        protocol_version: ProtocolVersion::current(),
        backend_kind: BackendKind::Simulator,
        reader_name: "JCIM".to_string(),
        backend_capabilities: capabilities.clone(),
    };
    let health = BackendHealth {
        backend_kind: BackendKind::Simulator,
        status: BackendHealthStatus::Ready,
        message: "ok".to_string(),
        protocol_version: ProtocolVersion::current(),
    };
    assert_eq!(handshake.backend_capabilities, capabilities);
    assert_eq!(health.status, BackendHealthStatus::Ready);
}
