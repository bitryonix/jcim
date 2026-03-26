# Design Decisions

## Simulator-first product

- JCIM 0.2 is centered on one primary capability: APDU-driven Java Card simulation from CAP input.
- Source projects are supported by building them to CAP and starting the same simulator flow.
- Physical-card utilities remain in scope, but they are explicitly secondary.

Reason:
- the workspace should optimize for one honest simulator story instead of multiple weaker runtime stories
- CAP-first behavior keeps source builds and raw CAP usage aligned

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
- raw `.cap` input must be a first-class API concept rather than a hidden internal path
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
- The maintained simulator backend is the official CAP-capable Java Card simulator tooling.
- `builtin` and `jcardsim` are removed from the maintained product surface.

Reason:
- builtin execution was not high fidelity
- `jcardsim` was JAR-backed and did not make raw CAP a first-class simulator input

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
