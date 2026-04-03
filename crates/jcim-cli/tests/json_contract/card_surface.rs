use super::support::{
    assert_kind, parse_json, path_arg, repo_root, run_cli_on_mock_service, socket_support,
    spawn_mock_service, temp_root,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn card_json_commands_cover_the_mock_reader_surface() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "card_json_commands_cover_the_mock_reader_surface",
    ) {
        return;
    }

    let root = temp_root("json-card-suite");
    let (shutdown_tx, server) = spawn_mock_service(&root).await;
    let project_dir = repo_root().join("examples/satochip/workdir");
    let project_arg = path_arg(&project_dir);
    let applet_aid = "5361746F4368697000";

    let readers = parse_json(
        "card readers",
        &run_cli_on_mock_service(&root, &["--json", "card", "readers"]).stdout,
    );
    assert_kind(&readers, "card.readers");
    assert_eq!(readers["readers"][0]["name"], "Mock Reader 0");

    let card_status = parse_json(
        "card status",
        &run_cli_on_mock_service(&root, &["--json", "card", "status"]).stdout,
    );
    assert_kind(&card_status, "card.status");
    assert_eq!(card_status["reader_name"], "Mock Reader 0");

    let install = parse_json(
        "card install",
        &run_cli_on_mock_service(
            &root,
            &["--json", "card", "install", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&install, "card.install");
    let package_aid = install["package_aid"]
        .as_str()
        .expect("package aid")
        .to_string();

    let packages = parse_json(
        "card packages",
        &run_cli_on_mock_service(&root, &["--json", "card", "packages"]).stdout,
    );
    assert_kind(&packages, "card.packages");

    let applets = parse_json(
        "card applets",
        &run_cli_on_mock_service(&root, &["--json", "card", "applets"]).stdout,
    );
    assert_kind(&applets, "card.applets");

    let card_apdu = parse_json(
        "card apdu",
        &run_cli_on_mock_service(
            &root,
            &["--json", "card", "apdu", "00A40400095361746F4368697000"],
        )
        .stdout,
    );
    assert_kind(&card_apdu, "apdu.response");
    assert_eq!(card_apdu["status_word"], "9000");

    let card_reset = parse_json(
        "card reset",
        &run_cli_on_mock_service(&root, &["--json", "card", "reset"]).stdout,
    );
    assert_kind(&card_reset, "card.reset");

    let card_iso_status = parse_json(
        "card iso status",
        &run_cli_on_mock_service(&root, &["--json", "card", "iso", "status"]).stdout,
    );
    assert_kind(&card_iso_status, "session.iso");

    let card_iso_select = parse_json(
        "card iso select",
        &run_cli_on_mock_service(
            &root,
            &["--json", "card", "iso", "select", "--aid", applet_aid],
        )
        .stdout,
    );
    assert_kind(&card_iso_select, "apdu.response");
    assert_eq!(card_iso_select["status_word"], "9000");

    let card_channel_open = parse_json(
        "card iso channel-open",
        &run_cli_on_mock_service(&root, &["--json", "card", "iso", "channel-open"]).stdout,
    );
    assert_kind(&card_channel_open, "channel.summary");
    let opened_channel = card_channel_open["channel_number"]
        .as_u64()
        .expect("opened card channel")
        .to_string();

    let card_channel_close = parse_json(
        "card iso channel-close",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "iso",
                "channel-close",
                "--channel",
                &opened_channel,
            ],
        )
        .stdout,
    );
    assert_kind(&card_channel_close, "channel.summary");

    let card_secure_open = parse_json(
        "card iso secure-open",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "iso",
                "secure-open",
                "--protocol",
                "scp03",
                "--security-level",
                "3",
                "--session-id",
                "card-session",
            ],
        )
        .stdout,
    );
    assert_kind(&card_secure_open, "secure_messaging.summary");

    let card_secure_advance = parse_json(
        "card iso secure-advance",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "iso",
                "secure-advance",
                "--increment",
                "2",
            ],
        )
        .stdout,
    );
    assert_kind(&card_secure_advance, "secure_messaging.summary");

    let card_secure_close = parse_json(
        "card iso secure-close",
        &run_cli_on_mock_service(&root, &["--json", "card", "iso", "secure-close"]).stdout,
    );
    assert_kind(&card_secure_close, "secure_messaging.summary");

    let card_gp_select = parse_json(
        "card gp select-isd",
        &run_cli_on_mock_service(&root, &["--json", "card", "gp", "select-isd"]).stdout,
    );
    assert_kind(&card_gp_select, "apdu.response");
    assert_eq!(card_gp_select["status_word"], "9000");

    let card_gp_status_apps = parse_json(
        "card gp get-status applications",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "get-status",
                "--kind",
                "applications",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_status_apps, "gp.status");

    let card_gp_status_isd = parse_json(
        "card gp get-status isd",
        &run_cli_on_mock_service(
            &root,
            &["--json", "card", "gp", "get-status", "--kind", "isd"],
        )
        .stdout,
    );
    assert_kind(&card_gp_status_isd, "gp.status");
    let security_domain_aid = card_gp_status_isd["entries"]
        .as_array()
        .and_then(|entries| entries.first())
        .and_then(|entry| entry["aid"].as_str())
        .expect("card security domain aid")
        .to_string();

    let card_gp_set_locked = parse_json(
        "card gp set-card-status locked",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-card-status",
                "--state",
                "card-locked",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_set_locked, "apdu.response");

    let card_gp_set_secured = parse_json(
        "card gp set-card-status secured",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-card-status",
                "--state",
                "secured",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_set_secured, "apdu.response");

    let card_gp_lock_app = parse_json(
        "card gp set-application-status lock",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-application-status",
                "--aid",
                applet_aid,
                "--transition",
                "lock",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_lock_app, "apdu.response");

    let card_gp_unlock_app = parse_json(
        "card gp set-application-status unlock",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-application-status",
                "--aid",
                applet_aid,
                "--transition",
                "unlock",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_unlock_app, "apdu.response");

    let card_gp_lock_sd = parse_json(
        "card gp set-security-domain-status",
        &run_cli_on_mock_service(
            &root,
            &[
                "--json",
                "card",
                "gp",
                "set-security-domain-status",
                "--aid",
                &security_domain_aid,
                "--transition",
                "lock",
            ],
        )
        .stdout,
    );
    assert_kind(&card_gp_lock_sd, "apdu.response");

    let delete = parse_json(
        "card delete",
        &run_cli_on_mock_service(&root, &["--json", "card", "delete", &package_aid]).stdout,
    );
    assert_kind(&delete, "card.delete");
    assert_eq!(delete["deleted"], true);

    let _ = shutdown_tx.send(());
    server.await.expect("server task").expect("server result");
    let _ = std::fs::remove_dir_all(root);
}
