//! Build-request resolution from clean-slate project manifests.

use jcim_config::project::{ProjectConfig, resolve_project_path};
use jcim_core::error::Result;

use super::types::{BuildAppletMetadata, BuildArtifactRequest};

/// Build a manifest-derived request from one `jcim.toml`.
pub fn artifact_metadata_from_project(
    project_root: &std::path::Path,
    project_config: &ProjectConfig,
) -> Result<BuildArtifactRequest> {
    let mut source_roots = vec![resolve_project_path(
        project_root,
        &project_config.source_root(),
    )];
    source_roots.extend(
        project_config
            .source
            .extra_roots
            .iter()
            .map(|path| resolve_project_path(project_root, path)),
    );

    Ok(BuildArtifactRequest {
        project_root: project_root.to_path_buf(),
        build_kind: project_config.build.kind,
        source_roots,
        command: project_config.build.command.clone(),
        cap_output: project_config
            .build
            .cap_output
            .as_ref()
            .map(|path| resolve_project_path(project_root, path)),
        profile: project_config.metadata.profile,
        emit: project_config.build.emit.clone(),
        package_name: project_config.metadata.package_name.clone(),
        package_aid: project_config.metadata.package_aid.clone(),
        version: project_config.build.version.clone(),
        applets: project_config
            .metadata
            .applets
            .clone()
            .into_iter()
            .map(|applet| BuildAppletMetadata {
                class_name: applet.class_name,
                aid: applet.aid,
            })
            .collect(),
        dependencies: project_config
            .build
            .dependencies
            .iter()
            .map(|path| resolve_project_path(project_root, path))
            .collect(),
    })
}
