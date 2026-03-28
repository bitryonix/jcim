use jcim_config::project::UserConfig;
use jcim_core::apdu::ResponseApdu;
use jcim_core::{globalplatform, iso7816};

use super::{MockPhysicalCardAdapter, PhysicalCardAdapter, ResolvedGpKeyset};

#[tokio::test]
async fn mock_card_tracks_get_response_and_logical_channels() {
    let adapter = MockPhysicalCardAdapter::new();
    let user = UserConfig::default();

    let select = iso7816::select_file(0x0101);
    let select_response = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(select.to_bytes()))
        .await
        .expect("select response");
    assert_eq!(
        ResponseApdu::parse(&hex::decode(select_response).expect("hex"))
            .expect("response")
            .sw,
        0x9000
    );

    let read = iso7816::read_binary(0, 4);
    let read_response = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(read.to_bytes()))
        .await
        .expect("read response");
    let read_response =
        ResponseApdu::parse(&hex::decode(read_response).expect("hex")).expect("response");
    assert_eq!(read_response.data, b"JCIM");
    assert_eq!(read_response.sw, 0x6108);

    let get_response = iso7816::get_response(8);
    let get_response = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(get_response.to_bytes()))
        .await
        .expect("get response");
    let get_response =
        ResponseApdu::parse(&hex::decode(get_response).expect("hex")).expect("response");
    assert_eq!(get_response.data, b" mock EF");
    assert_eq!(get_response.sw, 0x9000);

    let open_channel = iso7816::manage_channel_open();
    let open_response = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(open_channel.to_bytes()))
        .await
        .expect("open channel");
    let open_response =
        ResponseApdu::parse(&hex::decode(open_response).expect("hex")).expect("response");
    assert_eq!(open_response.data, vec![1]);

    let status = adapter.card_status(&user, None).await.expect("status");
    assert!(
        status
            .session_state
            .open_channels
            .iter()
            .any(|entry| entry.channel_number == 1)
    );

    let close_channel = iso7816::manage_channel_close(1);
    let close_response = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(close_channel.to_bytes()))
        .await
        .expect("close channel");
    let close_response =
        ResponseApdu::parse(&hex::decode(close_response).expect("hex")).expect("response");
    assert_eq!(close_response.sw, 0x9000);
}

#[tokio::test]
async fn mock_card_enforces_retry_counters_and_blocking() {
    let adapter = MockPhysicalCardAdapter::new();
    let user = UserConfig::default();

    let wrong = iso7816::verify(0x80, b"0000");
    let first = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(wrong.to_bytes()))
        .await
        .expect("first verify");
    let first = ResponseApdu::parse(&hex::decode(first).expect("hex")).expect("response");
    assert_eq!(first.sw, 0x63C2);

    let second = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(wrong.to_bytes()))
        .await
        .expect("second verify");
    let second = ResponseApdu::parse(&hex::decode(second).expect("hex")).expect("response");
    assert_eq!(second.sw, 0x63C1);

    let third = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(wrong.to_bytes()))
        .await
        .expect("third verify");
    let third = ResponseApdu::parse(&hex::decode(third).expect("hex")).expect("response");
    assert_eq!(third.sw, iso7816::StatusWord::AUTH_METHOD_BLOCKED.as_u16());
}

#[tokio::test]
async fn mock_gp_get_status_supports_pagination() {
    let adapter = MockPhysicalCardAdapter::new();
    let user = UserConfig::default();
    {
        let mut state = adapter.state.lock().expect("lock");
        for suffix in 0u8..12 {
            state.applets.push(super::CardAppletSummary {
                aid: format!("A0000001510000{:02X}", suffix),
                description: format!("Applet {suffix}"),
            });
        }
    }

    let first = globalplatform::get_status(
        globalplatform::RegistryKind::Applications,
        globalplatform::GetStatusOccurrence::FirstOrAll,
    );
    let first = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(first.to_bytes()))
        .await
        .expect("first page");
    let first = ResponseApdu::parse(&hex::decode(first).expect("hex")).expect("response");
    assert_eq!(first.sw, iso7816::StatusWord::MORE_DATA_AVAILABLE.as_u16());

    let next = globalplatform::get_status(
        globalplatform::RegistryKind::Applications,
        globalplatform::GetStatusOccurrence::Next,
    );
    let next = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(next.to_bytes()))
        .await
        .expect("next page");
    let next = ResponseApdu::parse(&hex::decode(next).expect("hex")).expect("response");
    assert!(matches!(next.sw, 0x9000 | 0x6310));
}

#[tokio::test]
async fn mock_card_supports_helper_style_gp_auth_flow() {
    let adapter = MockPhysicalCardAdapter::new();
    let user = UserConfig::default();
    let keyset = ResolvedGpKeyset {
        name: "mock".to_string(),
        mode: globalplatform::ScpMode::Scp03,
        enc_hex: "404142434445464748494A4B4C4D4E4F".to_string(),
        mac_hex: "505152535455565758595A5B5C5D5E5F".to_string(),
        dek_hex: "606162636465666768696A6B6C6D6E6F".to_string(),
    };

    adapter
        .open_gp_secure_channel(&user, None, &keyset, 0x03)
        .await
        .expect("open gp secure channel");

    let summary = adapter.card_status(&user, None).await.expect("status");
    assert!(summary.session_state.secure_messaging.active);
    assert_eq!(
        summary.session_state.secure_messaging.protocol,
        Some(iso7816::SecureMessagingProtocol::Scp03)
    );

    let response = adapter
        .transmit_gp_secure_command(
            &user,
            None,
            &keyset,
            0x03,
            &globalplatform::get_status(
                globalplatform::RegistryKind::Applications,
                globalplatform::GetStatusOccurrence::FirstOrAll,
            ),
        )
        .await
        .expect("authenticated get status");
    assert!(matches!(response.sw, 0x9000 | 0x6310));

    let updated = adapter
        .card_status(&user, None)
        .await
        .expect("updated status");
    assert!(updated.session_state.secure_messaging.command_counter >= 1);
}

#[tokio::test]
async fn mock_card_install_delete_updates_inventory_and_status() {
    let adapter = MockPhysicalCardAdapter::new();
    let user = UserConfig::default();
    {
        let mut state = adapter.state.lock().expect("lock");
        state.packages.push(super::CardPackageSummary {
            aid: "A0000001510101".to_string(),
            description: "MockPkg 1.0".to_string(),
        });
        state.applets.push(super::CardAppletSummary {
            aid: "A000000151010101".to_string(),
            description: "MockApplet".to_string(),
        });
    }

    let packages = adapter.list_packages(&user, None).await.expect("packages");
    let applets = adapter.list_applets(&user, None).await.expect("applets");
    assert_eq!(packages.packages.len(), 1);
    assert_eq!(applets.applets.len(), 1);

    let lock = globalplatform::set_application_status(
        &jcim_core::aid::Aid::from_hex("A000000151010101").expect("aid"),
        globalplatform::LockTransition::Lock,
    );
    let lock = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(lock.to_bytes()))
        .await
        .expect("lock");
    let lock = ResponseApdu::parse(&hex::decode(lock).expect("hex")).expect("response");
    assert_eq!(lock.sw, 0x9000);

    let select =
        iso7816::select_by_name(&jcim_core::aid::Aid::from_hex("A000000151010101").expect("aid"));
    let select = adapter
        .transmit_apdu(&user, None, &hex::encode_upper(select.to_bytes()))
        .await
        .expect("select");
    let select = ResponseApdu::parse(&hex::decode(select).expect("hex")).expect("response");
    assert_eq!(
        select.sw,
        iso7816::StatusWord::WARNING_SELECTED_FILE_INVALIDATED.as_u16()
    );

    adapter
        .delete_item(&user, None, "A000000151010101")
        .await
        .expect("delete applet");
    let applets = adapter.list_applets(&user, None).await.expect("applets");
    assert!(applets.applets.is_empty());
}
