use super::support::{assert_kind, parse_json, path_arg, run_cli, socket_support, temp_root};

#[test]
fn project_build_system_and_simulation_json_commands_cover_the_managed_surface() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "project_build_system_and_simulation_json_commands_cover_the_managed_surface",
    ) {
        return;
    }

    let root = temp_root("json-sim-suite");
    let project_dir = root.join("demo");
    let project_arg = path_arg(&project_dir);
    let applet_aid = "F00000000101";

    let project_new = parse_json(
        "project new",
        &run_cli(
            &root,
            &[
                "--json",
                "project",
                "new",
                "demo",
                "--directory",
                &project_arg,
            ],
        )
        .stdout,
    );
    assert_kind(&project_new, "project.summary");

    let project_show = parse_json(
        "project show",
        &run_cli(
            &root,
            &["--json", "project", "show", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&project_show, "project.details");

    let build = parse_json(
        "build",
        &run_cli(&root, &["--json", "build", "--project", &project_arg]).stdout,
    );
    assert_kind(&build, "build.summary");
    assert!(
        build["artifacts"]
            .as_array()
            .expect("build artifacts")
            .iter()
            .any(|artifact| artifact["kind"] == "cap")
    );

    let artifacts = parse_json(
        "build artifacts",
        &run_cli(
            &root,
            &["--json", "build", "artifacts", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&artifacts, "build.summary");

    let project_clean = parse_json(
        "project clean",
        &run_cli(
            &root,
            &["--json", "project", "clean", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&project_clean, "project.clean");

    let system_setup = parse_json(
        "system setup",
        &run_cli(&root, &["--json", "system", "setup"]).stdout,
    );
    assert_kind(&system_setup, "system.setup");

    let system_doctor = parse_json(
        "system doctor",
        &run_cli(&root, &["--json", "system", "doctor"]).stdout,
    );
    assert_kind(&system_doctor, "system.doctor");

    let service_status = parse_json(
        "system service status",
        &run_cli(&root, &["--json", "system", "service", "status"]).stdout,
    );
    assert_kind(&service_status, "system.service_status");
    assert_eq!(service_status["running"], true);

    let sim_start = parse_json(
        "sim start",
        &run_cli(
            &root,
            &["--json", "sim", "start", "--project", &project_arg],
        )
        .stdout,
    );
    assert_kind(&sim_start, "simulation.summary");

    let sim_status = parse_json(
        "sim status",
        &run_cli(&root, &["--json", "sim", "status"]).stdout,
    );
    assert_kind(&sim_status, "simulation.list");

    let sim_logs = parse_json(
        "sim logs",
        &run_cli(&root, &["--json", "sim", "logs"]).stdout,
    );
    assert_kind(&sim_logs, "simulation.events");

    let sim_apdu = parse_json(
        "sim apdu",
        &run_cli(
            &root,
            &["--json", "sim", "apdu", "00A4040006F0000000010100"],
        )
        .stdout,
    );
    assert_kind(&sim_apdu, "apdu.response");
    assert_eq!(sim_apdu["status_word"], "9000");

    let sim_reset = parse_json(
        "sim reset",
        &run_cli(&root, &["--json", "sim", "reset"]).stdout,
    );
    assert_kind(&sim_reset, "simulation.reset");

    let sim_iso_status = parse_json(
        "sim iso status",
        &run_cli(&root, &["--json", "sim", "iso", "status"]).stdout,
    );
    assert_kind(&sim_iso_status, "session.iso");

    let sim_iso_select = parse_json(
        "sim iso select",
        &run_cli(
            &root,
            &["--json", "sim", "iso", "select", "--aid", applet_aid],
        )
        .stdout,
    );
    assert_kind(&sim_iso_select, "apdu.response");
    assert_eq!(sim_iso_select["status_word"], "9000");

    let sim_channel_open = parse_json(
        "sim iso channel-open",
        &run_cli(&root, &["--json", "sim", "iso", "channel-open"]).stdout,
    );
    assert_kind(&sim_channel_open, "channel.summary");
    let opened_channel = sim_channel_open["channel_number"]
        .as_u64()
        .expect("opened channel number")
        .to_string();

    let sim_channel_close = parse_json(
        "sim iso channel-close",
        &run_cli(
            &root,
            &[
                "--json",
                "sim",
                "iso",
                "channel-close",
                "--channel",
                &opened_channel,
            ],
        )
        .stdout,
    );
    assert_kind(&sim_channel_close, "channel.summary");

    let sim_secure_open = parse_json(
        "sim iso secure-open",
        &run_cli(
            &root,
            &[
                "--json",
                "sim",
                "iso",
                "secure-open",
                "--protocol",
                "scp03",
                "--security-level",
                "3",
                "--session-id",
                "sim-session",
            ],
        )
        .stdout,
    );
    assert_kind(&sim_secure_open, "secure_messaging.summary");

    let sim_secure_advance = parse_json(
        "sim iso secure-advance",
        &run_cli(
            &root,
            &["--json", "sim", "iso", "secure-advance", "--increment", "2"],
        )
        .stdout,
    );
    assert_kind(&sim_secure_advance, "secure_messaging.summary");

    let sim_secure_close = parse_json(
        "sim iso secure-close",
        &run_cli(&root, &["--json", "sim", "iso", "secure-close"]).stdout,
    );
    assert_kind(&sim_secure_close, "secure_messaging.summary");

    let sim_stop = parse_json(
        "sim stop",
        &run_cli(&root, &["--json", "sim", "stop"]).stdout,
    );
    assert_kind(&sim_stop, "simulation.summary");

    let _ = std::fs::remove_dir_all(root);
}
