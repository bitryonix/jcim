//! Project manifests and machine-local configuration for JCIM 0.2.

mod manifest;
mod paths;
mod user;

pub use manifest::{
    ArtifactKind, BuildKind, ProjectAppletConfig, ProjectBuildConfig, ProjectCardConfig,
    ProjectConfig, ProjectMetadataConfig, ProjectSimulatorConfig, ProjectSourceConfig,
};
pub use paths::{
    MANAGED_BUNDLES_DIR_NAME, ManagedPaths, PROJECT_MANIFEST_NAME, USER_CONFIG_FILE_NAME,
    find_project_manifest, project_name_from_root, resolve_project_path,
};
pub use user::UserConfig;
