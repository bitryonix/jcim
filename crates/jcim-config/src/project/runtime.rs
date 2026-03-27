//! Local-service runtime metadata and safe cleanup helpers.
#![allow(clippy::missing_docs_in_private_items)]

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_core::error::{JcimError, Result};
use serde::{Deserialize, Serialize};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::{FileTypeExt, MetadataExt};

/// Runtime metadata persisted next to one managed daemon socket.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ServiceRuntimeRecord {
    /// On-disk format version for the runtime metadata record.
    #[serde(default = "current_runtime_record_format_version")]
    pub format_version: u32,
    /// Process id that owns the current daemon instance.
    pub pid: u32,
    /// Socket path the daemon bound during startup.
    pub socket_path: PathBuf,
    /// Path to the `jcimd` binary that created the runtime record.
    pub service_binary_path: PathBuf,
    /// Startup-captured fingerprint of the daemon binary.
    pub service_binary_fingerprint: String,
}

impl ServiceRuntimeRecord {
    /// Load one runtime record when present.
    pub fn load_if_present(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let record: Self = toml::from_str(&std::fs::read_to_string(path)?)?;
        if record.format_version > current_runtime_record_format_version() {
            return Err(JcimError::Unsupported(format!(
                "unsupported local-service runtime metadata format {} at {}; expected <= {}",
                record.format_version,
                path.display(),
                current_runtime_record_format_version()
            )));
        }
        Ok(Some(record))
    }

    /// Persist one runtime record using an atomic temp-file-plus-rename write.
    pub fn write_to_path(&self, path: &Path) -> Result<()> {
        validate_runtime_file_destination(path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp_path = path.with_extension(format!("tmp-{}", unique_suffix()));
        let encoded = toml::to_string_pretty(self).map_err(|error| {
            JcimError::Unsupported(format!(
                "unable to encode local-service runtime record: {error}"
            ))
        })?;
        std::fs::write(&temp_path, encoded)?;
        std::fs::rename(&temp_path, path)?;
        Ok(())
    }
}

/// Current runtime metadata file format version.
pub const fn current_runtime_record_format_version() -> u32 {
    1
}

/// Derive the runtime metadata path that corresponds to one daemon socket path.
pub fn runtime_metadata_path_for_socket(socket_path: &Path) -> PathBuf {
    let parent = socket_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let stem = socket_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("jcimd");
    parent.join(format!("{stem}.runtime.toml"))
}

/// Remove one stale managed socket after validating its type and ownership.
pub fn remove_owned_socket_if_present(socket_path: &Path, owner_dir: &Path) -> Result<()> {
    remove_owned_path_if_present(socket_path, owner_dir, RuntimePathKind::Socket)
}

/// Remove one stale runtime metadata file after validating its type and ownership.
pub fn remove_owned_runtime_file_if_present(path: &Path, owner_dir: &Path) -> Result<()> {
    remove_owned_path_if_present(path, owner_dir, RuntimePathKind::RegularFile)
}

fn validate_runtime_file_destination(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(JcimError::Unsupported(format!(
            "refusing to overwrite symlinked runtime metadata path {}",
            path.display()
        )));
    }
    if !file_type.is_file() {
        return Err(JcimError::Unsupported(format!(
            "refusing to overwrite non-file runtime metadata path {}",
            path.display()
        )));
    }
    Ok(())
}

fn remove_owned_path_if_present(
    path: &Path,
    owner_dir: &Path,
    kind: RuntimePathKind,
) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_runtime_path(path, owner_dir, &metadata, kind)?;
            std::fs::remove_file(path)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validate_runtime_path(
    path: &Path,
    owner_dir: &Path,
    metadata: &std::fs::Metadata,
    kind: RuntimePathKind,
) -> Result<()> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(JcimError::Unsupported(format!(
            "refusing to remove symlinked local-service {} at {}",
            kind.label(),
            path.display()
        )));
    }
    match kind {
        RuntimePathKind::Socket if !file_type.is_socket() => {
            return Err(JcimError::Unsupported(format!(
                "refusing to remove non-socket path at {}",
                path.display()
            )));
        }
        RuntimePathKind::RegularFile if !file_type.is_file() => {
            return Err(JcimError::Unsupported(format!(
                "refusing to remove non-file runtime metadata at {}",
                path.display()
            )));
        }
        _ => {}
    }

    let owner_metadata = std::fs::metadata(owner_dir)?;
    if metadata.uid() != owner_metadata.uid() {
        return Err(JcimError::Unsupported(format!(
            "refusing to remove local-service {} at {} because its owner does not match {}",
            kind.label(),
            path.display(),
            owner_dir.display()
        )));
    }
    Ok(())
}

fn unique_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

#[derive(Clone, Copy)]
enum RuntimePathKind {
    Socket,
    RegularFile,
}

impl RuntimePathKind {
    fn label(self) -> &'static str {
        match self {
            Self::Socket => "socket",
            Self::RegularFile => "runtime metadata file",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        ServiceRuntimeRecord, current_runtime_record_format_version,
        remove_owned_runtime_file_if_present, remove_owned_socket_if_present,
        runtime_metadata_path_for_socket,
    };

    #[test]
    fn runtime_metadata_path_uses_socket_stem() {
        assert_eq!(
            runtime_metadata_path_for_socket(Path::new("/tmp/jcimd.sock")),
            PathBuf::from("/tmp/jcimd.runtime.toml")
        );
        assert_eq!(
            runtime_metadata_path_for_socket(Path::new("/tmp/custom.sock")),
            PathBuf::from("/tmp/custom.runtime.toml")
        );
    }

    #[test]
    fn runtime_record_round_trips_through_disk() {
        let root = unique_root("runtime-record");
        std::fs::create_dir_all(&root).expect("create root");
        let path = root.join("jcimd.runtime.toml");
        let record = ServiceRuntimeRecord {
            format_version: current_runtime_record_format_version(),
            pid: 42,
            socket_path: root.join("jcimd.sock"),
            service_binary_path: PathBuf::from("/tmp/jcimd"),
            service_binary_fingerprint: "123:456:789".to_string(),
        };

        record.write_to_path(&path).expect("write runtime record");
        assert_eq!(
            ServiceRuntimeRecord::load_if_present(&path)
                .expect("load runtime record")
                .expect("runtime record"),
            record
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn remove_owned_runtime_file_rejects_symlinks() {
        let root = unique_root("symlink-metadata");
        std::fs::create_dir_all(&root).expect("create root");
        let target = root.join("target");
        std::fs::write(&target, "data").expect("write target");
        let link = root.join("jcimd.runtime.toml");
        std::os::unix::fs::symlink(&target, &link).expect("create symlink");

        let error = remove_owned_runtime_file_if_present(&link, &root).expect_err("reject symlink");
        assert!(error.to_string().contains("refusing to remove symlinked"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn remove_owned_socket_rejects_regular_files() {
        let root = unique_root("regular-socket");
        std::fs::create_dir_all(&root).expect("create root");
        let socket_path = root.join("jcimd.sock");
        std::fs::write(&socket_path, "not a socket").expect("write file");

        let error =
            remove_owned_socket_if_present(&socket_path, &root).expect_err("reject regular file");
        assert!(error.to_string().contains("refusing to remove non-socket"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_record_rejects_unknown_future_format() {
        let root = unique_root("future-runtime-record");
        std::fs::create_dir_all(&root).expect("create root");
        let path = root.join("jcimd.runtime.toml");
        std::fs::write(
            &path,
            r#"
format_version = 99
pid = 42
socket_path = "/tmp/jcimd.sock"
service_binary_path = "/tmp/jcimd"
service_binary_fingerprint = "abc"
"#,
        )
        .expect("write future runtime record");

        let error = ServiceRuntimeRecord::load_if_present(&path).expect_err("reject future format");
        assert!(
            error
                .to_string()
                .contains("unsupported local-service runtime metadata format")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn unique_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from("/tmp").join(format!("jcim-runtime-{label}-{unique:x}"))
    }
}
