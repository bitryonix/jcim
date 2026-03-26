//! Runtime configuration loading and profile resolution for JCIM.

use std::path::{Path, PathBuf};

use jcim_core::error::Result;
use jcim_core::model::{BackendKind, CardProfile, CardProfileId, HardwareProfile};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
/// Backend-specific configuration shared by the local service and runtime startup paths.
pub struct BackendConfig {
    /// Backend implementation to launch.
    pub kind: BackendKind,
    /// Java executable override used when JCIM does not select a bundled runtime automatically.
    pub java_bin: String,
    /// Root directory that contains simulator bundle subdirectories.
    pub bundle_root: PathBuf,
}

impl Default for BackendConfig {
    fn default() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../bundled-backends")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from("bundled-backends"));
        Self {
            kind: BackendKind::Simulator,
            java_bin: "java".to_string(),
            bundle_root: root,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
/// Top-level simulator configuration used by the local service, backend adapters, and tests.
pub struct RuntimeConfig {
    /// Backend process selection.
    pub backend: BackendConfig,
    /// Card profile to resolve before runtime overrides are applied.
    pub profile_id: CardProfileId,
    /// Optional hardware override merged on top of the builtin profile.
    pub hardware_override: Option<HardwareProfile>,
    /// Optional CAP artifact path reported to the backend for diagnostics and inventory context.
    pub cap_path: Option<PathBuf>,
    /// Optional compiled classes directory used by the managed Java simulator.
    pub classes_path: Option<PathBuf>,
    /// Optional extra runtime classpath entries used to load applet dependencies.
    #[serde(default)]
    pub runtime_classpath: Vec<PathBuf>,
    /// Optional simulator metadata file used by source-backed simulator flows.
    pub simulator_metadata_path: Option<PathBuf>,
    /// Additional export hint files used during CAP import validation.
    pub export_paths: Vec<PathBuf>,
    /// Optional ATR override applied after profile resolution.
    pub atr: Option<Vec<u8>>,
    /// Optional reader name override reported to clients and external bundles.
    pub reader_name: Option<String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            backend: BackendConfig::default(),
            profile_id: CardProfileId::Classic305,
            hardware_override: None,
            cap_path: None,
            classes_path: None,
            runtime_classpath: Vec::new(),
            simulator_metadata_path: None,
            export_paths: Vec::new(),
            atr: None,
            reader_name: None,
        }
    }
}

impl RuntimeConfig {
    /// Decode runtime configuration from a TOML string.
    pub fn from_toml_str(value: &str) -> Result<Self> {
        Ok(toml::from_str(value)?)
    }

    /// Decode runtime configuration from a TOML file on disk.
    pub fn from_toml_path(path: &Path) -> Result<Self> {
        Self::from_toml_str(&std::fs::read_to_string(path)?)
    }

    /// Resolve the effective card profile after applying hardware, ATR, and reader-name overrides.
    pub fn resolve_profile(&self) -> CardProfile {
        let mut profile = CardProfile::builtin(self.profile_id);
        // Hardware overrides intentionally apply before ATR and reader-name overrides so callers
        // can replace the whole hardware model and still customize those two common fields last.
        if let Some(hardware) = &self.hardware_override {
            profile.hardware = hardware.clone();
        }
        if let Some(atr) = &self.atr {
            profile.hardware.atr = atr.clone();
        }
        if let Some(reader_name) = &self.reader_name {
            profile.reader_name = reader_name.clone();
        }
        profile
    }

    /// Return the bundle directory that corresponds to the selected backend kind.
    pub fn backend_bundle_dir(&self) -> PathBuf {
        self.backend
            .bundle_root
            .join(self.backend.kind.default_bundle_subdir())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jcim_core::model::{
        BackendKind, CardProfile, CardProfileId, HardwareProfile, MemoryLimits,
    };

    use super::{BackendConfig, RuntimeConfig};

    #[test]
    fn runtime_config_defaults_to_classic305_builtin() {
        let config = RuntimeConfig::default();
        assert_eq!(config.backend.kind, BackendKind::Simulator);
        assert_eq!(config.profile_id, CardProfileId::Classic305);
    }

    #[test]
    fn runtime_config_resolves_profile_overrides() {
        let config = RuntimeConfig {
            profile_id: CardProfileId::Classic22,
            reader_name: Some("Custom Reader".to_string()),
            atr: Some(vec![0x3B, 0x99]),
            hardware_override: Some(HardwareProfile {
                name: "Lab card".to_string(),
                atr: vec![0x3B, 0x22],
                memory: MemoryLimits {
                    persistent_bytes: 1234,
                    transient_reset_bytes: 10,
                    transient_deselect_bytes: 11,
                    apdu_buffer_bytes: 12,
                    commit_buffer_bytes: 13,
                    install_scratch_bytes: 14,
                    stack_bytes: 15,
                    page_bytes: 16,
                    erase_block_bytes: 17,
                    journal_bytes: 18,
                    wear_limit: Some(19),
                },
                max_apdu_size: 20,
                supports_scp02: true,
                supports_scp03: false,
            }),
            ..RuntimeConfig::default()
        };

        let profile = config.resolve_profile();
        assert_eq!(profile.reader_name, "Custom Reader");
        assert_eq!(profile.hardware.name, "Lab card");
        assert_eq!(profile.hardware.atr, vec![0x3B, 0x99]);
        assert_eq!(profile.hardware.memory.persistent_bytes, 1234);
    }

    #[test]
    fn runtime_config_parses_from_toml_and_resolves_bundle_dir() {
        let config = RuntimeConfig {
            backend: BackendConfig {
                kind: BackendKind::Simulator,
                java_bin: "java17".to_string(),
                bundle_root: PathBuf::from("/opt/jcim-backends"),
            },
            classes_path: Some(PathBuf::from("/opt/jcim-build/classes")),
            runtime_classpath: vec![PathBuf::from("/opt/jcim-build/lib/helper.jar")],
            simulator_metadata_path: Some(PathBuf::from("/opt/jcim-build/simulator.properties")),
            ..RuntimeConfig::default()
        };
        let encoded = toml::to_string(&config).expect("encode");
        let decoded = RuntimeConfig::from_toml_str(&encoded).expect("decode");
        assert_eq!(decoded.backend.kind, BackendKind::Simulator);
        assert_eq!(decoded.backend.java_bin, "java17");
        assert_eq!(
            decoded.classes_path,
            Some(PathBuf::from("/opt/jcim-build/classes"))
        );
        assert_eq!(
            decoded.runtime_classpath,
            vec![PathBuf::from("/opt/jcim-build/lib/helper.jar")]
        );
        assert_eq!(
            decoded.simulator_metadata_path,
            Some(PathBuf::from("/opt/jcim-build/simulator.properties"))
        );
        assert_eq!(
            decoded.backend_bundle_dir(),
            PathBuf::from("/opt/jcim-backends/simulator")
        );
    }

    #[test]
    fn resolve_profile_matches_builtin_profile_defaults_without_overrides() {
        let config = RuntimeConfig {
            profile_id: CardProfileId::Classic221,
            ..RuntimeConfig::default()
        };
        assert_eq!(
            config.resolve_profile(),
            CardProfile::builtin(CardProfileId::Classic221)
        );
    }
}
