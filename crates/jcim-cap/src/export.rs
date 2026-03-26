//! Import resolution support for CAP verification.

use std::collections::HashMap;
use std::path::Path;

use jcim_core::aid::Aid;
use jcim_core::error::{JcimError, Result};
use jcim_core::model::JavaCardClassicVersion;
use serde::{Deserialize, Serialize};

use crate::cap::ImportedPackage;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
/// Export metadata for a package that may satisfy CAP imports.
pub struct PackageExport {
    /// Human-readable package name.
    pub name: String,
    /// Exported package AID.
    pub aid: Aid,
    /// Exported major version.
    pub major: u8,
    /// Exported minor version.
    pub minor: u8,
    /// Earliest Java Card version that includes this export.
    pub introduced_in: JavaCardClassicVersion,
}

#[derive(Clone, Debug, Default)]
/// Registry of builtin and caller-supplied package exports.
pub struct ExportRegistry {
    /// Export entries keyed by package AID.
    packages: HashMap<Aid, PackageExport>,
}

impl ExportRegistry {
    /// Create a registry seeded with builtin exports available in the selected version.
    pub fn new_for_version(version: JavaCardClassicVersion) -> Self {
        let mut registry = Self::default();
        for package in builtin_exports()
            .into_iter()
            .filter(|package| package.introduced_in <= version)
        {
            registry.packages.insert(package.aid.clone(), package);
        }
        registry
    }

    /// Register a single export entry.
    pub fn register(&mut self, package: PackageExport) {
        self.packages.insert(package.aid.clone(), package);
    }

    /// Load an export hint from JSON, TOML, or line-based `key=value` text.
    pub fn register_hint_file(&mut self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path).map_err(|error| {
            JcimError::Unsupported(format!(
                "failed to load export hint file {}: {error}",
                path.display()
            ))
        })?;

        let package = if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            serde_json::from_str::<PackageExport>(&content)?
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            toml::from_str::<PackageExport>(&content)?
        } else {
            parse_hint_lines(&content)?
        };
        self.register(package);
        Ok(())
    }

    /// Validate that each imported package is available in the registry.
    pub fn validate_imports(&self, imports: &[ImportedPackage]) -> Result<()> {
        for imported in imports {
            if !self.packages.contains_key(&imported.aid) {
                return Err(JcimError::Unsupported(format!(
                    "imported package {} is not available in the export registry",
                    imported.aid
                )));
            }
        }
        Ok(())
    }
}

/// Parse the repository's lightweight `key=value` export-hint format.
fn parse_hint_lines(content: &str) -> Result<PackageExport> {
    let mut name = None;
    let mut aid = None;
    let mut major = None;
    let mut minor = None;

    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "name" => name = Some(value.to_string()),
            "aid" => aid = Some(Aid::from_hex(value)?),
            "major" => major = value.parse::<u8>().ok(),
            "minor" => minor = value.parse::<u8>().ok(),
            _ => {}
        }
    }

    Ok(PackageExport {
        name: name
            .ok_or_else(|| JcimError::Unsupported("export hint is missing `name`".to_string()))?,
        aid: aid
            .ok_or_else(|| JcimError::Unsupported("export hint is missing `aid`".to_string()))?,
        major: major.unwrap_or(1),
        minor: minor.unwrap_or(0),
        introduced_in: JavaCardClassicVersion::V2_1,
    })
}

/// Return the builtin export set shipped with JCIM.
fn builtin_exports() -> Vec<PackageExport> {
    [
        (
            "javacard.framework",
            "A0000000620001",
            1,
            0,
            JavaCardClassicVersion::V2_1,
        ),
        (
            "javacard.security",
            "A0000000620101",
            1,
            0,
            JavaCardClassicVersion::V2_1,
        ),
        (
            "javacard.security",
            "A0000000620102",
            1,
            0,
            JavaCardClassicVersion::V2_1,
        ),
        (
            "javacardx.crypto",
            "A0000000620201",
            1,
            0,
            JavaCardClassicVersion::V2_2,
        ),
        (
            "javacardx.framework.util",
            "A0000000620002",
            1,
            0,
            JavaCardClassicVersion::V2_2,
        ),
    ]
    .into_iter()
    .map(|(name, aid, major, minor, introduced_in)| PackageExport {
        name: name.to_string(),
        aid: Aid::from_hex(aid).expect("static AID"),
        major,
        minor,
        introduced_in,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ExportRegistry, PackageExport, builtin_exports, parse_hint_lines};
    use crate::cap::ImportedPackage;
    use jcim_core::aid::Aid;
    use jcim_core::model::JavaCardClassicVersion;

    fn temp_path(suffix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "jcim-export-{suffix}-{unique}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn builtin_registry_is_filtered_by_version() {
        let v21 = ExportRegistry::new_for_version(JavaCardClassicVersion::V2_1);
        assert!(
            v21.validate_imports(&[ImportedPackage {
                aid: Aid::from_hex("A0000000620001").expect("aid"),
                major: 1,
                minor: 0,
            }])
            .is_ok()
        );
        assert!(
            v21.validate_imports(&[ImportedPackage {
                aid: Aid::from_hex("A0000000620102").expect("aid"),
                major: 1,
                minor: 0,
            }])
            .is_ok()
        );
        assert!(
            v21.validate_imports(&[ImportedPackage {
                aid: Aid::from_hex("A0000000620201").expect("aid"),
                major: 1,
                minor: 0,
            }])
            .is_err()
        );
    }

    #[test]
    fn register_overrides_or_adds_package_exports() {
        let mut registry = ExportRegistry::default();
        let package = PackageExport {
            name: "example.pkg".to_string(),
            aid: Aid::from_hex("A0000001510001").expect("aid"),
            major: 1,
            minor: 2,
            introduced_in: JavaCardClassicVersion::V3_0_1,
        };
        registry.register(package.clone());
        assert!(
            registry
                .validate_imports(&[ImportedPackage {
                    aid: package.aid,
                    major: 1,
                    minor: 2,
                }])
                .is_ok()
        );
    }

    #[test]
    fn register_hint_file_supports_json_toml_and_line_formats() {
        let dir = temp_path("hints");
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let json_path = dir.join("hint.json");
        let toml_path = dir.join("hint.toml");
        let lines_path = dir.join("hint.exp");
        let package = PackageExport {
            name: "example.pkg".to_string(),
            aid: Aid::from_hex("A0000001510002").expect("aid"),
            major: 1,
            minor: 0,
            introduced_in: JavaCardClassicVersion::V2_2,
        };
        std::fs::write(
            &json_path,
            serde_json::to_vec(&package).expect("json encode"),
        )
        .expect("write json");
        std::fs::write(&toml_path, toml::to_string(&package).expect("toml encode"))
            .expect("write toml");
        std::fs::write(
            &lines_path,
            "name=example.lines\naid=A0000001510003\nmajor=2\nminor=4\n",
        )
        .expect("write lines");

        let mut registry = ExportRegistry::default();
        registry.register_hint_file(&json_path).expect("json hint");
        registry.register_hint_file(&toml_path).expect("toml hint");
        registry
            .register_hint_file(&lines_path)
            .expect("lines hint");

        assert!(
            registry
                .validate_imports(&[ImportedPackage {
                    aid: Aid::from_hex("A0000001510002").expect("aid"),
                    major: 1,
                    minor: 0,
                }])
                .is_ok()
        );
        assert!(
            registry
                .validate_imports(&[ImportedPackage {
                    aid: Aid::from_hex("A0000001510003").expect("aid"),
                    major: 2,
                    minor: 4,
                }])
                .is_ok()
        );

        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn parse_hint_lines_requires_name_and_aid() {
        assert!(parse_hint_lines("name=only-name").is_err());
        assert!(parse_hint_lines("aid=A0000001510001").is_err());

        let parsed =
            parse_hint_lines("name=parsed\naid=A0000001510004\nmajor=9\nminor=8\n").expect("parse");
        assert_eq!(parsed.name, "parsed");
        assert_eq!(parsed.major, 9);
        assert_eq!(parsed.minor, 8);
        assert_eq!(parsed.introduced_in, JavaCardClassicVersion::V2_1);
    }

    #[test]
    fn builtin_exports_are_present_and_order_independent() {
        let exports = builtin_exports();
        assert!(
            exports
                .iter()
                .any(|package| package.name == "javacard.framework")
        );
        assert!(
            exports
                .iter()
                .any(|package| package.name == "javacard.security")
        );
    }
}
