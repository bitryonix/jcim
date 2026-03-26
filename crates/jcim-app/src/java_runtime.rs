//! Bundled JVM resolution for managed simulator and helper workflows.
#![allow(clippy::missing_docs_in_private_items)]
// Internal helper module for runtime resolution and extraction. The public-facing behavior is
// documented on the app/service surface; keeping the private helper layer lightly documented here
// preserves signal without forcing rustdoc boilerplate onto every extraction helper.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use jcim_core::error::{JcimError, Result};
use sha2::{Digest, Sha256};

const BUNDLED_RUNTIME_VERSION: &str = "temurin-11.0.30+7";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JavaRuntimeSource {
    Bundled,
    Configured,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedJavaRuntime {
    pub(crate) java_bin: PathBuf,
    pub(crate) source: JavaRuntimeSource,
    pub(crate) label: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BundledJavaArchive {
    os: &'static str,
    arch: &'static str,
    archive_name: &'static str,
    sha256: &'static str,
}

const SUPPORTED_ARCHIVES: &[BundledJavaArchive] = &[
    BundledJavaArchive {
        os: "macos",
        arch: "aarch64",
        archive_name: "OpenJDK11U-jre_aarch64_mac_hotspot_11.0.30_7.tar.gz",
        sha256: "e6bd2ae0053d5768897d2a53e10236bba26bdbce77fab9bf06bfc6a866bf3009",
    },
    BundledJavaArchive {
        os: "macos",
        arch: "x86_64",
        archive_name: "OpenJDK11U-jre_x64_mac_hotspot_11.0.30_7.tar.gz",
        sha256: "fa444f334f2702806370766678c94841a95955f211eed35dec8447e4c33496d1",
    },
    BundledJavaArchive {
        os: "linux",
        arch: "x86_64",
        archive_name: "OpenJDK11U-jre_x64_linux_hotspot_11.0.30_7.tar.gz",
        sha256: "d851e43d81ec6ff7f28efe28c42b4787a045e8f59cdcd6434dece98d8342eb8a",
    },
    BundledJavaArchive {
        os: "linux",
        arch: "aarch64",
        archive_name: "OpenJDK11U-jre_aarch64_linux_hotspot_11.0.30_7.tar.gz",
        sha256: "9d6a8d3a33c308bbc7332e4c2e2f9a94fbbc56417863496061ef6defef9c5391",
    },
];

pub(crate) fn resolve_java_runtime(
    managed_bundle_root: &Path,
    configured_java_bin: &str,
) -> Result<ResolvedJavaRuntime> {
    if let Some(archive) = bundled_runtime_for(std::env::consts::OS, std::env::consts::ARCH) {
        let java_bin = ensure_bundled_runtime(
            archive,
            &bundled_runtime_archive_root(),
            managed_bundle_root,
        )?;
        return Ok(ResolvedJavaRuntime {
            java_bin,
            source: JavaRuntimeSource::Bundled,
            label: format!("bundled {BUNDLED_RUNTIME_VERSION}"),
        });
    }

    Ok(ResolvedJavaRuntime {
        java_bin: PathBuf::from(configured_java_bin),
        source: JavaRuntimeSource::Configured,
        label: format!("configured {}", configured_java_bin.trim()),
    })
}

fn bundled_runtime_for(os: &str, arch: &str) -> Option<BundledJavaArchive> {
    SUPPORTED_ARCHIVES
        .iter()
        .copied()
        .find(|archive| archive.os == os && archive.arch == arch)
}

fn bundled_runtime_archive_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/java-runtimes")
        .join(BUNDLED_RUNTIME_VERSION)
}

fn ensure_bundled_runtime(
    archive: BundledJavaArchive,
    archive_root: &Path,
    managed_bundle_root: &Path,
) -> Result<PathBuf> {
    let install_root = managed_bundle_root.join("java").join(format!(
        "{BUNDLED_RUNTIME_VERSION}-{}-{}",
        archive.os, archive.arch
    ));
    if let Some(java_bin) = find_java_bin(&install_root) {
        return Ok(java_bin);
    }

    let archive_path = archive_root.join(archive.archive_name);
    if !archive_path.exists() {
        return Err(JcimError::Unsupported(format!(
            "bundled Java runtime archive is missing: {}",
            archive_path.display()
        )));
    }
    verify_archive_checksum(&archive_path, archive.sha256)?;

    if let Some(parent) = install_root.parent() {
        fs::create_dir_all(parent)?;
    }
    if install_root.exists() {
        fs::remove_dir_all(&install_root)?;
    }

    let staging_root = managed_bundle_root.join("java").join(format!(
        ".extract-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&staging_root)?;
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&staging_root)
        .status()
        .map_err(|error| {
            JcimError::Unsupported(format!(
                "unable to extract bundled Java runtime {}: {error}",
                archive_path.display()
            ))
        })?;
    if !status.success() {
        let _ = fs::remove_dir_all(&staging_root);
        return Err(JcimError::Unsupported(format!(
            "extracting bundled Java runtime failed for {}",
            archive_path.display()
        )));
    }

    let extracted_root = extracted_runtime_root(&staging_root)?;
    fs::rename(&extracted_root, &install_root).map_err(|error| {
        JcimError::Unsupported(format!(
            "unable to finalize bundled Java runtime at {}: {error}",
            install_root.display()
        ))
    })?;
    if staging_root.exists() {
        let _ = fs::remove_dir_all(&staging_root);
    }

    find_java_bin(&install_root).ok_or_else(|| {
        JcimError::Unsupported(format!(
            "bundled Java runtime did not expose a java executable under {}",
            install_root.display()
        ))
    })
}

fn verify_archive_checksum(archive_path: &Path, expected_hex: &str) -> Result<()> {
    let digest = Sha256::digest(&fs::read(archive_path)?);
    let actual_hex = hex::encode(digest);
    if actual_hex != expected_hex {
        return Err(JcimError::Unsupported(format!(
            "bundled Java runtime checksum mismatch for {}",
            archive_path.display()
        )));
    }
    Ok(())
}

fn extracted_runtime_root(staging_root: &Path) -> Result<PathBuf> {
    let mut entries = fs::read_dir(staging_root)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    if entries.len() == 1 && entries[0].is_dir() {
        return Ok(entries.remove(0));
    }
    Ok(staging_root.to_path_buf())
}

fn find_java_bin(root: &Path) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = fs::read_dir(&current).ok()?;
        for entry in entries.filter_map(|entry| entry.ok()) {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()) == Some("java")
                && path
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    == Some("bin")
            {
                return Some(path);
            }
        }
    }
    None
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::{BundledJavaArchive, bundled_runtime_for, ensure_bundled_runtime};
    use jcim_core::error::Result;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("jcim-java-runtime-{label}-{unique}"))
    }

    #[test]
    fn bundle_matrix_covers_supported_hosts() {
        assert!(bundled_runtime_for("macos", "aarch64").is_some());
        assert!(bundled_runtime_for("macos", "x86_64").is_some());
        assert!(bundled_runtime_for("linux", "x86_64").is_some());
        assert!(bundled_runtime_for("linux", "aarch64").is_some());
        assert!(bundled_runtime_for("windows", "x86_64").is_none());
    }

    #[test]
    fn ensure_bundled_runtime_extracts_fake_archive() -> Result<()> {
        let archive_root = temp_dir("archive-root");
        let managed_root = temp_dir("managed-root");
        let payload_root = temp_dir("payload-root");
        let runtime_dir = payload_root.join("fake-jre");
        fs::create_dir_all(runtime_dir.join("bin"))?;
        fs::write(runtime_dir.join("bin/java"), b"#!/bin/sh\nexit 0\n")?;

        fs::create_dir_all(&archive_root)?;
        fs::create_dir_all(&managed_root)?;
        let archive_path = archive_root.join("fake-runtime.tar.gz");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&payload_root)
            .arg("fake-jre")
            .status()
            .expect("tar");
        assert!(status.success());
        let checksum = hex::encode(Sha256::digest(fs::read(&archive_path)?));
        let checksum = Box::leak(checksum.into_boxed_str());
        let archive_name = Box::leak("fake-runtime.tar.gz".to_string().into_boxed_str());
        let asset = BundledJavaArchive {
            os: "macos",
            arch: "aarch64",
            archive_name,
            sha256: checksum,
        };

        let java_bin = ensure_bundled_runtime(asset, &archive_root, &managed_root)?;
        assert!(java_bin.ends_with("bin/java"));
        assert!(java_bin.exists());

        let _ = fs::remove_dir_all(&archive_root);
        let _ = fs::remove_dir_all(&managed_root);
        let _ = fs::remove_dir_all(&payload_root);
        Ok(())
    }
}
