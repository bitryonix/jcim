//! Project and machine-local path helpers.

use crate::managed_files::write_regular_file_atomic;
use jcim_core::error::{JcimError, Result};
use std::env;
use std::path::{Path, PathBuf};

/// Canonical manifest file name.
pub const PROJECT_MANIFEST_NAME: &str = "jcim.toml";
/// Machine-local user config file name.
pub const USER_CONFIG_FILE_NAME: &str = "config.toml";
/// Machine-local project registry file name.
pub const PROJECT_REGISTRY_FILE_NAME: &str = "projects.toml";
/// Managed subdirectory that stores bundled runtime assets.
pub const MANAGED_BUNDLES_DIR_NAME: &str = "bundled";
/// Runtime metadata file name used to track one local daemon instance.
pub const SERVICE_RUNTIME_FILE_NAME: &str = "jcimd.runtime.toml";

/// Machine-local JCIM directories discovered for the current platform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedPaths {
    /// Primary managed data root used for extracted runtime assets and compatibility migration.
    pub root: PathBuf,
    /// Managed configuration directory.
    pub config_dir: PathBuf,
    /// Managed durable data directory.
    pub data_dir: PathBuf,
    /// Managed durable state directory.
    pub state_dir: PathBuf,
    /// Managed runtime directory for the local socket and ephemeral state.
    pub runtime_dir: PathBuf,
    /// Managed cache directory.
    pub cache_dir: PathBuf,
    /// User config file path.
    pub config_path: PathBuf,
    /// Known-project registry path.
    pub registry_path: PathBuf,
    /// Managed local service socket path.
    pub service_socket_path: PathBuf,
    /// Runtime metadata path for the managed daemon socket.
    pub runtime_metadata_path: PathBuf,
    /// Managed log directory.
    pub log_dir: PathBuf,
    /// Managed bundled-asset root.
    pub bundle_root: PathBuf,
    /// Legacy one-root location used for compatibility migration of older installs.
    legacy_root: PathBuf,
}

impl ManagedPaths {
    /// Discover machine-local JCIM paths.
    pub fn discover() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            return discover_macos_paths();
        }

        #[cfg(target_os = "linux")]
        {
            return discover_linux_paths();
        }

        #[allow(unreachable_code)]
        Err(JcimError::Unsupported(
            "JCIM currently supports macOS and Linux only".to_string(),
        ))
    }

    /// Build managed JCIM paths under one explicit root.
    pub fn for_root(root: PathBuf) -> Self {
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let state_dir = root.join("state");
        let runtime_dir = root.join("run");
        let cache_dir = root.join("cache");
        let log_dir = root.join("logs");
        let bundle_root = data_dir.join(MANAGED_BUNDLES_DIR_NAME);
        Self {
            config_path: config_dir.join(USER_CONFIG_FILE_NAME),
            registry_path: state_dir.join(PROJECT_REGISTRY_FILE_NAME),
            service_socket_path: runtime_dir.join("jcimd.sock"),
            runtime_metadata_path: runtime_dir.join(SERVICE_RUNTIME_FILE_NAME),
            config_dir,
            data_dir,
            state_dir,
            runtime_dir,
            cache_dir,
            log_dir,
            bundle_root,
            legacy_root: root.clone(),
            root,
        }
    }

    /// Build managed JCIM paths for one explicit HOME/XDG environment without mutating process env.
    pub fn for_env_roots(
        home: PathBuf,
        _xdg_config_home: Option<PathBuf>,
        _xdg_data_home: Option<PathBuf>,
        _xdg_state_home: Option<PathBuf>,
        _xdg_cache_home: Option<PathBuf>,
        _xdg_runtime_dir: Option<PathBuf>,
    ) -> Self {
        #[cfg(target_os = "macos")]
        {
            let root = home
                .join("Library")
                .join("Application Support")
                .join("jcim");
            let config_dir = root.join("config");
            let data_dir = root.join("data");
            let state_dir = root.join("state");
            let runtime_dir = root.join("run");
            let cache_dir = home.join("Library").join("Caches").join("jcim");
            let log_dir = home.join("Library").join("Logs").join("jcim");
            let bundle_root = data_dir.join(MANAGED_BUNDLES_DIR_NAME);

            return Self {
                root: root.clone(),
                config_dir: config_dir.clone(),
                data_dir: data_dir.clone(),
                state_dir: state_dir.clone(),
                runtime_dir: runtime_dir.clone(),
                cache_dir,
                config_path: config_dir.join(USER_CONFIG_FILE_NAME),
                registry_path: state_dir.join(PROJECT_REGISTRY_FILE_NAME),
                service_socket_path: runtime_dir.join("jcimd.sock"),
                runtime_metadata_path: runtime_dir.join(SERVICE_RUNTIME_FILE_NAME),
                log_dir,
                bundle_root,
                legacy_root: root,
            };
        }

        #[cfg(target_os = "linux")]
        {
            let config_home = _xdg_config_home.unwrap_or_else(|| home.join(".config"));
            let data_home = _xdg_data_home.unwrap_or_else(|| home.join(".local").join("share"));
            let state_home = _xdg_state_home.unwrap_or_else(|| home.join(".local").join("state"));
            let cache_home = _xdg_cache_home.unwrap_or_else(|| home.join(".cache"));
            let root = data_home.join("jcim");
            let config_dir = config_home.join("jcim");
            let data_dir = root.clone();
            let state_dir = state_home.join("jcim");
            let runtime_dir = _xdg_runtime_dir
                .map(|path| path.join("jcim"))
                .unwrap_or_else(|| state_dir.join("run"));
            let cache_dir = cache_home.join("jcim");
            let log_dir = state_dir.join("logs");
            let bundle_root = data_dir.join(MANAGED_BUNDLES_DIR_NAME);

            return Self {
                root: root.clone(),
                config_dir: config_dir.clone(),
                data_dir: data_dir.clone(),
                state_dir: state_dir.clone(),
                runtime_dir: runtime_dir.clone(),
                cache_dir,
                config_path: config_dir.join(USER_CONFIG_FILE_NAME),
                registry_path: state_dir.join(PROJECT_REGISTRY_FILE_NAME),
                service_socket_path: runtime_dir.join("jcimd.sock"),
                runtime_metadata_path: runtime_dir.join(SERVICE_RUNTIME_FILE_NAME),
                log_dir,
                bundle_root,
                legacy_root: root,
            };
        }

        #[allow(unreachable_code)]
        Self::for_root(home.join("jcim"))
    }

    /// Prepare the managed layout and migrate legacy config and registry files when needed.
    pub fn prepare_layout(&self) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.state_dir)?;
        std::fs::create_dir_all(&self.runtime_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::create_dir_all(&self.log_dir)?;
        std::fs::create_dir_all(&self.bundle_root)?;

        migrate_legacy_file_if_missing(&self.legacy_config_path(), &self.config_path)?;
        migrate_legacy_file_if_missing(&self.legacy_registry_path(), &self.registry_path)?;
        Ok(())
    }

    /// Return the legacy one-root path that older JCIM versions wrote into.
    pub fn legacy_root(&self) -> &Path {
        &self.legacy_root
    }

    /// Return the pre-0.3 legacy config file path under the one-root layout.
    fn legacy_config_path(&self) -> PathBuf {
        self.legacy_root.join(USER_CONFIG_FILE_NAME)
    }

    /// Return the pre-0.3 legacy registry file path under the one-root layout.
    fn legacy_registry_path(&self) -> PathBuf {
        self.legacy_root.join(PROJECT_REGISTRY_FILE_NAME)
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

#[cfg(target_os = "macos")]
/// Discover the managed JCIM path layout for macOS hosts.
fn discover_macos_paths() -> Result<ManagedPaths> {
    let home = required_home("HOME is not set, so JCIM cannot resolve its managed directories")?;
    let root = home
        .join("Library")
        .join("Application Support")
        .join("jcim");
    let config_dir = root.join("config");
    let data_dir = root.join("data");
    let state_dir = root.join("state");
    let runtime_dir = root.join("run");
    let cache_dir = home.join("Library").join("Caches").join("jcim");
    let log_dir = home.join("Library").join("Logs").join("jcim");
    let bundle_root = data_dir.join(MANAGED_BUNDLES_DIR_NAME);

    Ok(ManagedPaths {
        root: root.clone(),
        config_dir: config_dir.clone(),
        data_dir: data_dir.clone(),
        state_dir: state_dir.clone(),
        runtime_dir: runtime_dir.clone(),
        cache_dir,
        config_path: config_dir.join(USER_CONFIG_FILE_NAME),
        registry_path: state_dir.join(PROJECT_REGISTRY_FILE_NAME),
        service_socket_path: runtime_dir.join("jcimd.sock"),
        runtime_metadata_path: runtime_dir.join(SERVICE_RUNTIME_FILE_NAME),
        log_dir,
        bundle_root,
        legacy_root: root,
    })
}

#[cfg(target_os = "linux")]
/// Discover the managed JCIM path layout for Linux hosts and XDG conventions.
fn discover_linux_paths() -> Result<ManagedPaths> {
    let home = required_home("HOME is not set, so JCIM cannot resolve its managed directories")?;
    let config_home = env_path_or_default("XDG_CONFIG_HOME", home.join(".config"));
    let data_home = env_path_or_default("XDG_DATA_HOME", home.join(".local").join("share"));
    let state_home = env_path_or_default("XDG_STATE_HOME", home.join(".local").join("state"));
    let cache_home = env_path_or_default("XDG_CACHE_HOME", home.join(".cache"));
    let root = data_home.join("jcim");
    let config_dir = config_home.join("jcim");
    let data_dir = root.clone();
    let state_dir = state_home.join("jcim");
    let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .map(|path| path.join("jcim"))
        .unwrap_or_else(|| state_dir.join("run"));
    let cache_dir = cache_home.join("jcim");
    let log_dir = state_dir.join("logs");
    let bundle_root = data_dir.join(MANAGED_BUNDLES_DIR_NAME);

    Ok(ManagedPaths {
        root: root.clone(),
        config_dir: config_dir.clone(),
        data_dir: data_dir.clone(),
        state_dir: state_dir.clone(),
        runtime_dir: runtime_dir.clone(),
        cache_dir,
        config_path: config_dir.join(USER_CONFIG_FILE_NAME),
        registry_path: state_dir.join(PROJECT_REGISTRY_FILE_NAME),
        service_socket_path: runtime_dir.join("jcimd.sock"),
        runtime_metadata_path: runtime_dir.join(SERVICE_RUNTIME_FILE_NAME),
        log_dir,
        bundle_root,
        legacy_root: root,
    })
}

/// Return the current `HOME` path or surface one managed-path discovery error.
fn required_home(message: &str) -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| JcimError::Unsupported(message.to_string()))
}

#[cfg(target_os = "linux")]
/// Return one Linux XDG path override or its default fallback path.
fn env_path_or_default(var: &str, default: PathBuf) -> PathBuf {
    env::var_os(var).map(PathBuf::from).unwrap_or(default)
}

/// Copy one legacy config or registry file into the managed layout when the destination is still absent.
fn migrate_legacy_file_if_missing(source: &Path, destination: &Path) -> Result<()> {
    if source == destination || destination.exists() || !source.exists() {
        return Ok(());
    }

    let contents = std::fs::read(source)?;
    write_regular_file_atomic(destination, &contents, "managed migration target")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ManagedPaths, PROJECT_REGISTRY_FILE_NAME, USER_CONFIG_FILE_NAME};

    #[test]
    fn for_root_splits_paths_into_separate_directories() {
        let paths = ManagedPaths::for_root(PathBuf::from("/tmp/jcim-layout"));
        assert_eq!(paths.config_dir, PathBuf::from("/tmp/jcim-layout/config"));
        assert_eq!(paths.data_dir, PathBuf::from("/tmp/jcim-layout/data"));
        assert_eq!(paths.state_dir, PathBuf::from("/tmp/jcim-layout/state"));
        assert_eq!(paths.runtime_dir, PathBuf::from("/tmp/jcim-layout/run"));
        assert_eq!(paths.cache_dir, PathBuf::from("/tmp/jcim-layout/cache"));
        assert_eq!(
            paths.config_path,
            paths.config_dir.join(USER_CONFIG_FILE_NAME)
        );
        assert_eq!(
            paths.registry_path,
            paths.state_dir.join(PROJECT_REGISTRY_FILE_NAME)
        );
        assert_eq!(
            paths.runtime_metadata_path,
            paths.runtime_dir.join("jcimd.runtime.toml")
        );
    }

    #[test]
    fn prepare_layout_migrates_legacy_config_and_registry_once() {
        let root = unique_root("prepare-layout");
        std::fs::create_dir_all(&root).expect("create legacy root");
        std::fs::write(root.join(USER_CONFIG_FILE_NAME), "java_bin = \"java17\"\n")
            .expect("write legacy config");
        std::fs::write(
            root.join(PROJECT_REGISTRY_FILE_NAME),
            "[[projects]]\nproject_id = \"demo\"\nproject_path = \"/tmp/demo\"\n",
        )
        .expect("write legacy registry");

        let paths = ManagedPaths::for_root(root.clone());
        paths.prepare_layout().expect("prepare layout");
        paths.prepare_layout().expect("prepare layout again");

        assert_eq!(
            std::fs::read_to_string(&paths.config_path).expect("new config"),
            "java_bin = \"java17\"\n"
        );
        assert!(
            std::fs::read_to_string(&paths.registry_path)
                .expect("new registry")
                .contains("project_id = \"demo\"")
        );
        assert!(root.join(USER_CONFIG_FILE_NAME).exists());
        assert!(root.join(PROJECT_REGISTRY_FILE_NAME).exists());

        let _ = std::fs::remove_dir_all(root);
    }

    fn unique_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from("/tmp").join(format!("jcim-config-{label}-{unique:x}"))
    }
}
