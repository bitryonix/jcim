//! Integration coverage for the `jcim-core` public surface.

#![forbid(unsafe_code)]

use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::globalplatform;
use jcim_core::iso7816::{
    self, Atr, CommandDomain, CommandKind, IsoCapabilities, IsoSessionState, ProtocolParameters,
    SecureMessagingProtocol, apply_response_to_session, describe_command,
};
use jcim_core::model::{
    BackendCapabilities, BackendKind, CardProfile, CardProfileId, InstallRequest, MemoryStatus,
    PackageSummary, ProtocolVersion, RuntimeSnapshot, VirtualAppletMetadata,
};

#[test]
fn public_command_builders_round_trip_and_update_session_state() {
    let applet_aid = Aid::from_hex("A000000151000001").expect("aid");
    let select = iso7816::select_by_name(&applet_aid);
    let parsed_select = CommandApdu::parse(&select.to_bytes()).expect("parse select");
    assert_eq!(parsed_select, select);

    let descriptor = describe_command(&parsed_select);
    assert_eq!(descriptor.domain, CommandDomain::Iso7816);
    assert_eq!(descriptor.kind, CommandKind::Select);

    let atr = Atr::parse(&[0x3B, 0x80, 0x01, 0x00]).expect("atr");
    let mut state =
        IsoSessionState::reset(Some(atr.clone()), Some(ProtocolParameters::from_atr(&atr)));
    apply_response_to_session(&mut state, &select, &ResponseApdu::status(0x9000))
        .expect("apply select");
    assert_eq!(state.selected_aid, Some(applet_aid.clone()));

    let secure = iso7816::external_authenticate(0x01, 0x00, &[0xAA, 0xBB], None);
    apply_response_to_session(&mut state, &secure, &ResponseApdu::status(0x9000))
        .expect("apply secure messaging");
    assert!(state.secure_messaging.active);
    assert_eq!(
        state.secure_messaging.protocol,
        Some(SecureMessagingProtocol::Iso7816)
    );

    let gp_status = globalplatform::get_status(
        globalplatform::RegistryKind::Applications,
        globalplatform::GetStatusOccurrence::FirstOrAll,
    );
    let parsed_gp = CommandApdu::parse(&gp_status.to_bytes()).expect("parse gp status");
    let gp_descriptor = describe_command(&parsed_gp);
    assert_eq!(gp_descriptor.domain, CommandDomain::GlobalPlatform);
    assert_eq!(gp_descriptor.kind, CommandKind::GpGetStatus);
}

#[test]
fn public_model_types_capture_install_and_runtime_snapshot_state() {
    let package_aid = Aid::from_hex("A000000151000000").expect("package aid");
    let applet_aid = Aid::from_hex("A000000151000001").expect("applet aid");
    let profile = CardProfile::builtin(CardProfileId::Classic305);

    let install = InstallRequest::from_selectable_flag(vec![0xCA, 0xFE], true);
    assert!(install.make_selectable());

    let package = PackageSummary {
        package_aid: package_aid.clone(),
        package_name: "com.example.demo".to_string(),
        version: "1.0".to_string(),
        applet_count: 1,
    };
    let applet = VirtualAppletMetadata {
        package_aid: package_aid.clone(),
        applet_aid: applet_aid.clone(),
        instance_aid: applet_aid.clone(),
        selectable: true,
        package_name: package.package_name.clone(),
        applet_name: Some("DemoApplet".to_string()),
    };
    assert_eq!(applet.package_aid, package.package_aid);

    let snapshot = RuntimeSnapshot {
        backend_kind: BackendKind::Simulator,
        profile_id: profile.profile_id(),
        version: profile.version,
        backend_capabilities: BackendCapabilities {
            protocol_version: ProtocolVersion::current(),
            iso_capabilities: IsoCapabilities::default(),
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
            executes_real_methods: true,
            wire_compatible_scp02: false,
            wire_compatible_scp03: true,
            supported_profiles: vec![profile.profile_id()],
        },
        atr: profile.hardware.atr.clone(),
        reader_name: profile.reader_name.clone(),
        iso_capabilities: IsoCapabilities::default(),
        power_on: true,
        selected_aid: Some(applet_aid),
        session_state: IsoSessionState::reset(None, None),
        memory_limits: profile.hardware.memory.clone(),
        memory_status: MemoryStatus::default(),
    };

    assert_eq!(snapshot.profile_id, CardProfileId::Classic305);
    assert_eq!(snapshot.reader_name, profile.reader_name);
    assert!(snapshot.backend_capabilities.supports_snapshot);
    assert_eq!(snapshot.selected_aid, Some(applet.instance_aid));
}
