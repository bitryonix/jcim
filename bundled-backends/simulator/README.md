## Official Simulator Bundle

This directory contains the maintained CAP-first simulator backend for JCIM 0.2.

It is launched by the local JCIM service through `jcim-backends` and is responsible for:

- starting the official Java Card simulator process available in the bundled SDK set
- installing a CAP through the official `scriptgen` flow
- exchanging APDUs through the official `apduio` transport library

The bundle expects:

- `libs/jcim-simulator-backend.jar`
- `manifest.toml`
- the repository-bundled SDKs under `third_party/javacard_sdks`

Host support is intentionally honest:

- Linux: native official simulator flow for the bundled 2.2.x SDKs
- Windows: native official simulator flow for the bundled 3.x SDKs
- macOS: requires an operator-provided container command through `JCIM_SIMULATOR_CONTAINER_CMD`

This is the only maintained simulator bundle in the workspace.
