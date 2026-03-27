//! Supply-chain manifest checks for shipped third-party and bundled binaries.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};
use toml::Value;

#[test]
fn third_party_manifest_tracks_shipped_binary_checksums() {
    let manifest_path = repo_root().join("third_party/THIRD_PARTY.toml");
    let manifest = std::fs::read_to_string(&manifest_path).expect("read third-party manifest");
    let parsed: Value = toml::from_str(&manifest).expect("parse third-party manifest");
    let entries = parsed.as_table().expect("top-level third-party table");

    for required in [
        "gppro",
        "ecj",
        "ant_javacard",
        "jcardsim",
        "jcim_card_helper",
        "jcim_simulator_backend",
    ] {
        assert!(
            entries.contains_key(required),
            "third-party manifest is missing the `{required}` entry"
        );
    }

    for (entry_name, entry) in entries {
        let entry = entry
            .as_table()
            .unwrap_or_else(|| panic!("entry `{entry_name}` must be a table"));
        for field in [
            "name",
            "version",
            "artifact",
            "license",
            "upstream",
            "update_cadence",
            "notes",
        ] {
            assert!(
                entry
                    .get(field)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty()),
                "entry `{entry_name}` is missing a non-empty `{field}` field"
            );
        }

        let artifact_path = repo_root().join(
            entry["artifact"]
                .as_str()
                .unwrap_or_else(|| panic!("entry `{entry_name}` has a non-string artifact")),
        );
        assert!(
            artifact_path.exists(),
            "entry `{entry_name}` points to a missing artifact {}",
            artifact_path.display()
        );

        if artifact_path.is_file() {
            let expected = entry
                .get("sha256")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("file artifact `{entry_name}` is missing `sha256`"));
            let actual = sha256_file(&artifact_path);
            assert_eq!(
                actual,
                expected,
                "checksum mismatch for `{entry_name}` at {}",
                artifact_path.display()
            );
        }
    }
}

#[test]
fn bundled_runtime_archives_match_documented_checksums() {
    let documented = bundled_runtime_checksums();
    assert_eq!(
        documented.len(),
        4,
        "expected four documented bundled runtime checksums"
    );
    for (artifact, expected) in documented {
        let path = repo_root()
            .join("third_party/java-runtimes/temurin-11.0.30+7")
            .join(artifact);
        assert!(
            path.exists(),
            "missing bundled runtime archive {}",
            path.display()
        );
        assert_eq!(
            sha256_file(&path),
            expected,
            "checksum mismatch for {}",
            path.display()
        );
    }
}

#[test]
fn tracked_shipped_artifacts_are_manifest_covered() {
    let manifest = third_party_manifest();
    let artifacts = manifest_artifacts(&manifest);
    let tracked = tracked_files(["third_party", "bundled-backends"]);

    let uncovered: Vec<String> = tracked
        .iter()
        .filter(|path| is_shipped_artifact(path))
        .filter(|path| !artifacts.iter().any(|artifact| artifact.covers(path)))
        .map(|path| path.display().to_string())
        .collect();

    assert!(
        uncovered.is_empty(),
        "tracked shipped artifacts must be covered by third_party/THIRD_PARTY.toml: {uncovered:#?}"
    );
}

#[test]
fn tracked_asset_trees_do_not_contain_os_or_editor_cruft() {
    let tracked = tracked_files(["third_party", "bundled-backends"]);
    let cruft: Vec<String> = tracked
        .iter()
        .filter(|path| is_tracked_cruft(path))
        .map(|path| path.display().to_string())
        .collect();

    assert!(
        cruft.is_empty(),
        "tracked asset trees must not contain OS/editor cruft: {cruft:#?}"
    );
}

#[derive(Debug)]
struct ManifestArtifact {
    path: PathBuf,
    directory: bool,
}

impl ManifestArtifact {
    fn covers(&self, path: &Path) -> bool {
        path == self.path || self.directory && path.starts_with(&self.path)
    }
}

fn bundled_runtime_checksums() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        (
            "OpenJDK11U-jre_aarch64_linux_hotspot_11.0.30_7.tar.gz",
            "9d6a8d3a33c308bbc7332e4c2e2f9a94fbbc56417863496061ef6defef9c5391",
        ),
        (
            "OpenJDK11U-jre_aarch64_mac_hotspot_11.0.30_7.tar.gz",
            "e6bd2ae0053d5768897d2a53e10236bba26bdbce77fab9bf06bfc6a866bf3009",
        ),
        (
            "OpenJDK11U-jre_x64_linux_hotspot_11.0.30_7.tar.gz",
            "d851e43d81ec6ff7f28efe28c42b4787a045e8f59cdcd6434dece98d8342eb8a",
        ),
        (
            "OpenJDK11U-jre_x64_mac_hotspot_11.0.30_7.tar.gz",
            "fa444f334f2702806370766678c94841a95955f211eed35dec8447e4c33496d1",
        ),
    ])
}

fn third_party_manifest() -> Value {
    let manifest_path = repo_root().join("third_party/THIRD_PARTY.toml");
    let manifest = std::fs::read_to_string(&manifest_path).expect("read third-party manifest");
    toml::from_str(&manifest).expect("parse third-party manifest")
}

fn manifest_artifacts(manifest: &Value) -> Vec<ManifestArtifact> {
    manifest
        .as_table()
        .expect("top-level third-party table")
        .iter()
        .map(|(entry_name, entry)| {
            let entry = entry
                .as_table()
                .unwrap_or_else(|| panic!("entry `{entry_name}` must be a table"));
            let path = PathBuf::from(
                entry["artifact"]
                    .as_str()
                    .unwrap_or_else(|| panic!("entry `{entry_name}` has a non-string artifact")),
            );
            let full_path = repo_root().join(&path);
            ManifestArtifact {
                path,
                directory: full_path.is_dir(),
            }
        })
        .collect()
}

fn tracked_files<const N: usize>(paths: [&str; N]) -> Vec<PathBuf> {
    let output = Command::new("git")
        .args(["ls-files", "--"])
        .args(paths)
        .current_dir(repo_root())
        .output()
        .expect("run `git ls-files` for governed asset trees");
    assert!(
        output.status.success(),
        "`git ls-files` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("tracked file list must be valid UTF-8")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

fn is_shipped_artifact(path: &Path) -> bool {
    let display = path.display().to_string();
    display.ends_with(".jar")
        || display.ends_with(".tar.gz")
        || display.ends_with(".tgz")
        || display.ends_with(".zip")
        || display.ends_with(".exe")
        || display.ends_with(".dll")
        || display.ends_with(".dylib")
        || display.ends_with(".so")
}

fn is_tracked_cruft(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    file_name == ".DS_Store"
        || file_name == "Thumbs.db"
        || file_name == "Desktop.ini"
        || file_name.ends_with('~')
        || matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("swp" | "swo")
        )
        || path
            .components()
            .any(|component| component.as_os_str() == "__MACOSX")
}

fn sha256_file(path: &Path) -> String {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|error| panic!("failed to read {} for hashing: {error}", path.display()));
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}
