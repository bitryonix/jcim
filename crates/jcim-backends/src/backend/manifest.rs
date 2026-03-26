//! Bundle manifest parsing and configuration validation for external backends.

use std::collections::BTreeMap;
use std::path::Path;

use jcim_config::config::RuntimeConfig;
use jcim_core::error::{JcimError, Result};
use jcim_core::model::{BackendKind, CardProfileId, ProtocolVersion};
use serde::Deserialize;

use super::reply::validate_protocol;

/// Bundle manifest loaded from `manifest.toml` for an external backend.
#[derive(Debug, Deserialize)]
pub(super) struct BackendBundleManifest {
    /// Protocol version the bundle expects to speak.
    #[serde(default = "default_protocol_version")]
    pub(super) protocol_version: ProtocolVersion,
    /// JVM main class used to launch Java-backed bundles.
    pub(super) main_class: String,
    /// Classpath entries resolved relative to the bundle directory for Java-backed bundles.
    pub(super) classpath: Vec<String>,
    /// Additional process arguments passed before JCIM-managed arguments.
    #[serde(default)]
    pub(super) args: Vec<String>,
    /// Additional environment variables exported to the backend process.
    #[serde(default)]
    pub(super) env: BTreeMap<String, String>,
    /// Startup timeout in milliseconds for the handshake probe.
    #[serde(default = "default_startup_timeout_ms")]
    pub(super) startup_timeout_ms: u64,
    /// Whether the bundle accepts a CAP path at startup.
    #[serde(default)]
    pub(super) accepts_cap: bool,
    /// Explicitly supported profiles, or empty for "not declared".
    #[serde(default)]
    pub(super) supported_profiles: Vec<CardProfileId>,
}

/// Default the manifest protocol version to the current workspace version.
pub(super) fn default_protocol_version() -> ProtocolVersion {
    ProtocolVersion::current()
}

/// Default startup timeout used when the manifest omits one.
pub(super) fn default_startup_timeout_ms() -> u64 {
    3_000
}

/// Validate that runtime config and bundle manifest agree on supported inputs and versions.
pub(super) fn validate_external_config(
    config: &RuntimeConfig,
    manifest: &BackendBundleManifest,
) -> Result<()> {
    validate_protocol(ProtocolVersion::current(), manifest.protocol_version)?;
    validate_launch_contract(manifest)?;

    if !manifest.supported_profiles.is_empty()
        && !manifest.supported_profiles.contains(&config.profile_id)
    {
        return Err(JcimError::Unsupported(format!(
            "the selected backend bundle does not support profile {}",
            config.profile_id
        )));
    }

    match config.backend.kind {
        BackendKind::Simulator => {
            if config.cap_path.is_none() {
                return Err(JcimError::Unsupported(
                    "the simulator backend requires a CAP path".to_string(),
                ));
            }
            if !manifest.accepts_cap {
                return Err(JcimError::Unsupported(
                    "the selected simulator bundle manifest does not accept a CAP path".to_string(),
                ));
            }
            Ok(())
        }
        _ => Err(JcimError::Unsupported(
            "unsupported external backend kind".to_string(),
        )),
    }
}

/// Validate that the manifest declares the launch metadata required by the JVM launcher.
fn validate_launch_contract(manifest: &BackendBundleManifest) -> Result<()> {
    if manifest.main_class.trim().is_empty() {
        return Err(JcimError::BackendStartup(
            "java backend manifest is missing main_class".to_string(),
        ));
    }
    if manifest.classpath.is_empty() {
        return Err(JcimError::BackendStartup(
            "java backend manifest is missing classpath entries".to_string(),
        ));
    }
    Ok(())
}

/// Resolve manifest classpath entries against the bundle directory.
pub(super) fn resolve_classpath(bundle_dir: &Path, entries: &[String]) -> Result<Vec<String>> {
    let mut classpath = Vec::new();
    for entry in entries {
        if let Some(prefix) = entry.strip_suffix("/*") {
            let dir = bundle_dir.join(prefix);
            if !dir.exists() {
                continue;
            }
            let mut children = std::fs::read_dir(dir)?
                .filter_map(|item| item.ok())
                .map(|item| item.path())
                .collect::<Vec<_>>();
            // Sorting keeps classpath expansion deterministic so startup and tests do not depend on
            // filesystem iteration order.
            children.sort();
            classpath.extend(children.into_iter().map(|path| path.display().to_string()));
        } else {
            let path = bundle_dir.join(entry);
            if path.exists() {
                classpath.push(path.display().to_string());
            }
        }
    }
    Ok(classpath)
}
