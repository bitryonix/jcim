## Managed Simulator Bundle

This directory contains the maintained managed-Java simulator backend for JCIM 0.3.

It is launched by the local JCIM service through `jcim-backends` and is responsible for:

- starting the bundled `jcardsim`-backed simulator backend inside a managed JVM
- loading applets from compiled project classes plus simulator metadata
- exchanging APDUs through the JSON-line backend protocol
- returning authoritative ISO session state after stateful operations

The bundle expects:

- `libs/jcim-simulator-backend.jar`
- `manifest.toml`
- `../../third_party/jcardsim/jcardsim.jar`

The service supplies:

- `classes_path`
- `simulator_metadata_path`
- project/runtime classpath entries
- the bundled or configured `java` binary to launch the backend

Maintained host support:

- macOS: zero-setup managed Java path
- Linux: zero-setup managed Java path
- other hosts: not part of the maintained zero-setup contract for JCIM 0.3

The backend does not install CAP files directly. JCIM starts simulations from project-backed
classes, runtime classpath entries, and simulator metadata.

On hosts outside the maintained Linux/macOS `x86_64` and `aarch64` matrix, operators must use the
explicit fallback path documented at `jcim system setup --java-bin /path/to/java`.

This is the only maintained simulator bundle in the workspace.

Governance:

- `libs/jcim-simulator-backend.jar` and `third_party/jcardsim/jcardsim.jar` are tracked in
  `third_party/THIRD_PARTY.toml` with explicit update cadence and SHA-256 verification.
- Changes to the backend jar or the pinned `jcardsim` jar should update that manifest in the same
  review as the binary refresh.
