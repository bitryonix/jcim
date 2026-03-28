//! Atomic managed-file persistence helpers.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_core::error::{JcimError, Result};

/// Atomically write one managed regular file using a temp file, `sync_all`, and rename.
pub fn write_regular_file_atomic(path: &Path, contents: &[u8], description: &str) -> Result<()> {
    validate_regular_file_destination(path, description)?;
    let Some(parent) = path.parent() else {
        return Err(JcimError::Unsupported(format!(
            "{description} path has no parent directory: {}",
            path.display()
        )));
    };
    std::fs::create_dir_all(parent)?;

    let temp_path = path.with_extension(format!("tmp-{}", unique_suffix()));
    let mut temp_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)?;
    if let Err(error) = temp_file
        .write_all(contents)
        .and_then(|_| temp_file.sync_all())
    {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error.into());
    }
    drop(temp_file);

    if let Err(error) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error.into());
    }

    sync_directory(parent)?;
    Ok(())
}

/// Fail closed unless the destination is absent or already a regular file.
fn validate_regular_file_destination(path: &Path, description: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = std::fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(JcimError::Unsupported(format!(
            "refusing to overwrite symlinked {description} path {}",
            path.display()
        )));
    }
    if !file_type.is_file() {
        return Err(JcimError::Unsupported(format!(
            "refusing to overwrite non-file {description} path {}",
            path.display()
        )));
    }
    Ok(())
}

/// Sync the containing directory so the rename is durable across crashes.
fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

/// Build a coarse unique suffix for the temp file path.
fn unique_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::write_regular_file_atomic;

    #[test]
    fn atomic_writer_creates_and_replaces_regular_files() {
        let root = temp_root("replace");
        let path = root.join("nested/config.toml");

        write_regular_file_atomic(&path, b"version = 1\n", "test file").expect("initial write");
        write_regular_file_atomic(&path, b"version = 2\n", "test file").expect("replace write");

        assert_eq!(
            std::fs::read_to_string(&path).expect("read replaced file"),
            "version = 2\n"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn atomic_writer_rejects_symlink_destinations() {
        let root = temp_root("symlink");
        let target = root.join("target.toml");
        let path = root.join("config.toml");
        std::fs::create_dir_all(&root).expect("create root");
        std::fs::write(&target, "target").expect("write target");
        std::os::unix::fs::symlink(&target, &path).expect("create symlink");

        let error = write_regular_file_atomic(&path, b"version = 1\n", "test file")
            .expect_err("symlink should fail closed");
        assert!(
            error
                .to_string()
                .contains("refusing to overwrite symlinked")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn atomic_writer_rejects_non_file_destinations() {
        let root = temp_root("directory");
        let path = root.join("config.toml");
        std::fs::create_dir_all(&path).expect("create directory at destination");

        let error = write_regular_file_atomic(&path, b"version = 1\n", "test file")
            .expect_err("directory should fail closed");
        assert!(error.to_string().contains("refusing to overwrite non-file"));

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from("/tmp").join(format!("jcim-config-atomic-{label}-{unique:x}"))
    }
}
