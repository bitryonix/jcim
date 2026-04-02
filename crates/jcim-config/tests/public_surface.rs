//! Integration coverage for the `jcim-config` public surface.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_config::project::{
    ManagedPaths, ProjectConfig, ServiceRuntimeRecord, UserConfig,
    current_runtime_record_format_version, find_project_manifest, project_name_from_root,
    resolve_project_path, runtime_metadata_path_for_socket,
};

#[test]
fn project_manifest_helpers_round_trip_through_disk_and_path_resolution() {
    let root = temp_root("manifest");
    let project_root = root.join("demo-project");
    let nested = project_root.join("src/main/javacard/demo");
    std::fs::create_dir_all(&nested).expect("create nested source root");

    let manifest_path = project_root.join("jcim.toml");
    let manifest = ProjectConfig::default_for_project_name("Demo Project");
    std::fs::write(
        &manifest_path,
        manifest.to_pretty_toml().expect("encode manifest"),
    )
    .expect("write manifest");

    let loaded = ProjectConfig::from_toml_path(&manifest_path).expect("decode manifest");
    assert_eq!(loaded.metadata.name, "Demo Project");
    assert_eq!(loaded.source_root(), PathBuf::from("src/main/javacard"));
    assert!(!loaded.is_command_build());

    assert_eq!(
        find_project_manifest(&nested).expect("discover manifest"),
        manifest_path
    );
    assert_eq!(project_name_from_root(&project_root), "demo-project");
    assert_eq!(
        resolve_project_path(&project_root, Path::new("dist/demo.cap")),
        project_root.join("dist/demo.cap")
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn managed_paths_prepare_layout_and_runtime_metadata_round_trip() {
    let root = temp_root("managed-layout");
    let managed_paths = ManagedPaths::for_root(root.join("managed"));
    managed_paths.prepare_layout().expect("prepare layout");

    assert!(managed_paths.config_dir.exists());
    assert!(managed_paths.state_dir.exists());
    assert!(managed_paths.runtime_dir.exists());
    assert_eq!(
        runtime_metadata_path_for_socket(&managed_paths.service_socket_path),
        managed_paths.runtime_metadata_path
    );

    let user_config = UserConfig {
        java_bin: "/opt/jcim/java".to_string(),
        bundle_root: Some(managed_paths.bundle_root.clone()),
        default_reader: Some("Reader 0".to_string()),
    };
    user_config
        .save_to_path(&managed_paths.config_path)
        .expect("save user config");
    assert_eq!(
        UserConfig::load_or_default(&managed_paths.config_path).expect("load user config"),
        user_config
    );

    let runtime_record = ServiceRuntimeRecord {
        format_version: current_runtime_record_format_version(),
        pid: std::process::id(),
        socket_path: managed_paths.service_socket_path.clone(),
        service_binary_path: PathBuf::from("/tmp/jcimd"),
        service_binary_fingerprint: "fingerprint".to_string(),
    };
    runtime_record
        .write_to_path(&managed_paths.runtime_metadata_path)
        .expect("write runtime metadata");
    assert_eq!(
        ServiceRuntimeRecord::load_if_present(&managed_paths.runtime_metadata_path)
            .expect("load runtime metadata")
            .expect("runtime metadata present"),
        runtime_record
    );

    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-config-public-{label}-{unique:x}"))
}
