# Design Decisions

## Simulator-first product

- JCIM 0.2 is centered on one primary capability: APDU-driven Java Card simulation from JCIM
  project builds.
- Source projects are supported by building them once and starting a managed class-backed
  simulation from the emitted classes, runtime classpath, and simulator metadata.
- Physical-card utilities remain in scope, but they are explicitly secondary.

Reason:
- the workspace should optimize for one honest simulator story instead of multiple weaker runtime stories
- project-backed behavior keeps source builds, examples, and simulator execution aligned across
  macOS and Linux without extra operator setup

## Service-first control plane

- `jcimd` is the single local gRPC control plane.
- `jcim-cli` is a thin client shell.
- `jcim-sdk` is the canonical Rust developer client over the same service.
- Future GUI work is expected to use the same gRPC contract.

Reason:
- one warm local service owns project resolution, simulator state, and machine-local configuration
- this keeps the CLI, Rust API, and future GUI behavior consistent

## Clean application boundary

- `jcim-app` is the transport-neutral application boundary.
- It owns:
  - project discovery and registry
  - CAP build orchestration
  - simulation lifecycle
  - physical-card operations behind an injectable adapter boundary
  - machine-local setup and doctor flows

Reason:
- transport code should not own simulator or card behavior
- real-card behavior needs one production adapter and one deterministic test adapter
- the same use cases should serve CLI, Rust SDK, and future GUI consumers

## Public API contract

- `jcim-api` protobuf is the public local contract.
- The service exposes task-oriented services:
  - workspace
  - project
  - build
  - simulator
  - card
  - system

Reason:
- the simulator contract should be explicit, testable, and GUI-ready
- project-backed startup keeps the maintained simulator contract narrow and explicit
- typed card responses should remove the need for Rust or CLI callers to parse helper text

## CLI redesign

- The public CLI is task-oriented:
  - `project`
  - `build`
  - `sim`
  - `card`
  - `system`

Reason:
- commands should reflect operator tasks instead of internal subsystems
- simulator control should use simulator vocabulary directly

## Simulator engine posture

- The maintained backend kind is `simulator`.
- The maintained simulator backend is a bundled managed-Java `jcardsim` engine.
- It loads applets from project build outputs instead of installing CAP files into an external
  simulator process.
- `native` and `container` engine modes remain decode-compatible for one release, but
  `managed_java` is the maintained engine mode for new simulations.

Reason:
- zero-setup macOS and Linux behavior matters more than preserving the old official-simulator path
- a managed class-backed simulator keeps the maintained path deterministic and self-contained
- CAP artifacts still matter for install and build inspection, but not as a maintained simulator
  startup path

## Configuration model

- `jcim.toml` is the project-facing manifest.
- Machine-local state lives under the managed JCIM root in one user config file and one project registry file.
- The manifest uses:
  - `[project]`
  - `[source]`
  - `[build]`
  - `[simulator]`
  - `[card]`

Reason:
- configuration should reflect simulator and card tasks, not deleted runtime splits
- source-of-truth project state and machine-local settings should stay clearly separated

## Compatibility posture

- JCIM 0.2 is a deliberate breaking redesign.
- Old runtime modes, old command trees, and old manifest sections are not preserved.

Reason:
- carrying the old surface forward would keep the architecture anchored to the wrong center of gravity

## Validation posture

- Simulator and card flows are validated at the service boundary.
- CI coverage uses a deterministic in-memory physical-card adapter for real-card lifecycle semantics.
- True hardware validation remains opt-in through environment-gated tests.

Reason:
- lifecycle behavior should be verified end to end without depending on live hardware in every test run
- hardware validation should stay first-class without making the default test suite flaky
