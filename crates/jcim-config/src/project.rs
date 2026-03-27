//! Project manifests and machine-local configuration for JCIM 0.3.

mod manifest;
mod paths;
mod runtime;
mod user;

pub use manifest::{
    ArtifactKind, BuildKind, ProjectAppletConfig, ProjectBuildConfig, ProjectCardConfig,
    ProjectConfig, ProjectMetadataConfig, ProjectSimulatorConfig, ProjectSourceConfig,
};
pub use paths::{
    MANAGED_BUNDLES_DIR_NAME, ManagedPaths, PROJECT_MANIFEST_NAME, PROJECT_REGISTRY_FILE_NAME,
    SERVICE_RUNTIME_FILE_NAME, USER_CONFIG_FILE_NAME, find_project_manifest,
    project_name_from_root, resolve_project_path,
};
pub use runtime::{
    ServiceRuntimeRecord, current_runtime_record_format_version,
    remove_owned_runtime_file_if_present, remove_owned_socket_if_present,
    runtime_metadata_path_for_socket,
};
pub use user::UserConfig;
