use super::support::{
    assert_kind, parse_json, path_arg, run_cli, run_cli_failure, socket_support, temp_root,
};

#[test]
fn system_service_status_json_is_versioned_even_when_service_is_not_running() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    let root = temp_root("json-service-status");
    let output = run_cli(&root, &["--json", "system", "service", "status"]);
    let json = parse_json("service status", &output.stdout);

    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], "system.service_status");
    assert_eq!(json["running"], false);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_new_json_includes_version_and_kind_markers() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "project_new_json_includes_version_and_kind_markers",
    ) {
        return;
    }

    let root = temp_root("json-project-new");
    let project_dir = root.join("demo");
    let output = run_cli(
        &root,
        &[
            "--json",
            "project",
            "new",
            "demo",
            "--directory",
            &path_arg(&project_dir),
        ],
    );
    let json = parse_json("project new", &output.stdout);
    let expected_project_path = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.clone());

    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], "project.summary");
    assert_eq!(json["name"], "demo");
    assert_eq!(
        json["project_path"],
        expected_project_path.display().to_string()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn simulation_status_json_uses_the_list_kind() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported("simulation_status_json_uses_the_list_kind") {
        return;
    }

    let root = temp_root("json-sim-status");
    let output = run_cli(&root, &["--json", "sim", "status"]);
    let json = parse_json("sim status", &output.stdout);

    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], "simulation.list");
    assert!(json["simulations"].is_array());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn json_failures_go_to_stderr_with_the_error_envelope() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "json_failures_go_to_stderr_with_the_error_envelope",
    ) {
        return;
    }

    let root = temp_root("json-error");
    let missing_project = root.join("missing-project");
    let output = run_cli_failure(
        &root,
        &["--json", "build", "--project", &path_arg(&missing_project)],
    );
    let json = parse_json("json error", &output.stderr);

    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    assert_eq!(json["schema_version"], "jcim-cli.v2");
    assert_eq!(json["kind"], "error");
    assert!(!json["message"].as_str().expect("error message").is_empty());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn json_success_and_error_flows_remain_stable_across_repeated_daemon_bootstrap() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    if !socket_support::unix_domain_sockets_supported(
        "json_success_and_error_flows_remain_stable_across_repeated_daemon_bootstrap",
    ) {
        return;
    }

    let root = temp_root("jb");
    let missing_project = root.join("missing-project");

    let initial_status = parse_json(
        "initial sim status",
        &run_cli(&root, &["--json", "sim", "status"]).stdout,
    );
    assert_kind(&initial_status, "simulation.list");

    let failed_build = parse_json(
        "failed build",
        &run_cli_failure(
            &root,
            &["--json", "build", "--project", &path_arg(&missing_project)],
        )
        .stderr,
    );
    assert_kind(&failed_build, "error");

    let recovered_status = parse_json(
        "recovered sim status",
        &run_cli(&root, &["--json", "sim", "status"]).stdout,
    );
    assert_kind(&recovered_status, "simulation.list");

    let _ = std::fs::remove_dir_all(root);
}
