//! Build metadata persistence helpers.

use std::path::{Path, PathBuf};

use jcim_core::error::{JcimError, Result};

use super::types::{ArtifactMetadata, BuildAppletMetadata};

/// Load artifact metadata from `.jcim/build/metadata.toml` when it exists.
pub fn load_artifact_metadata(project_root: &Path) -> Result<Option<ArtifactMetadata>> {
    let path = artifact_metadata_path(project_root);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(toml::from_str(&std::fs::read_to_string(path)?)?))
}

/// Persist stable build metadata so later workflow commands can reuse the last successful build.
pub(crate) fn save_artifact_metadata(
    project_root: &Path,
    metadata: &ArtifactMetadata,
) -> Result<()> {
    let path = artifact_metadata_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        toml::to_string_pretty(metadata).map_err(|error| {
            JcimError::Unsupported(format!("unable to encode artifact metadata: {error}"))
        })?,
    )?;
    Ok(())
}

/// Return the standard project-local build directory under `.jcim/build/`.
pub(crate) fn project_build_root(project_root: &Path) -> PathBuf {
    project_root.join(".jcim/build")
}

/// Return the path to the persisted artifact metadata file for one project.
fn artifact_metadata_path(project_root: &Path) -> PathBuf {
    project_build_root(project_root).join("metadata.toml")
}

/// Emit the simulator metadata file that backend bundles read at startup.
pub(crate) fn write_simulator_metadata(
    path: &Path,
    package_name: &str,
    package_aid: &jcim_core::aid::Aid,
    version: &str,
    applets: &[BuildAppletMetadata],
) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut contents = String::new();
    contents.push_str(&format!("package.name={package_name}\n"));
    contents.push_str(&format!("package.aid={}\n", package_aid.to_hex()));
    contents.push_str(&format!("package.version={version}\n"));
    contents.push_str(&format!("applet.count={}\n", applets.len()));
    for (index, applet) in applets.iter().enumerate() {
        contents.push_str(&format!("applet.{index}.class={}\n", applet.class_name));
        contents.push_str(&format!("applet.{index}.aid={}\n", applet.aid.to_hex()));
    }
    std::fs::write(path, contents)?;
    Ok(())
}

/// Return a root-relative path when possible so metadata remains project-portable.
pub(crate) fn relativize_path(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}
