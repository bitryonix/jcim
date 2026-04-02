//! Integration coverage for the `jcim-backends` public surface.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_backends::backend::BackendHandle;
use jcim_build::{
    artifact_metadata_from_project, build_project_artifacts_if_stale_with_java_bin,
    build_toolchain_layout,
};
use jcim_config::config::RuntimeConfig;
use jcim_config::project::{ManagedPaths, ProjectConfig, UserConfig};
use jcim_core::aid::Aid;
use jcim_core::iso7816;
use jcim_core::model::ProtocolVersion;

#[tokio::test]
async fn backend_handle_fails_closed_when_required_runtime_inputs_are_missing() {
    let error = BackendHandle::from_config(RuntimeConfig::default())
        .err()
        .expect("missing classes and metadata should fail");
    assert!(
        error
            .to_string()
            .contains("requires a compiled classes path")
    );
}

#[tokio::test]
async fn backend_handle_serves_handshake_snapshot_and_apdu_traffic_for_built_project() {
    let root = temp_root("backend-handle");
    let managed_paths = ManagedPaths::for_root(root.join("managed"));
    let app = jcim_app::JcimApp::load_with_paths(managed_paths.clone()).expect("load app");
    app.setup_toolchains(None)
        .expect("setup managed toolchains");

    let user_config = UserConfig::load_or_default(&managed_paths.config_path).expect("load config");
    let project_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/satochip/workdir");
    let project = ProjectConfig::from_toml_path(&project_root.join("jcim.toml")).expect("manifest");
    let request = artifact_metadata_from_project(&project_root, &project).expect("build request");
    let toolchain = build_toolchain_layout().expect("toolchain");
    let outcome = build_project_artifacts_if_stale_with_java_bin(
        &request,
        &toolchain,
        PathBuf::from(&user_config.java_bin).as_path(),
    )
    .expect("build project");

    let mut runtime_config = RuntimeConfig {
        profile_id: project.metadata.profile,
        cap_path: outcome
            .metadata
            .cap_path
            .clone()
            .map(|path| project_root.join(path)),
        classes_path: Some(project_root.join(&outcome.metadata.classes_path)),
        runtime_classpath: outcome
            .metadata
            .runtime_classpath
            .iter()
            .map(|path| project_root.join(path))
            .collect(),
        simulator_metadata_path: Some(project_root.join(&outcome.metadata.simulator_metadata_path)),
        reader_name: Some("Integration Reader".to_string()),
        ..RuntimeConfig::default()
    };
    runtime_config.backend.java_bin = user_config.java_bin;
    runtime_config.backend.bundle_root = user_config
        .bundle_root
        .expect("setup toolchains should persist bundle root");

    let handle = BackendHandle::from_config(runtime_config).expect("spawn backend");
    let handshake = handle
        .handshake(ProtocolVersion::current())
        .await
        .expect("handshake");
    assert_eq!(handshake.protocol_version, ProtocolVersion::current());
    assert!(handshake.backend_capabilities.supports_apdu);
    assert_eq!(handshake.reader_name, "Integration Reader");

    let select = handle
        .transmit_typed_apdu(iso7816::select_by_name(
            &Aid::from_hex("5361746F4368697000").expect("aid"),
        ))
        .await
        .expect("select applet");
    assert_eq!(select.response.sw, 0x9000);

    let snapshot = handle.snapshot().await.expect("snapshot");
    assert_eq!(snapshot.reader_name, "Integration Reader");
    assert!(snapshot.power_on);
    assert!(snapshot.selected_aid.is_some());

    handle.shutdown().await.expect("shutdown backend");
    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-backends-public-{label}-{unique:x}"))
}
