//! Product-facing `jcim.toml` manifest model.

use std::path::PathBuf;

use jcim_core::aid::Aid;
use jcim_core::error::{JcimError, Result};
use jcim_core::model::CardProfileId;
use serde::{Deserialize, Serialize};

/// Supported build strategies for one JCIM project.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BuildKind {
    /// JCIM owns source discovery and Java Card build orchestration.
    Native,
    /// JCIM invokes an explicit external command and harvests declared outputs.
    Command,
}

/// Artifact kinds emitted by the build subsystem.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// CAP archive for simulator and physical-card workflows.
    Cap,
}

/// Declared applet entry in one project manifest.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct ProjectAppletConfig {
    /// Fully qualified applet class name.
    pub class_name: String,
    /// Applet instance AID.
    pub aid: Aid,
}

impl Default for ProjectAppletConfig {
    fn default() -> Self {
        Self {
            class_name: String::new(),
            aid: Aid::from_slice(&[0xF0, 0x00, 0x00, 0x00, 0x01, 0x01])
                .unwrap_or_else(|_| unreachable!("static default applet aid is valid")),
        }
    }
}

/// Core project identity stored under `[project]`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct ProjectMetadataConfig {
    /// Human-facing project name.
    pub name: String,
    /// Java Card profile selected for build and runtime defaults.
    pub profile: CardProfileId,
    /// Java package name for the project.
    pub package_name: String,
    /// Package AID for CAP packaging and session metadata.
    pub package_aid: Aid,
    /// Declared applets in the project.
    pub applets: Vec<ProjectAppletConfig>,
}

impl Default for ProjectMetadataConfig {
    fn default() -> Self {
        Self {
            name: "jcim-project".to_string(),
            profile: CardProfileId::Classic222,
            package_name: "com.example.demo".to_string(),
            package_aid: Aid::from_slice(&[0xF0, 0x00, 0x00, 0x00, 0x01])
                .unwrap_or_else(|_| unreachable!("static default package aid is valid")),
            applets: Vec::new(),
        }
    }
}

/// Source layout settings stored under `[source]`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
#[serde(default)]
pub struct ProjectSourceConfig {
    /// Primary source root. Defaults to `src/main/javacard`.
    pub root: Option<PathBuf>,
    /// Optional extra Java roots for shared or helper code.
    pub extra_roots: Vec<PathBuf>,
}

/// Build settings stored under `[build]`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct ProjectBuildConfig {
    /// Selected build strategy.
    pub kind: BuildKind,
    /// Artifact kinds emitted by default.
    pub emit: Vec<ArtifactKind>,
    /// Explicit external build command when `kind = "command"`.
    pub command: Option<String>,
    /// Declared CAP output for command builds.
    pub cap_output: Option<PathBuf>,
    /// Extra dependency roots or jars.
    pub dependencies: Vec<PathBuf>,
    /// Package version encoded into emitted artifacts.
    pub version: String,
}

impl Default for ProjectBuildConfig {
    fn default() -> Self {
        Self {
            kind: BuildKind::Native,
            emit: vec![ArtifactKind::Cap],
            command: None,
            cap_output: None,
            dependencies: Vec::new(),
            version: "1.0".to_string(),
        }
    }
}

/// Simulator defaults stored under `[simulator]`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct ProjectSimulatorConfig {
    /// Whether stale sources should be rebuilt automatically before starting a simulation.
    pub auto_build: bool,
    /// Whether JCIM should issue one explicit reset after the CAP install flow completes.
    pub reset_after_start: bool,
}

impl Default for ProjectSimulatorConfig {
    fn default() -> Self {
        Self {
            auto_build: true,
            reset_after_start: false,
        }
    }
}

/// Real-card defaults stored under `[card]`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct ProjectCardConfig {
    /// Default physical reader name for card operations.
    pub default_reader: Option<String>,
    /// Optional explicit CAP path override for card install flows.
    pub default_cap_path: Option<PathBuf>,
    /// Whether card install should build stale projects automatically.
    pub auto_build_before_install: bool,
}

impl Default for ProjectCardConfig {
    fn default() -> Self {
        Self {
            default_reader: None,
            default_cap_path: None,
            auto_build_before_install: true,
        }
    }
}

/// Clean-slate `jcim.toml` manifest.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
#[serde(default)]
pub struct ProjectConfig {
    /// Project identity and package metadata.
    #[serde(rename = "project")]
    pub metadata: ProjectMetadataConfig,
    /// Source layout settings.
    pub source: ProjectSourceConfig,
    /// Build settings.
    pub build: ProjectBuildConfig,
    /// Simulator defaults.
    pub simulator: ProjectSimulatorConfig,
    /// Real-card defaults.
    pub card: ProjectCardConfig,
}

impl ProjectConfig {
    /// Build a starter project manifest for one project name.
    pub fn default_for_project_name(project_name: &str) -> Self {
        let normalized = project_name
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .map(|character| character.to_ascii_lowercase())
            .collect::<String>();
        let suffix = if normalized.is_empty() {
            "demo".to_string()
        } else {
            normalized
        };
        Self {
            metadata: ProjectMetadataConfig {
                name: project_name.to_string(),
                package_name: format!("com.jcim.{suffix}"),
                applets: vec![ProjectAppletConfig {
                    class_name: format!("com.jcim.{suffix}.{}Applet", title_case(project_name)),
                    aid: Aid::from_slice(&[0xF0, 0x00, 0x00, 0x00, 0x01, 0x01])
                        .unwrap_or_else(|_| unreachable!("static default applet aid is valid")),
                }],
                ..ProjectMetadataConfig::default()
            },
            ..Self::default()
        }
    }

    /// Return the effective primary source root.
    pub fn source_root(&self) -> PathBuf {
        self.source
            .root
            .clone()
            .unwrap_or_else(|| PathBuf::from("src/main/javacard"))
    }

    /// Return whether the manifest uses an external command build.
    pub fn is_command_build(&self) -> bool {
        self.build.kind == BuildKind::Command
    }

    /// Decode one project manifest from TOML text.
    pub fn from_toml_str(value: &str) -> Result<Self> {
        Ok(toml::from_str(value)?)
    }

    /// Decode one project manifest from disk.
    pub fn from_toml_path(path: &std::path::Path) -> Result<Self> {
        Self::from_toml_str(&std::fs::read_to_string(path)?)
    }

    /// Encode the manifest as pretty TOML for disk writes.
    pub fn to_pretty_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|error| {
            JcimError::Unsupported(format!("unable to encode project config: {error}"))
        })
    }
}

/// Convert a human-facing project name into a Java-style class-name suffix.
fn title_case(value: &str) -> String {
    let mut output = String::new();
    let mut capitalize = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if capitalize {
                output.push(character.to_ascii_uppercase());
                capitalize = false;
            } else {
                output.push(character.to_ascii_lowercase());
            }
        } else {
            capitalize = true;
        }
    }
    if output.is_empty() {
        "Demo".to_string()
    } else {
        output
    }
}
