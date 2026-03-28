# jcim-config

`jcim-config` owns operational configuration for JCIM.

It includes:

- simulator backend launch selection
- project-local `jcim.toml` manifests
- source-root and build-command metadata
- CAP artifact metadata
- simulator defaults
- real-card defaults for `jcim card ...`
- machine-local user configuration
- Java Card profile selection
- hardware overrides
- TOML loading helpers

Illustrative low-level use:

```rust
use jcim_config::config::RuntimeConfig;

let config = RuntimeConfig::default();
assert_eq!(config.backend.kind, jcim_core::model::BackendKind::Simulator);
```

Illustrative workflow-first use:

```rust
use jcim_config::project::ProjectConfig;

let project = ProjectConfig::default_for_project_name("demo-card");
assert_eq!(project.metadata.name, "demo-card");
assert!(project.simulator.auto_build);
```

The project manifest surface now includes:

- `[project]` for package metadata and applets
- `[source]` for source roots
- `[build]` for `native` or `command` CAP builds
- `[simulator]` for simulator startup behavior
- `[card]` for default reader and CAP resolution during physical-card operations
