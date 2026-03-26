//! Hardware-gated SDK coverage for real-card workflows.

#![forbid(unsafe_code)]

use jcim_core::apdu::CommandApdu;
use jcim_core::{aid::Aid, globalplatform};
use jcim_sdk::{
    CardConnectionKind, CardConnectionTarget, CardInstallSource, JcimClient, ProjectRef, ReaderRef,
};

#[tokio::test]
async fn hardware_card_lifecycle_runs_when_enabled() {
    if std::env::var("JCIM_HARDWARE_TESTS").ok().as_deref() != Some("1") {
        return;
    }

    let reader = std::env::var("JCIM_TEST_CARD_READER")
        .ok()
        .map(ReaderRef::named)
        .unwrap_or(ReaderRef::Default);

    let client = JcimClient::connect_or_start().await.expect("connect");
    let project = ProjectRef::from_path("examples/satochip/workdir");
    let _build = client.build_project(&project).await.expect("build");
    let install = client
        .install_cap_on(CardInstallSource::Project(project), reader.clone())
        .await
        .expect("install");
    assert!(!install.package_aid.is_empty());

    let packages = client
        .list_packages_on(reader.clone())
        .await
        .expect("packages");
    assert!(
        packages
            .packages
            .iter()
            .any(|package| package.aid == install.package_aid)
    );

    let applets = client
        .list_applets_on(reader.clone())
        .await
        .expect("applets");
    assert!(!applets.applets.is_empty());

    let connection = client
        .open_card_connection(CardConnectionTarget::Reader(reader.clone()))
        .await
        .expect("open card connection");
    assert_eq!(connection.kind(), CardConnectionKind::Reader);

    let select =
        CommandApdu::parse(&hex::decode("00A40400095361746F4368697000").expect("decode select"))
            .expect("parse select");
    let response = connection.transmit(&select).await.expect("select applet");
    assert_eq!(response.sw, 0x9000);

    let _session = connection.session_state().await.expect("session state");
    let _atr = connection.reset_summary().await.expect("reset");
    connection.close().await.expect("close connection");
}

#[tokio::test]
async fn hardware_card_gp_secure_channel_runs_when_enabled() {
    if std::env::var("JCIM_HARDWARE_TESTS").ok().as_deref() != Some("1") {
        return;
    }
    if std::env::var("JCIM_HARDWARE_GP_TESTS").ok().as_deref() != Some("1") {
        return;
    }

    let keyset_name = hardware_gp_keyset_name();
    let reader = std::env::var("JCIM_TEST_CARD_READER")
        .ok()
        .map(ReaderRef::named)
        .unwrap_or(ReaderRef::Default);

    let client = JcimClient::connect_or_start().await.expect("connect");
    let summary = client
        .open_gp_secure_channel_on_card_with_reader(Some(&keyset_name), Some(0x03), reader.clone())
        .await
        .expect("open gp secure channel");
    assert_eq!(
        summary.selected_aid,
        Aid::from_slice(&globalplatform::ISSUER_SECURITY_DOMAIN_AID).expect("isd aid"),
    );
    assert!(summary.session_state.secure_messaging.active);

    client
        .gp_get_status_on_card_with_reader(
            globalplatform::RegistryKind::Applications,
            globalplatform::GetStatusOccurrence::FirstOrAll,
            reader.clone(),
        )
        .await
        .expect("gp get status over authenticated session");

    let closed = client
        .close_gp_secure_channel_on_card_with_reader(reader)
        .await
        .expect("close gp secure channel");
    assert!(!closed.session_state.secure_messaging.active);
}

fn hardware_gp_keyset_name() -> String {
    let name = std::env::var("JCIM_HARDWARE_GP_KEYSET")
        .ok()
        .or_else(|| std::env::var("JCIM_GP_DEFAULT_KEYSET").ok())
        .expect("GP hardware tests require JCIM_HARDWARE_GP_KEYSET or JCIM_GP_DEFAULT_KEYSET");
    let prefix = format!("JCIM_GP_{}", gp_keyset_env_name(&name));
    for suffix in ["MODE", "ENC", "MAC", "DEK"] {
        let variable = format!("{prefix}_{suffix}");
        assert!(
            std::env::var_os(&variable).is_some(),
            "GP hardware tests require {variable}",
        );
    }
    name
}

fn gp_keyset_env_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}
