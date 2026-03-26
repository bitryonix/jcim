//! Source discovery and fingerprinting for stale-build detection.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use jcim_core::error::Result;

use super::types::BuildArtifactRequest;

/// Hash source files and build-relevant metadata so stale-build detection stays deterministic.
pub(crate) fn compute_source_fingerprint(request: &BuildArtifactRequest) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(format!("{:?}", request.build_kind));
    hasher.update(request.profile.to_string());
    hasher.update(request.package_name.as_bytes());
    hasher.update(request.package_aid.to_hex().as_bytes());
    hasher.update(request.version.as_bytes());
    if let Some(command) = &request.command {
        hasher.update(command.as_bytes());
    }
    for emit in &request.emit {
        hasher.update(format!("{emit:?}"));
    }
    for applet in &request.applets {
        hasher.update(applet.class_name.as_bytes());
        hasher.update(applet.aid.to_hex().as_bytes());
    }
    for dependency in &request.dependencies {
        hasher.update(dependency.display().to_string().as_bytes());
    }
    for source in discover_java_sources(request)? {
        hasher.update(source.display().to_string().as_bytes());
        hasher.update(std::fs::read(source)?);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Discover Java source inputs for the request, honoring explicit file overrides first.
pub(crate) fn discover_java_sources(request: &BuildArtifactRequest) -> Result<Vec<PathBuf>> {
    let mut discovered = Vec::new();
    for root in &request.source_roots {
        collect_java_sources(root, &mut discovered)?;
    }
    discovered.sort();
    discovered.dedup();
    Ok(discovered)
}

/// Recursively collect `.java` files below one source root.
fn collect_java_sources(root: &Path, discovered: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_java_sources(&path, discovered)?;
        } else if path.extension().and_then(OsStr::to_str) == Some("java") {
            discovered.push(path);
        }
    }
    Ok(())
}
