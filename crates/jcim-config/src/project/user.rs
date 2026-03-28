//! Machine-local user configuration for the JCIM 0.3 local platform.

use std::path::{Path, PathBuf};

use crate::managed_files::write_regular_file_atomic;
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
        let encoded = toml::to_string_pretty(self).map_err(|error| {
            JcimError::Unsupported(format!("unable to encode user config: {error}"))
        })?;
        write_regular_file_atomic(path, encoded.as_bytes(), "machine-local user config")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::UserConfig;

    #[test]
    fn user_config_save_replaces_existing_contents() {
        let root = temp_root("replace");
        let path = root.join("config.toml");
        let first = UserConfig {
            java_bin: "java".to_string(),
            bundle_root: None,
            default_reader: Some("Reader A".to_string()),
        };
        let second = UserConfig {
            java_bin: "/usr/bin/java".to_string(),
            bundle_root: Some(PathBuf::from("/tmp/bundles")),
            default_reader: Some("Reader B".to_string()),
        };

        first.save_to_path(&path).expect("save first config");
        second.save_to_path(&path).expect("replace config");

        let saved = UserConfig::load_or_default(&path).expect("load config");
        assert_eq!(saved, second);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn user_config_save_rejects_symlink_destinations() {
        let root = temp_root("symlink");
        let target = root.join("target.toml");
        let path = root.join("config.toml");
        std::fs::create_dir_all(&root).expect("create root");
        std::fs::write(&target, "target").expect("write target");
        std::os::unix::fs::symlink(&target, &path).expect("symlink");

        let error = UserConfig::default()
            .save_to_path(&path)
            .expect_err("symlink should fail closed");
        assert!(
            error
                .to_string()
                .contains("refusing to overwrite symlinked")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from("/tmp").join(format!("jcim-user-config-{label}-{unique:x}"))
    }
}
