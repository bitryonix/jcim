//! Java Card source and artifact build orchestration for JCIM.
//!
//! # Why this exists
//! Workflow-first CLI commands need a maintained place to turn Java Card source projects into CAP
//! artifacts without hard-coding toolchain details into the CLI crate.

mod build;

pub use build::{
    ArtifactMetadata, BuildAppletMetadata, BuildArtifactRequest, BuildOutcome, ToolchainLayout,
    artifact_metadata_from_project, build_project_artifacts, build_project_artifacts_if_stale,
    build_toolchain_layout, load_artifact_metadata,
};
pub use jcim_config::project::{ArtifactKind, BuildKind};
