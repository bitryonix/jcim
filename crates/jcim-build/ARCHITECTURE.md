# jcim-build Architecture

## Intent

`jcim-build` owns Java Card source discovery, build planning, artifact production, and build
metadata for JCIM projects.

## Structure

- `build.rs`: façade that re-exports the maintained build modules
- `build/request.rs`: typed build request inputs and CLI-facing conversion helpers
- `build/types.rs`: shared build result and metadata types
- `build/metadata.rs`: `.jcim/build/metadata.toml` persistence
- `build/fingerprint.rs`: source hashing and stale-build detection
- `build/toolchain.rs`: bundled toolchain layout and path resolution
- `build/executor.rs`: native JCIM build execution and explicit external-build adapters

## Dependency direction

- `jcim-build` depends on `jcim-config`, `jcim-cap`, and core model types.
- Application services and CLI shells depend on `jcim-build` for planning and execution.
- The crate intentionally stays free of CLI parsing and local-service lifecycle concerns.

## Design notes

- Build outputs remain standardized under `.jcim/build/`.
- Rebuild/staleness decisions are centralized here so callers do not reconstruct hashing and
  metadata logic ad hoc.
- External Ant/Maven/Gradle integration remains explicit-command based by design.
