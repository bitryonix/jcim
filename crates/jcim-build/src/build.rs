//! Java Card source build orchestration.
//!
//! # Why this exists
//! Source-first JCIM workflows need a maintained place to discover Java inputs, invoke the bundled
//! Java Card toolchain, and publish stable artifact metadata for both simulator and real-card
//! commands.

mod executor;
mod fingerprint;
mod metadata;
mod request;
mod toolchain;
mod types;

pub use metadata::load_artifact_metadata;
pub use request::artifact_metadata_from_project;
pub use toolchain::build_toolchain_layout;
pub use types::{
    ArtifactMetadata, BuildAppletMetadata, BuildArtifactRequest, BuildOutcome, ToolchainLayout,
};

use std::path::Path;

use jcim_core::error::Result;

use self::executor::{build_external_project, build_jcim_project};
use self::fingerprint::compute_source_fingerprint;
use self::metadata::save_artifact_metadata;

/// Build one request unconditionally, overwriting old artifacts.
pub fn build_project_artifacts(
    request: &BuildArtifactRequest,
    toolchain: &ToolchainLayout,
) -> Result<BuildOutcome> {
    build_project_artifacts_with_java_bin(request, toolchain, Path::new("java"))
}

/// Build one request unconditionally with one explicit Java runtime executable.
pub fn build_project_artifacts_with_java_bin(
    request: &BuildArtifactRequest,
    toolchain: &ToolchainLayout,
    java_bin: &Path,
) -> Result<BuildOutcome> {
    let fingerprint = compute_source_fingerprint(request)?;
    let metadata = match request.build_kind {
        jcim_config::project::BuildKind::Native => {
            build_jcim_project(request, toolchain, &fingerprint, java_bin)?
        }
        jcim_config::project::BuildKind::Command => {
            build_external_project(request, &fingerprint, java_bin)?
        }
    };
    save_artifact_metadata(&request.project_root, &metadata)?;
    Ok(BuildOutcome {
        metadata,
        rebuilt: true,
    })
}

/// Build one request only when the current metadata is stale or missing.
pub fn build_project_artifacts_if_stale(
    request: &BuildArtifactRequest,
    toolchain: &ToolchainLayout,
) -> Result<BuildOutcome> {
    build_project_artifacts_if_stale_with_java_bin(request, toolchain, Path::new("java"))
}

/// Build one request only when the current metadata is stale or missing using one explicit Java
/// runtime executable.
pub fn build_project_artifacts_if_stale_with_java_bin(
    request: &BuildArtifactRequest,
    toolchain: &ToolchainLayout,
    java_bin: &Path,
) -> Result<BuildOutcome> {
    let fingerprint = compute_source_fingerprint(request)?;
    if let Some(metadata) = load_artifact_metadata(&request.project_root)? {
        let cap_ok = metadata
            .cap_path
            .as_ref()
            .is_none_or(|path| request.project_root.join(path).exists());
        let simulator_metadata_ok = request
            .project_root
            .join(&metadata.simulator_metadata_path)
            .exists();
        let classes_ok = request.project_root.join(&metadata.classes_path).exists();
        let runtime_classpath_ok = metadata
            .runtime_classpath
            .iter()
            .all(|path| request.project_root.join(path).exists());
        if metadata.source_fingerprint == fingerprint
            && cap_ok
            && simulator_metadata_ok
            && classes_ok
            && runtime_classpath_ok
        {
            return Ok(BuildOutcome {
                metadata,
                rebuilt: false,
            });
        }
    }

    build_project_artifacts_with_java_bin(request, toolchain, java_bin)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use jcim_config::project::{
        ArtifactKind, BuildKind, ProjectBuildConfig, ProjectConfig, ProjectMetadataConfig,
        ProjectSourceConfig,
    };
    use jcim_core::model::CardProfileId;

    use super::executor::format_aid_for_converter;
    use super::fingerprint::compute_source_fingerprint;
    use super::{BuildArtifactRequest, artifact_metadata_from_project, build_toolchain_layout};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("jcim-build-{name}-{unique}"))
    }

    #[test]
    fn build_toolchain_layout_points_into_third_party() {
        let layout = build_toolchain_layout().expect("toolchain");
        assert!(layout.ecj_jar.ends_with("third_party/ecj/ecj.jar"));
        assert!(layout.sdk_root.ends_with("third_party/javacard_sdks"));
    }

    #[test]
    fn converter_aid_format_matches_oracle_cli_expectations() {
        let aid = jcim_core::aid::Aid::from_hex("A00000006203010C01").expect("aid");
        assert_eq!(
            format_aid_for_converter(&aid),
            "0xA0:0x00:0x00:0x00:0x62:0x03:0x01:0x0C:0x01"
        );
    }

    #[test]
    fn project_request_requires_source_metadata() {
        let root = temp_dir("request");
        std::fs::create_dir_all(&root).expect("mkdir");
        let mut project = ProjectConfig::default_for_project_name("demo");
        project.source = ProjectSourceConfig {
            root: Some(std::path::PathBuf::from("src/main/javacard")),
            ..ProjectSourceConfig::default()
        };
        project.build = ProjectBuildConfig {
            kind: BuildKind::Native,
            emit: vec![ArtifactKind::Cap],
            ..ProjectBuildConfig::default()
        };
        project.metadata = ProjectMetadataConfig {
            profile: CardProfileId::Classic305,
            package_name: "demo.pkg".to_string(),
            package_aid: jcim_core::aid::Aid::from_hex("0102030405").expect("aid"),
            applets: vec![jcim_config::project::ProjectAppletConfig {
                class_name: "demo.HelloApplet".to_string(),
                aid: jcim_core::aid::Aid::from_hex("010203040506").expect("aid"),
            }],
            ..ProjectMetadataConfig::default()
        };

        let request = artifact_metadata_from_project(&root, &project).expect("request");
        assert_eq!(request.package_name, "demo.pkg");
        assert_eq!(request.profile, CardProfileId::Classic305);
    }

    #[test]
    fn fingerprint_changes_with_source_contents() {
        let root = temp_dir("fingerprint");
        let src = root.join("src/main/javacard/demo");
        std::fs::create_dir_all(&src).expect("mkdir");
        let source = src.join("HelloApplet.java");
        std::fs::write(&source, b"package demo; class HelloApplet {}").expect("write");
        let request = BuildArtifactRequest {
            project_root: root.clone(),
            build_kind: jcim_config::project::BuildKind::Native,
            source_roots: vec![root.join("src/main/javacard")],
            command: None,
            cap_output: None,
            profile: CardProfileId::Classic305,
            emit: vec![jcim_config::project::ArtifactKind::Cap],
            package_name: "demo.pkg".to_string(),
            package_aid: jcim_core::aid::Aid::from_hex("0102030405").expect("aid"),
            version: "1.0".to_string(),
            applets: vec![super::BuildAppletMetadata {
                class_name: "demo.HelloApplet".to_string(),
                aid: jcim_core::aid::Aid::from_hex("010203040506").expect("aid"),
            }],
            dependencies: Vec::new(),
        };
        let first = compute_source_fingerprint(&request).expect("first");
        std::fs::write(&source, b"package demo; class HelloApplet { int x; }").expect("write");
        let second = compute_source_fingerprint(&request).expect("second");
        assert_ne!(first, second);
    }
}
