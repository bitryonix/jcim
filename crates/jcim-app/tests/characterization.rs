//! Characterization coverage for the transport-neutral JCIM application boundary.

#![forbid(unsafe_code)]

#[path = "../../../tests/support/socket.rs"]
mod socket_support;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_app::{JcimApp, MockPhysicalCardAdapter, ProjectSelectorInput, SimulationSelectorInput};
use jcim_config::project::{ManagedPaths, UserConfig};
use jcim_core::{globalplatform, iso7816};

fn app_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[test]
fn project_build_and_registry_behavior_is_characterized() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    let root = temp_root("project-build");
    let project_dir = root.join("demo");
    let managed_paths = ManagedPaths::for_root(root.clone());
    let app = JcimApp::load_with_paths_and_card_adapter(
        managed_paths.clone(),
        Arc::new(MockPhysicalCardAdapter::new()),
    )
    .expect("load app");

    let created = app
        .create_project("demo", &project_dir)
        .expect("create project");
    assert_eq!(created.project.name, "demo");
    assert!(project_dir.join("jcim.toml").exists());

    let selector = selector_for(&project_dir);
    let loaded = app.get_project(&selector).expect("get project");
    assert_eq!(loaded.project.project_id, created.project.project_id);
    assert!(loaded.manifest_toml.contains("[project]"));

    let listed = app.list_projects().expect("list projects");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].project_id, created.project.project_id);

    let registry_text =
        std::fs::read_to_string(&managed_paths.registry_path).expect("read registry");
    assert!(registry_text.contains(&created.project.project_id));
    assert!(registry_text.contains(&project_dir.display().to_string()));

    let (built_project, artifacts, _rebuilt) = app.build_project(&selector).expect("build project");
    assert_eq!(built_project.project_id, created.project.project_id);
    assert!(!artifacts.is_empty());
    assert!(artifacts.iter().any(|artifact| artifact.kind == "cap"));
    assert!(artifacts.iter().all(|artifact| artifact.path.exists()));

    let events = app.build_events(&selector).expect("build events");
    assert!(
        events
            .iter()
            .any(|event| event.message.contains("building project"))
    );
    assert!(
        events
            .iter()
            .any(|event| event.message.contains("build completed")
                || event.message.contains("build reused"))
    );

    let (_project, current_artifacts) = app.get_artifacts(&selector).expect("current artifacts");
    assert_eq!(current_artifacts, artifacts);

    let cleaned = app.clean_project(&selector).expect("clean project");
    assert_eq!(cleaned, created.project.project_path.join(".jcim"));
    assert!(!cleaned.exists());

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn simulation_lifecycle_behavior_is_characterized() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    let _guard = app_lock().lock().await;
    let root = temp_root("simulation");
    let project_dir = root.join("demo");
    let app = JcimApp::load_with_paths_and_card_adapter(
        ManagedPaths::for_root(root.clone()),
        Arc::new(MockPhysicalCardAdapter::new()),
    )
    .expect("load app");
    app.create_project("demo", &project_dir)
        .expect("create project");

    let selector = selector_for(&project_dir);
    let simulation = app
        .start_project_simulation(&selector)
        .await
        .expect("start simulation");
    assert_eq!(simulation.status.as_str(), "running");
    assert!(simulation.health.contains("ready") || simulation.health.contains("degraded"));

    let simulations = app.list_simulations().expect("list simulations");
    assert_eq!(simulations.len(), 1);
    assert_eq!(simulations[0].simulation_id, simulation.simulation_id);

    let simulation_selector = SimulationSelectorInput {
        simulation_id: simulation.simulation_id.clone(),
    };
    let loaded = app
        .get_simulation(&simulation_selector)
        .expect("get simulation");
    assert_eq!(loaded.simulation_id, simulation.simulation_id);
    let events = app
        .simulation_events(&simulation_selector)
        .expect("simulation events");
    assert!(
        events
            .iter()
            .any(|event| event.message.contains("simulation started"))
    );

    let applet_aid = jcim_core::aid::Aid::from_hex("F00000000101").expect("default applet aid");
    let exchange = app
        .transmit_command(&simulation_selector, &iso7816::select_by_name(&applet_aid))
        .await
        .expect("select applet");
    assert_eq!(exchange.response.sw, 0x9000);
    assert_eq!(
        app.simulation_session_state(&simulation_selector)
            .expect("session state")
            .selected_aid,
        Some(applet_aid)
    );

    let secure = app
        .open_simulation_secure_messaging(
            &simulation_selector,
            Some(jcim_core::iso7816::SecureMessagingProtocol::Scp03),
            Some(0x03),
            Some("sim-session".to_string()),
        )
        .await
        .expect("open secure messaging");
    assert!(secure.session_state.secure_messaging.active);

    let advanced = app
        .advance_simulation_secure_messaging(&simulation_selector, 2)
        .await
        .expect("advance secure messaging");
    assert_eq!(advanced.session_state.secure_messaging.command_counter, 2);

    let channel = app
        .manage_simulation_channel(&simulation_selector, true, None)
        .await
        .expect("open simulation channel");
    assert_eq!(channel.channel_number, Some(1));

    let closed = app
        .close_simulation_secure_messaging(&simulation_selector)
        .await
        .expect("close secure messaging");
    assert!(!closed.session_state.secure_messaging.active);

    let reset = app
        .reset_simulation_summary(&simulation_selector)
        .await
        .expect("reset simulation");
    assert!(reset.atr.is_some());

    let stopped = app
        .stop_simulation(&simulation_selector)
        .await
        .expect("stop simulation");
    assert_eq!(stopped.status.as_str(), "stopped");

    let restarted = app
        .start_project_simulation(&selector)
        .await
        .expect("restart simulation");
    assert_ne!(restarted.simulation_id, simulation.simulation_id);
    let restarted_selector = SimulationSelectorInput {
        simulation_id: restarted.simulation_id,
    };
    let restarted_stop = app
        .stop_simulation(&restarted_selector)
        .await
        .expect("stop restarted simulation");
    assert_eq!(restarted_stop.status.as_str(), "stopped");

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn failed_simulation_start_is_retained_and_recoverable() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    let _guard = app_lock().lock().await;
    let root = temp_root("simulation-failure");
    let project_dir = root.join("demo");
    let managed_paths = ManagedPaths::for_root(root.clone());
    managed_paths
        .prepare_layout()
        .expect("prepare managed layout");
    UserConfig {
        bundle_root: Some(root.join("missing-bundles")),
        ..UserConfig::default()
    }
    .save_to_path(&managed_paths.config_path)
    .expect("save user config");
    let app = JcimApp::load_with_paths_and_card_adapter(
        managed_paths,
        Arc::new(MockPhysicalCardAdapter::new()),
    )
    .expect("load app");
    app.create_project("demo", &project_dir)
        .expect("create project");

    let selector = selector_for(&project_dir);
    let error = app
        .start_project_simulation(&selector)
        .await
        .expect_err("invalid bundle root should fail startup");
    let message = error.to_string();
    assert!(message.contains("simulation `sim-"));
    assert!(message.contains("backend bundle manifest not found"));

    let simulations = app.list_simulations().expect("list simulations");
    assert_eq!(simulations.len(), 1);
    assert_eq!(simulations[0].status.as_str(), "failed");
    let simulation_selector = SimulationSelectorInput {
        simulation_id: simulations[0].simulation_id.clone(),
    };

    let events = app
        .simulation_events(&simulation_selector)
        .expect("simulation events");
    assert!(
        events
            .iter()
            .any(|event| event.message.contains("simulation prepared"))
    );
    assert!(
        events
            .iter()
            .any(|event| event.message.contains("simulation start failed"))
    );

    let stopped = app
        .stop_simulation(&simulation_selector)
        .await
        .expect("stop failed simulation");
    assert_eq!(stopped.status.as_str(), "stopped");

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn mock_card_session_and_typed_iso_gp_behavior_are_characterized() {
    let _service_lock = socket_support::acquire_cross_process_lock("local-service");
    let _guard = app_lock().lock().await;
    let root = temp_root("mock-card");
    let project_dir = root.join("demo");
    let app = JcimApp::load_with_paths_and_card_adapter(
        ManagedPaths::for_root(root.clone()),
        Arc::new(MockPhysicalCardAdapter::new()),
    )
    .expect("load app");
    app.create_project("demo", &project_dir)
        .expect("create project");

    let selector = selector_for(&project_dir);
    let install = app
        .install_project_cap(&selector, None)
        .await
        .expect("install project cap");
    assert!(!install.applets.is_empty());

    let readers = app.list_readers().await.expect("list readers");
    assert_eq!(readers.len(), 1);
    assert_eq!(readers[0].name, "Mock Reader 0");
    assert!(readers[0].card_present);

    let status = app.card_status(None).await.expect("card status");
    assert_eq!(status.reader_name, "Mock Reader 0");
    assert!(status.card_present);

    let packages = app.list_packages(None).await.expect("packages");
    assert_eq!(packages.reader_name, "Mock Reader 0");
    assert!(
        packages
            .packages
            .iter()
            .any(|package| package.aid == install.package_aid)
    );

    let applets = app.list_applets(None).await.expect("applets");
    assert!(
        applets
            .applets
            .iter()
            .any(|applet| applet.aid == install.applets[0].aid)
    );

    let selected = app
        .card_command(None, &globalplatform::select_issuer_security_domain())
        .await
        .expect("select isd");
    assert_eq!(selected.response.sw, 0x9000);

    let status_exchange = app
        .card_command(
            None,
            &globalplatform::get_status(
                globalplatform::RegistryKind::Applications,
                globalplatform::GetStatusOccurrence::FirstOrAll,
            ),
        )
        .await
        .expect("get status");
    let parsed = globalplatform::parse_get_status(
        globalplatform::RegistryKind::Applications,
        &status_exchange.response,
    )
    .expect("parse get status");
    assert!(
        parsed
            .entries
            .iter()
            .any(|entry| entry.aid.to_hex() == install.applets[0].aid)
    );

    let applet_aid = jcim_core::aid::Aid::from_hex(&install.applets[0].aid).expect("applet aid");
    let select = app
        .card_command(None, &iso7816::select_by_name(&applet_aid))
        .await
        .expect("select applet");
    assert_eq!(select.response.sw, 0x9000);
    assert_eq!(
        app.card_session_state(None)
            .expect("card session")
            .selected_aid,
        Some(applet_aid.clone())
    );

    let channel = app
        .manage_card_channel(None, true, None)
        .await
        .expect("open channel");
    assert_eq!(channel.channel_number, Some(1));

    let secure = app
        .open_card_secure_messaging(
            None,
            Some(jcim_core::iso7816::SecureMessagingProtocol::Scp03),
            Some(0x03),
            Some("mock-session".to_string()),
        )
        .expect("open card secure messaging");
    assert!(secure.session_state.secure_messaging.active);

    let advanced = app
        .advance_card_secure_messaging(None, 2)
        .expect("advance card secure messaging");
    assert_eq!(advanced.session_state.secure_messaging.command_counter, 2);

    let closed = app
        .close_card_secure_messaging(None)
        .expect("close card secure messaging");
    assert!(!closed.session_state.secure_messaging.active);

    let lock = app
        .card_command(
            None,
            &globalplatform::set_application_status(
                &applet_aid,
                globalplatform::LockTransition::Lock,
            ),
        )
        .await
        .expect("lock applet");
    assert!(jcim_core::iso7816::StatusWord::from(lock.response.sw).is_success());

    let locked_select = app
        .card_command(None, &iso7816::select_by_name(&applet_aid))
        .await
        .expect("select locked applet");
    assert_eq!(
        locked_select.response.sw,
        jcim_core::iso7816::StatusWord::WARNING_SELECTED_FILE_INVALIDATED.as_u16()
    );

    let unlock = app
        .card_command(
            None,
            &globalplatform::set_application_status(
                &applet_aid,
                globalplatform::LockTransition::Unlock,
            ),
        )
        .await
        .expect("unlock applet");
    assert!(jcim_core::iso7816::StatusWord::from(unlock.response.sw).is_success());

    let reset = app.reset_card_summary(None).await.expect("reset card");
    assert!(reset.atr.is_some());
    assert_eq!(reset.session_state.open_channels.len(), 1);

    let _ = std::fs::remove_dir_all(root);
}

fn selector_for(project_dir: &std::path::Path) -> ProjectSelectorInput {
    ProjectSelectorInput {
        project_path: Some(project_dir.to_path_buf()),
        project_id: None,
    }
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-app-characterization-{label}-{unique:x}"))
}
