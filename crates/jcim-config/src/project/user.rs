//! Machine-local user configuration for the JCIM 0.2 local platform.

use std::path::{Path, PathBuf};

use jcim_core::error::{JcimError, Result};
use serde::{Deserialize, Serialize};

/// Machine-local user configuration stored under the managed JCIM root.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct UserConfig {
    /// Java executable used for JVM-backed simulation and helper tools.
    pub java_bin: String,
    /// Managed bundle root used by runtime adapters.
    pub bundle_root: Option<PathBuf>,
    /// Preferred physical reader name for card operations.
    pub default_reader: Option<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            java_bin: "java".to_string(),
            bundle_root: None,
            default_reader: None,
        }
    }
}

impl UserConfig {
    /// Load user config from disk or return defaults when the file does not exist.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// Save user config to disk.
    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            path,
            toml::to_string_pretty(self).map_err(|error| {
                JcimError::Unsupported(format!("unable to encode user config: {error}"))
            })?,
        )?;
        Ok(())
    }
}
