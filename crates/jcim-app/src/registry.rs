//! Machine-local project registry helpers.

use std::path::{Path, PathBuf};

use jcim_config::managed_files::write_regular_file_atomic;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use jcim_core::error::{JcimError, Result};

/// Persisted project registry stored under the managed JCIM root.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct ProjectRegistry {
    /// Registered project entries.
    #[serde(default)]
    pub(crate) projects: Vec<RegisteredProject>,
}

/// One registered project record.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct RegisteredProject {
    /// Stable project id derived from the canonical path.
    pub(crate) project_id: String,
    /// Absolute project root path.
    pub(crate) project_path: PathBuf,
}

impl ProjectRegistry {
    /// Load the registry from disk or return an empty one when the file does not exist.
    pub(crate) fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// Persist the registry to disk.
    pub(crate) fn save_to_path(&self, path: &Path) -> Result<()> {
        let encoded = toml::to_string_pretty(self).map_err(|error| {
            JcimError::Unsupported(format!("unable to encode project registry: {error}"))
        })?;
        write_regular_file_atomic(path, encoded.as_bytes(), "project registry")
    }

    /// Upsert one project root and return its stable registry record.
    pub(crate) fn upsert(&mut self, project_root: &Path) -> Result<RegisteredProject> {
        let normalized = normalize_project_root(project_root)?;
        let project_id = project_id_for_path(&normalized);
        if let Some(existing) = self
            .projects
            .iter_mut()
            .find(|project| project.project_id == project_id || project.project_path == normalized)
        {
            existing.project_id = project_id.clone();
            existing.project_path = normalized.clone();
            return Ok(existing.clone());
        }
        let record = RegisteredProject {
            project_id,
            project_path: normalized,
        };
        self.projects.push(record.clone());
        self.projects
            .sort_by(|left, right| left.project_path.cmp(&right.project_path));
        Ok(record)
    }

    /// Resolve one registered project by id.
    pub(crate) fn by_id(&self, project_id: &str) -> Option<&RegisteredProject> {
        self.projects
            .iter()
            .find(|project| project.project_id == project_id)
    }
}

/// Normalize one project root path into a stable absolute representation.
pub(crate) fn normalize_project_root(project_root: &Path) -> Result<PathBuf> {
    if !project_root.exists() {
        return Err(JcimError::Unsupported(format!(
            "project path does not exist: {}",
            project_root.display()
        )));
    }
    Ok(project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf()))
}

/// Derive a stable project id from the canonical project root path.
pub(crate) fn project_id_for_path(project_root: &Path) -> String {
    let mut hasher = Sha1::new();
    hasher.update(project_root.display().to_string().as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..6])
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ProjectRegistry, RegisteredProject};

    #[test]
    fn registry_save_replaces_existing_contents() {
        let root = temp_root("replace");
        let path = root.join("projects.toml");
        let first = ProjectRegistry {
            projects: vec![RegisteredProject {
                project_id: "first".to_string(),
                project_path: PathBuf::from("/tmp/first"),
            }],
        };
        let second = ProjectRegistry {
            projects: vec![RegisteredProject {
                project_id: "second".to_string(),
                project_path: PathBuf::from("/tmp/second"),
            }],
        };

        first.save_to_path(&path).expect("save first registry");
        second.save_to_path(&path).expect("replace registry");

        let saved = ProjectRegistry::load_or_default(&path).expect("load registry");
        assert_eq!(saved, second);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn registry_save_rejects_symlink_destinations() {
        let root = temp_root("symlink");
        let target = root.join("target.toml");
        let path = root.join("projects.toml");
        std::fs::create_dir_all(&root).expect("create root");
        std::fs::write(&target, "target").expect("write target");
        std::os::unix::fs::symlink(&target, &path).expect("symlink");

        let error = ProjectRegistry::default()
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
        PathBuf::from("/tmp").join(format!("jcim-registry-{label}-{unique:x}"))
    }
}
