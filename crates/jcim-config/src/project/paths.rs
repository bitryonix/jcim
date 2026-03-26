//! Project and machine-local path helpers.

use std::env;
use std::path::{Path, PathBuf};

use jcim_core::error::{JcimError, Result};

/// Canonical manifest file name.
pub const PROJECT_MANIFEST_NAME: &str = "jcim.toml";
/// Machine-local user config file name.
pub const USER_CONFIG_FILE_NAME: &str = "config.toml";
/// Managed subdirectory that stores bundled runtime assets.
pub const MANAGED_BUNDLES_DIR_NAME: &str = "bundled";

/// Machine-local JCIM directories discovered for the current platform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedPaths {
    /// Root application-data directory for JCIM.
    pub root: PathBuf,
    /// User config file path.
    pub config_path: PathBuf,
    /// Known-project registry path.
    pub registry_path: PathBuf,
    /// Managed local service socket path.
    pub service_socket_path: PathBuf,
    /// Managed log directory.
    pub log_dir: PathBuf,
    /// Managed bundled-asset root.
    pub bundle_root: PathBuf,
}

impl ManagedPaths {
    /// Discover machine-local JCIM paths.
    pub fn discover() -> Result<Self> {
        let root = managed_root_dir()?;
        Ok(Self::for_root(root))
    }

    /// Build managed JCIM paths under one explicit root.
    pub fn for_root(root: PathBuf) -> Self {
        Self {
            config_path: root.join(USER_CONFIG_FILE_NAME),
            registry_path: root.join("projects.toml"),
            service_socket_path: root.join("run").join("jcimd.sock"),
            log_dir: root.join("logs"),
            bundle_root: root.join(MANAGED_BUNDLES_DIR_NAME),
            root,
        }
    }
}

/// Search upward from one directory for `jcim.toml`.
pub fn find_project_manifest(start_dir: &Path) -> Option<PathBuf> {
    start_dir
        .ancestors()
        .map(|dir| dir.join(PROJECT_MANIFEST_NAME))
        .find(|candidate| candidate.exists())
}

/// Return the human-facing project name derived from one root path.
pub fn project_name_from_root(project_root: &Path) -> String {
    project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("jcim-project")
        .to_string()
}

/// Resolve one project-local path relative to the project root unless already absolute.
pub fn resolve_project_path(project_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

/// Resolve the default managed JCIM root for the current supported platform.
fn managed_root_dir() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME").ok_or_else(|| {
            JcimError::Unsupported(
                "HOME is not set, so JCIM cannot resolve its managed application-data directory"
                    .to_string(),
            )
        })?;
        return Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("jcim"));
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(path) = env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(path).join("jcim"));
        }
        let home = env::var_os("HOME").ok_or_else(|| {
            JcimError::Unsupported(
                "HOME is not set, so JCIM cannot resolve its managed application-data directory"
                    .to_string(),
            )
        })?;
        return Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("jcim"));
    }

    #[allow(unreachable_code)]
    Err(JcimError::Unsupported(
        "JCIM currently supports macOS and Linux only".to_string(),
    ))
}
