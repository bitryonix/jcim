//! Public build request and metadata types.

use std::path::PathBuf;

use jcim_config::project::{ArtifactKind, BuildKind, ProjectAppletConfig};
use jcim_core::aid::Aid;
use jcim_core::model::CardProfileId;
use serde::{Deserialize, Serialize};

/// One applet entry recorded in generated artifact metadata.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct BuildAppletMetadata {
    /// Fully-qualified applet class name.
    pub class_name: String,
    /// Instance AID used for simulator and card-install workflows.
    pub aid: Aid,
}

impl From<ProjectAppletConfig> for BuildAppletMetadata {
    fn from(value: ProjectAppletConfig) -> Self {
        Self {
            class_name: value.class_name,
            aid: value.aid,
        }
    }
}

/// Resolved build request derived from a project manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildArtifactRequest {
    /// Project root that owns the source tree and `.jcim/build`.
    pub project_root: PathBuf,
    /// Selected build strategy.
    pub build_kind: BuildKind,
    /// Resolved source roots used for native discovery.
    pub source_roots: Vec<PathBuf>,
    /// External build command for command builds.
    pub command: Option<String>,
    /// Declared CAP output for command builds.
    pub cap_output: Option<PathBuf>,
    /// Requested build profile.
    pub profile: CardProfileId,
    /// Artifact kinds emitted by default.
    pub emit: Vec<ArtifactKind>,
    /// Java package name.
    pub package_name: String,
    /// Java Card package AID.
    pub package_aid: Aid,
    /// Package version.
    pub version: String,
    /// Declared applet classes and AIDs.
    pub applets: Vec<BuildAppletMetadata>,
    /// Extra dependency roots or jars.
    pub dependencies: Vec<PathBuf>,
}

/// Stable artifact metadata written under `.jcim/build/metadata.toml`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ArtifactMetadata {
    /// Build strategy that produced the artifacts.
    pub build_kind: BuildKind,
    /// Java Card profile used by the last successful build.
    pub profile: CardProfileId,
    /// Java package name.
    pub package_name: String,
    /// Java Card package AID.
    pub package_aid: Aid,
    /// Package version encoded into the CAP archive.
    pub version: String,
    /// Declared applets.
    pub applets: Vec<BuildAppletMetadata>,
    /// CAP artifact path relative to the project root when emitted.
    pub cap_path: Option<PathBuf>,
    /// Compiled classes directory relative to the project root.
    pub classes_path: PathBuf,
    /// Simulator metadata path relative to the project root.
    pub simulator_metadata_path: PathBuf,
    /// Fingerprint of the source tree and build-relevant metadata.
    pub source_fingerprint: String,
}

/// Outcome returned by one build attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildOutcome {
    /// Stable artifact metadata for the project.
    pub metadata: ArtifactMetadata,
    /// Whether the builder had to rebuild artifacts instead of reusing them.
    pub rebuilt: bool,
}

/// Resolved bundled toolchain layout used by the builder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolchainLayout {
    /// Workspace `third_party/` root.
    pub third_party_root: PathBuf,
    /// Bundled Eclipse compiler jar used for old Java Card classfile targets.
    pub ecj_jar: PathBuf,
    /// Bundled ant-javacard jar kept for provenance and operator escape hatches.
    pub ant_javacard_jar: PathBuf,
    /// Bundled Java Card SDK directory root.
    pub sdk_root: PathBuf,
}
