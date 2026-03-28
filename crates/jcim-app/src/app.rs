//! Local JCIM application services.
#![allow(clippy::missing_docs_in_private_items)]
// This module is the dense internal orchestration layer for the transport-neutral app service.
// We keep the public façade documented and avoid line-by-line docs on private glue code here.

mod builds;
mod cards;
mod events;
mod projects;
mod selectors;
mod simulations;
mod state;
mod system;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_backends::backend::BackendHandle;
use jcim_build::{
    ArtifactMetadata, artifact_metadata_from_project,
    build_project_artifacts_if_stale_with_java_bin, build_toolchain_layout, load_artifact_metadata,
};
use jcim_cap::prelude::CapPackage;
use jcim_config::config::RuntimeConfig;
use jcim_config::project::{
    BuildKind, ManagedPaths, PROJECT_MANIFEST_NAME, ProjectConfig, UserConfig, resolve_project_path,
};
use jcim_core::aid::Aid;
use jcim_core::apdu::CommandApdu;
use jcim_core::error::{JcimError, Result};
use jcim_core::globalplatform;
use jcim_core::iso7816;
use jcim_core::iso7816::{
    Atr, IsoCapabilities, IsoSessionState, ProtocolParameters, SecureMessagingProtocol,
};
use jcim_core::model::{BackendHealthStatus, BackendKind, CardProfileId, ProtocolVersion};

use crate::card::{
    JavaPhysicalCardAdapter, PhysicalCardAdapter, ResolvedGpKeyset, gppro_jar_path, helper_jar_path,
};
use crate::java_runtime::{JavaRuntimeSource, ResolvedJavaRuntime, resolve_java_runtime};
use crate::model::{
    ApduExchangeSummary, AppletSummary, ArtifactSummary, CardAppletInventory, CardDeleteSummary,
    CardInstallSummary, CardPackageInventory, CardReaderSummary, CardStatusSummary, EventLine,
    GpSecureChannelSummary, ManageChannelSummary, OverviewSummary, ProjectDetails,
    ProjectSelectorInput, ProjectSummary, ResetSummary, SecureMessagingSummary,
    ServiceStatusSummary, SetupSummary, SimulationSelectorInput, SimulationStatusKind,
    SimulationSummary,
};
use crate::registry::ProjectRegistry;

use self::events::remember_event;
use self::state::{AppState, PreparedSimulation, ResolvedProject, SimulationRecord};

/// Transport-neutral application façade for the JCIM 0.3 local platform.
#[derive(Clone)]
pub struct JcimApp {
    /// Shared mutable application state.
    state: Arc<AppState>,
}

impl JcimApp {
    /// Load the local application state from the managed JCIM directories.
    pub fn load() -> Result<Self> {
        Self::load_with_paths(ManagedPaths::discover()?)
    }

    /// Load the application state using an explicit managed root layout.
    pub fn load_with_paths(managed_paths: ManagedPaths) -> Result<Self> {
        Self::load_with_paths_and_card_adapter(managed_paths, Arc::new(JavaPhysicalCardAdapter))
    }

    /// Load the application state using an explicit managed root layout and card adapter.
    pub fn load_with_paths_and_card_adapter(
        managed_paths: ManagedPaths,
        card_adapter: Arc<dyn PhysicalCardAdapter>,
    ) -> Result<Self> {
        managed_paths.prepare_layout()?;

        let user_config = UserConfig::load_or_default(&managed_paths.config_path)?;
        let registry = ProjectRegistry::load_or_default(&managed_paths.registry_path)?;
        let (service_binary_path, service_binary_fingerprint) = current_service_binary_identity()?;
        let next_simulation_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            state: Arc::new(AppState::new(
                managed_paths,
                service_binary_path,
                service_binary_fingerprint,
                user_config,
                registry,
                card_adapter,
                next_simulation_id,
            )),
        })
    }

    /// Return the managed machine-local paths used by this application instance.
    pub fn managed_paths(&self) -> &ManagedPaths {
        &self.state.managed_paths
    }

    /// Return a high-level overview of the managed project and simulation state.
    pub fn overview(&self) -> Result<OverviewSummary> {
        Ok(OverviewSummary {
            known_project_count: self.list_projects()?.len() as u32,
            active_simulation_count: self.active_simulation_count(),
        })
    }
}

fn current_service_binary_identity() -> Result<(PathBuf, String)> {
    let path = std::env::current_exe()?;
    let metadata = std::fs::metadata(&path)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Ok((
        path,
        format!(
            "{}:{}:{}",
            metadata.len(),
            modified.as_secs(),
            modified.subsec_nanos()
        ),
    ))
}

trait BackendHealthStatusExt {
    fn status_string(self) -> &'static str;
}

impl BackendHealthStatusExt for BackendHealthStatus {
    fn status_string(self) -> &'static str {
        match self {
            BackendHealthStatus::Ready => "ready",
            BackendHealthStatus::Degraded => "degraded",
            BackendHealthStatus::Unavailable => "unavailable",
            _ => "unknown",
        }
    }
}

fn artifacts_from_metadata(
    project_root: &Path,
    metadata: &ArtifactMetadata,
) -> Vec<ArtifactSummary> {
    let mut artifacts = Vec::new();
    if let Some(path) = &metadata.cap_path {
        artifacts.push(ArtifactSummary {
            kind: "cap".to_string(),
            path: project_root.join(path),
        });
    }
    artifacts
}

fn required_artifact_path(
    project_root: &Path,
    relative: Option<&PathBuf>,
    message: &str,
) -> Result<PathBuf> {
    let relative = relative.ok_or_else(|| JcimError::Unsupported(message.to_string()))?;
    Ok(project_root.join(relative))
}

fn ensure_host_simulator_environment(_bundle_dir: &Path, _profile_id: CardProfileId) -> Result<()> {
    Ok(())
}

fn validate_simulation_artifacts(
    project_root: &Path,
    metadata: ArtifactMetadata,
) -> Result<ArtifactMetadata> {
    let cap_path = required_artifact_path(
        project_root,
        metadata.cap_path.as_ref(),
        "project build did not emit a CAP artifact required for simulation",
    )?;
    if !cap_path.exists() {
        return Err(JcimError::Unsupported(format!(
            "expected CAP artifact is missing at {}",
            cap_path.display()
        )));
    }
    let metadata_path = project_root.join(&metadata.simulator_metadata_path);
    if !metadata_path.exists() {
        return Err(JcimError::Unsupported(format!(
            "expected simulator metadata is missing at {}",
            metadata_path.display()
        )));
    }
    let classes_path = project_root.join(&metadata.classes_path);
    if !classes_path.exists() {
        return Err(JcimError::Unsupported(format!(
            "expected compiled classes are missing at {}",
            classes_path.display()
        )));
    }
    for dependency in &metadata.runtime_classpath {
        let dependency_path = project_root.join(dependency);
        if !dependency_path.exists() {
            return Err(JcimError::Unsupported(format!(
                "expected simulator runtime classpath entry is missing at {}",
                dependency_path.display()
            )));
        }
    }
    Ok(metadata)
}

fn default_bundle_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-backends")
}

fn gp_security_level(value: u8) -> globalplatform::SecurityLevel {
    match value {
        0x00 => globalplatform::SecurityLevel::None,
        0x01 => globalplatform::SecurityLevel::CommandMac,
        0x11 => globalplatform::SecurityLevel::CommandAndResponseMac,
        0x03 => globalplatform::SecurityLevel::CommandMacAndEncryption,
        0x13 => globalplatform::SecurityLevel::CommandAndResponseMacWithEncryption,
        other => globalplatform::SecurityLevel::Raw(other),
    }
}

fn gp_host_challenge() -> [u8; 8] {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut challenge = [0u8; 8];
    for (index, byte) in challenge.iter_mut().enumerate() {
        *byte = (nanos >> (index * 8)) as u8;
    }
    challenge
}

fn truncate_hex(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 16 {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..16])
    }
}

fn split_class_name(value: &str) -> Result<(String, String)> {
    if let Some((package_name, class_name)) = value.rsplit_once('.') {
        if class_name.is_empty() {
            return Err(JcimError::Unsupported(format!(
                "invalid applet class name `{value}`"
            )));
        }
        Ok((package_name.to_string(), class_name.to_string()))
    } else {
        Ok((String::new(), value.to_string()))
    }
}

fn sample_applet_source(package_name: &str, class_name: &str) -> String {
    let mut source = String::new();
    if !package_name.is_empty() {
        source.push_str(&format!("package {package_name};\n\n"));
    }
    source.push_str(
        "import javacard.framework.APDU;\n\
         import javacard.framework.Applet;\n\n",
    );
    source.push_str(&format!(
        "public final class {class_name} extends Applet {{\n\
             private {class_name}() {{}}\n\n\
             public static void install(byte[] buffer, short offset, byte length) {{\n\
                 new {class_name}().register();\n\
             }}\n\n\
             @Override\n\
             public void process(APDU apdu) {{\n\
                 if (selectingApplet()) {{\n\
                     return;\n\
                 }}\n\
                 apdu.setOutgoingAndSend((short) 0, (short) 0);\n\
             }}\n\
         }}\n"
    ));
    source
}

#[cfg(test)]
mod tests {
    use jcim_core::model::CardProfileId;

    use super::ensure_host_simulator_environment;

    #[test]
    fn host_environment_check_is_noop_for_managed_runtime() {
        ensure_host_simulator_environment(
            std::path::Path::new("/tmp/jcim/bundled-backends/simulator"),
            CardProfileId::Classic304,
        )
        .expect("managed java simulator environment");
    }
}
