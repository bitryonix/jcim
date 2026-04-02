//! Integration coverage for the `jcim-build` public surface.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_build::{
    artifact_metadata_from_project, build_project_artifacts_if_stale_with_java_bin,
    build_toolchain_layout, load_artifact_metadata,
};
use jcim_config::project::{BuildKind, ProjectConfig};

#[test]
fn external_command_builds_persist_metadata_and_reuse_stale_results() {
    let root = temp_root("external-command");
    let project_root = root.join("demo");
    std::fs::create_dir_all(&project_root).expect("create project root");

    let mut config = ProjectConfig::default_for_project_name("Demo");
    config.build.kind = BuildKind::Command;
    config.build.command = Some("mkdir -p dist && printf 'cap' > dist/demo.cap".to_string());
    config.build.cap_output = Some(PathBuf::from("dist/demo.cap"));

    let request = artifact_metadata_from_project(&project_root, &config).expect("build request");
    let toolchain = build_toolchain_layout().expect("toolchain");

    let first = build_project_artifacts_if_stale_with_java_bin(
        &request,
        &toolchain,
        Path::new("/bin/false"),
    )
    .expect("first external build");
    assert!(first.rebuilt);
    let first_cap = first
        .metadata
        .cap_path
        .as_ref()
        .expect("external build cap path");
    assert!(project_root.join(first_cap).exists());

    let saved = load_artifact_metadata(&project_root)
        .expect("load metadata")
        .expect("metadata present");
    assert_eq!(saved, first.metadata);

    let second = build_project_artifacts_if_stale_with_java_bin(
        &request,
        &toolchain,
        Path::new("/bin/false"),
    )
    .expect("reused external build");
    assert!(!second.rebuilt);
    assert_eq!(second.metadata, first.metadata);

    let mut changed_request = request.clone();
    changed_request.version = "2.0".to_string();
    let changed = build_project_artifacts_if_stale_with_java_bin(
        &changed_request,
        &toolchain,
        Path::new("/bin/false"),
    )
    .expect("changed external build");
    assert!(changed.rebuilt);
    assert_ne!(
        changed.metadata.source_fingerprint,
        first.metadata.source_fingerprint
    );

    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-build-public-{label}-{unique:x}"))
}
