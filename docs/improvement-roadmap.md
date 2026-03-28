# JCIM 0.3 Improvement Roadmap

This document is the maintainer-facing roadmap for aligning the implementation more closely with
the existing JCIM 0.3 architecture without changing the product direction.

## Invariants

The roadmap preserves these decisions:

- JCIM remains simulator-first.
- `jcimd` remains the single local control plane.
- `jcim-app` remains the transport-neutral application boundary.
- `jcim-cli` remains a thin task-oriented shell.
- `jcim-sdk` remains the canonical Rust client over the same service contract.
- The maintained simulator path remains project-backed, class-backed, and managed-Java.
- The maintained operator path remains typed ISO/IEC 7816 and typed GlobalPlatform, with raw APDU
  passthrough kept as an expert escape hatch.
- The repository keeps its current strictness posture: no `unsafe`, strong lints, strong docs, and
  explicit domain boundaries.

## Crate Architecture Summary

| Crate | Current role | Notes |
| --- | --- | --- |
| `jcim-core` | Shared AIDs, APDUs, errors, protocol/domain models | Protocol surfaces are now split into directory modules; the remaining concentration is mostly in `iso7816/commands/{decoded,builders}.rs`. |
| `jcim-config` | Project manifests, managed paths, runtime metadata, machine-local config | Now owns the shared atomic managed-file writer used by config/registry/runtime persistence. |
| `jcim-build` | Source discovery, toolchain layout, artifact metadata, CAP build orchestration | Stable lower-level dependency. |
| `jcim-cap` | CAP parsing and validation | CAP parsing is now split into archive/component/parser/validation modules; remaining work here is modest follow-up polish, not another structural split. |
| `jcim-backends` | Managed simulator backend spawning and JSON-line supervision | Good structural reference for bounded runtime ownership. |
| `jcim-api` | Sole maintained gRPC/protobuf contract | Still intentionally single-file at `proto/jcim/v0_3/service.proto`; now exports a descriptor set for compatibility checks. |
| `jcim-app` | Project/build/sim/card/system application services | Mutable state ownership is now explicit and fully split into state, runtime, and card submodules under the maintained façade. |
| `jcim-sdk` | Canonical Rust client, bootstrap, proto mapping, lifecycle helpers | Already internally split by domain. |
| `jcimd` | Local gRPC transport layer over Unix-domain sockets | Already has targeted stale-socket and runtime-cleanup tests. |
| `jcim-cli` | Task-oriented CLI shell and JSON contract | Already internally split and guarded by JSON/doc smoke tests. |

## Dependency And Responsibility Map

- `jcim-core` is the dependency root and stays transport/process/filesystem agnostic.
- `jcim-config`, `jcim-build`, `jcim-cap`, and `jcim-backends` remain lower-level support crates.
- `jcim-app` depends inward on those lower-level crates and owns product/application policy.
- `jcimd` depends on `jcim-app` and `jcim-api`; transport concerns stay there.
- `jcim-sdk` depends on `jcim-api` and hides bootstrap/channel/proto details behind Rust types.
- `jcim-cli` depends on `jcim-sdk` and should not accumulate business logic.

Current maintained compatibility surfaces:

- protobuf package `jcim.v0_3`
- CLI JSON schema `jcim-cli.v2`
- `jcim.toml`
- machine-local `config.toml`, `projects.toml`, and `jcimd.runtime.toml`
- public `jcim-sdk` types and methods such as `JcimClient`, `ProjectRef`, `SimulationRef`, and
  `ReaderRef`

## Hotspot Inventory

Current large files after the final convergence tranche:

| File | Lines | Risk |
| --- | ---: | --- |
| `crates/jcim-api/proto/jcim/v0_3/service.proto` | 668 | Intentionally governed as one compatibility-sensitive file for the 0.3 cycle. |
| `crates/jcim-core/src/iso7816/commands/decoded.rs` | 444 | Typed command decoding is now isolated, but still the single largest remaining ISO command slice. |
| `crates/jcim-app/src/card/mock_adapter/iso.rs` | 420 | Mock ISO command behavior is now isolated from inventory/GP/process concerns, but remains a dense deterministic protocol surface. |
| `crates/jcim-app/src/app/simulations/runtime/startup.rs` | 245 | Runtime startup is now isolated and explicitly preserves reserve -> startup -> commit/fail behavior. |
| `crates/jcim-core/src/iso7816/session.rs` | 319 | Session mutation and response application are clearer now, but still a meaningful correctness hotspot. |
| `crates/jcim-core/src/iso7816/commands/builders.rs` | 250 | APDU builder coverage is now isolated and easier to review, but still broad enough to merit continued characterization coverage. |
| `crates/jcim-app/src/app/state/simulations.rs` | 190 | The simulation store layer is explicit and bounded, and is now the main app-state correctness seam. |
| `crates/jcim-app/src/app/cards/inventory.rs` | 166 | Physical-card inventory/install/reset flows are now isolated from ISO/GP/raw-card transport paths. |

Implemented reductions in concentration are now:

- `crates/jcim-app/src/app/state/mod.rs`
- `crates/jcim-app/src/app/state/config.rs`
- `crates/jcim-app/src/app/state/registry.rs`
- `crates/jcim-app/src/app/state/simulations.rs`
- `crates/jcim-app/src/app/state/build_events.rs`
- `crates/jcim-app/src/app/state/card_sessions.rs`
- `crates/jcim-app/src/app/projects.rs`
- `crates/jcim-app/src/app/builds.rs`
- `crates/jcim-app/src/app/simulations/mod.rs`
- `crates/jcim-app/src/app/simulations/query.rs`
- `crates/jcim-app/src/app/simulations/runtime/mod.rs`
- `crates/jcim-app/src/app/simulations/runtime/startup.rs`
- `crates/jcim-app/src/app/simulations/runtime/control.rs`
- `crates/jcim-app/src/app/simulations/runtime/session.rs`
- `crates/jcim-app/src/app/simulations/runtime/events.rs`
- `crates/jcim-app/src/app/cards/mod.rs`
- `crates/jcim-app/src/app/cards/inventory.rs`
- `crates/jcim-app/src/app/cards/iso.rs`
- `crates/jcim-app/src/app/cards/gp.rs`
- `crates/jcim-app/src/app/cards/raw.rs`
- `crates/jcim-app/src/app/system.rs`
- `crates/jcim-app/src/app/selectors.rs`
- `crates/jcim-app/src/app/events.rs`
- `crates/jcim-app/src/card/adapter.rs`
- `crates/jcim-app/src/card/gp_keyset.rs`
- `crates/jcim-app/src/card/helper_tool.rs`
- `crates/jcim-app/src/card/inventory_parser.rs`
- `crates/jcim-app/src/card/java_adapter.rs`
- `crates/jcim-app/src/card/mock_adapter/mod.rs`
- `crates/jcim-app/src/card/mock_adapter/state.rs`
- `crates/jcim-app/src/card/mock_adapter/dispatch.rs`
- `crates/jcim-app/src/card/mock_adapter/inventory.rs`
- `crates/jcim-app/src/card/mock_adapter/iso.rs`
- `crates/jcim-app/src/card/mock_adapter/globalplatform.rs`
- `crates/jcim-core/src/iso7816/atr.rs`
- `crates/jcim-core/src/iso7816/status_word.rs`
- `crates/jcim-core/src/iso7816/secure_messaging.rs`
- `crates/jcim-core/src/iso7816/selection.rs`
- `crates/jcim-core/src/iso7816/session.rs`
- `crates/jcim-core/src/iso7816/commands/mod.rs`
- `crates/jcim-core/src/iso7816/commands/constants.rs`
- `crates/jcim-core/src/iso7816/commands/classification.rs`
- `crates/jcim-core/src/iso7816/commands/decoded.rs`
- `crates/jcim-core/src/iso7816/commands/builders.rs`
- `crates/jcim-core/src/globalplatform/commands.rs`
- `crates/jcim-core/src/globalplatform/lifecycle.rs`
- `crates/jcim-core/src/globalplatform/parsers.rs`
- `crates/jcim-core/src/globalplatform/secure_channel.rs`
- `crates/jcim-core/src/globalplatform/status.rs`
- `crates/jcim-cap/src/cap/archive.rs`
- `crates/jcim-cap/src/cap/components.rs`
- `crates/jcim-cap/src/cap/error.rs`
- `crates/jcim-cap/src/cap/parser.rs`
- `crates/jcim-cap/src/cap/validation.rs`

These now hold the extracted application/domain slices, state structs, selector resolution,
event retention helpers, physical-card helper/process seams, deterministic mock-card behavior,
protocol subdomains, GP secure-channel/parsing seams, and CAP archive/component/validation seams.

## Lock And Mutable-State Inventory

`jcim-app` still centrally owns mutable application state:

| Location | Primitive | Protected state | Current posture |
| --- | --- | --- | --- |
| `AppState.user_config` | `RwLock` | Machine-local user config | Short synchronous reads/writes only. |
| `AppState.registry` | `RwLock` | Machine-local project registry | Short synchronous reads/writes only. |
| `AppState.simulations` | `Mutex` | Managed simulation records and backend handles | Now accessed through store helpers; startup follows reserve -> start -> commit/fail, and callers clone handles or summaries before `.await`. |
| `AppState.build_events` | `Mutex` | Retained build event queues | Now accessed through dedicated retention helpers only. |
| `AppState.card_sessions` | `Mutex` | Reader-keyed ISO/GP session tracking | Now accessed through explicit helper methods for status sync, command application, secure messaging, GP channel state, and reset. |
| `MockPhysicalCardAdapter.state` | `Mutex` | Deterministic mock card model | Fine for tests and deterministic adapter behavior. |

`jcimd` uses `spawn_blocking` for synchronous `jcim-app` calls and already has dedicated runtime
metadata ownership/cleanup tests. The current target is not replacing these locks wholesale; it is
making lock scope and state ownership more explicit before deciding whether any substate deserves a
bounded actor model.

Current lock-policy notes after the lifecycle-hardening tranche:

- simulation, build-event, and card-session maps are mutated through `AppState` helpers rather than
  ad hoc locking in feature modules
- backend handles are cloned out of the simulation store before async work
- response/session-state commits happen in one bounded synchronous step after async work returns
- failed simulator startup now leaves a retained failed summary plus retained events instead of
  dropping the attempt on the floor
- the final 0.3 decision is to keep the synchronous store-helper model and not actorize
  `simulations` or `card_sessions`

## Behavior Preservation Checklist

Changes must preserve these maintained paths unless a versioned contract change is deliberately
planned:

- CLI command behavior for maintained command families
- CLI `--json` `schema_version` and `kind` markers plus payload keys
- daemon bootstrap, stale-socket replacement, runtime metadata ownership, and binary-identity
  checks
- project discovery and registry persistence
- build event retention and streaming behavior
- project-backed simulation lifecycle
- typed ISO/IEC 7816 and typed GlobalPlatform flows
- real-card helper parsing behavior and mock-card determinism
- managed path names and manifest/config file names

Current guardrails already in the repo:

- CLI JSON contract tests
- CLI docs smoke tests
- SDK lifecycle tests, including repeated restart and multi-client simulation access
- daemon runtime cleanup and stale-socket tests
- third-party governance tests

New guardrails added in this tranche:

- `crates/jcim-app/tests/characterization.rs`
- `crates/jcim-api/tests/descriptor_contract.rs`
- atomic managed-file writer tests plus config/registry persistence checks

## Risk Register

| Risk | Location | Failure mode | Mitigation |
| --- | --- | --- | --- |
| State concentration | `jcim-app/src/app/simulations/runtime/{startup,session,control}.rs` | Hidden coupling across simulation lifecycle, session tracking, and backend startup | Mitigated by the final split plus explicit store helpers and retained-startup characterization tests. |
| Parser/process coupling | `jcim-app/src/card/*` | Text parser edits accidentally alter helper launch behavior | Completed first pass by splitting helper launch, parsing, keyset handling, and mock adapter into distinct modules. |
| Non-atomic managed writes | config/registry persistence | Partial or torn writes on interruption | Completed for config/registry/runtime via shared atomic writer; keep future writes on the same path. |
| Contract drift | `jcim.v0_3` and CLI JSON | Silent field/service/shape changes | Descriptor-backed proto tests, JSON contract tests, and migration/versioning docs. |
| Lock-scope ambiguity | `jcim-app` mutable maps | Hard-to-debug future concurrency regressions | Centralize mutation helpers, document lock policy, and add interleaving tests. |
| Supply-chain drift | `third_party/`, `bundled-backends/` | Bundled asset and manifest divergence | Keep `THIRD_PARTY.toml` governance tests and contributor refresh docs. |

## Staged Execution Plan

### Stage 0: Audit Pack And Characterization Gate

Status: complete.

Implemented:

- this roadmap
- `jcim-app` characterization tests
- `jcim-api` descriptor compatibility tests

Success criteria:

- roadmap exists and is maintained
- compatibility surfaces are explicitly listed
- characterization tests run in normal CI

### Stage 1: Persistence Safety Before Structural Churn

Status: complete.

Implemented:

- shared atomic regular-file writer in `jcim-config`
- `UserConfig::save_to_path` now uses temp-file plus `sync_all` plus rename
- `ProjectRegistry::save_to_path` now uses the same path
- legacy managed-file migration now uses the same path
- runtime metadata keeps atomic persistence while using the shared helper

Success criteria:

- no open-coded managed config/registry writes remain on the normal persistence path
- symlink/non-file destinations fail closed for managed-file writes

### Stage 2: `jcim-app` Split

Status: complete.

Implemented:

- state structs moved to `src/app/state/{mod,config,registry,simulations,build_events,card_sessions}.rs`
- project orchestration moved to `src/app/projects.rs`
- build orchestration moved to `src/app/builds.rs`
- simulation orchestration moved to `src/app/simulations/{query,runtime}.rs`
- card-facing app orchestration moved to `src/app/cards/{mod,inventory,iso,gp,raw}.rs`
- system/toolchain orchestration moved to `src/app/system.rs`
- selector resolution moved to `src/app/selectors.rs`
- event retention moved to `src/app/events.rs`

Completed in the follow-on lifecycle tranche:

- simulation, build-event, and card-session state access now routes through explicit `AppState`
  helpers
- the simulation surface is split into query vs runtime modules
- simulation startup now reserves state before backend launch, then commits running or failed state
  explicitly

Success criteria:

- `app.rs` becomes façade glue instead of the main home of all internal helpers
- simulation and card-session mutation paths become easier to test in isolation

### Stage 3: `jcim-app/card` Boundary Cleanup

Status: complete.

Implemented:

- adapter contract moved to `src/card/adapter.rs`
- GP keyset resolution moved to `src/card/gp_keyset.rs`
- external helper/GPPro execution moved to `src/card/helper_tool.rs`
- inventory parsing moved to `src/card/inventory_parser.rs`
- bundled Java-backed adapter moved to `src/card/java_adapter.rs`
- deterministic mock adapter now split across `src/card/mock_adapter/{mod,state,dispatch,inventory,iso,globalplatform}.rs`
- parser tests now live beside the parser module
- mock-card behavior tests now live beside the mock adapter
- mock inventory/delete/status regression coverage now lives beside the mock adapter

Success criteria:

- parser-only edits cannot alter process launch behavior
- GP env-derived material never appears in diagnostics

### Stage 4: Protocol And CAP Decomposition

Status: complete.

Implemented:

- split `iso7816` into ATR, status-word, session, selection, secure-messaging, and command
  submodules
- split `iso7816::commands` again into constants, classification, decoded command types, and
  builders
- split `globalplatform` into commands, lifecycle, parsers, secure-channel, and status modules
- split `cap` into archive, components, parser, validation, and error modules
- preserved top-level re-exports and existing call sites

Success criteria:

- low-level protocol logic is locally testable and easier to review
- downstream imports remain stable

### Stage 5: Concurrency And Lifecycle Hardening

Status: complete.

Implemented:

- simulation, build-event, and card-session maps now use explicit store helpers
- simulation startup now follows reserve -> backend startup -> commit running/failed state
- simulation/card-session commits happen after async work returns instead of through open-coded map
  mutation
- `jcim-app` characterization coverage now protects repeated simulation cycles, retained failed
  startup records, and interleaved session mutations
- `jcim-sdk` lifecycle coverage now protects stale-runtime replacement plus daemon-healthy/backend-failing startup behavior
- `jcimd` runtime cleanup coverage now protects daemon recovery after managed backend startup failure

Success criteria:

- no lock is held across `.await`
- mutable maps are not updated ad hoc throughout the crate

### Stage 6: Contract Governance And Contributor Docs

Status: complete.

Implemented:

- descriptor-set export from `jcim-api`
- contributor/security/troubleshooting/third-party/versioning docs added
- architecture overview and ADR 0009 added
- API/CLI/system docs aligned around the 0.3 contract baseline
- optional proto splitting closed for 0.3; `service.proto` remains intentionally single-file

Success criteria:

- contributors can find one clear path for setup, contract expectations, third-party refreshes,
  troubleshooting, and release/versioning rules

### Stage 7: CI Realism For Supported Hosts

Status: complete.

Implemented:

- workflow matrix expanded to explicit Linux/macOS x86_64 and aarch64 runners
- new descriptor and `jcim-app` characterization tests added to required CI
- explicit targeted CI/release gates now call descriptor, characterization, CLI JSON/docs,
  SDK lifecycle, and daemon runtime-cleanup suites directly
- daemon-managing integration suites now run with `--test-threads=1`

Success criteria:

- the documented supported host matrix matches the actual required CI matrix
- new contract and characterization tests stay review-blocking
