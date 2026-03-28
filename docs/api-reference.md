# API Reference

JCIM 0.3 exposes its sole maintained local gRPC API over a Unix-domain socket.

Maintained contract rules:

- protobuf package: `jcim.v0_3`
- CLI JSON schema above the service boundary: `jcim-cli.v2`
- managed files used around the service: `jcim.toml`, `config.toml`, `projects.toml`,
  `jcimd.runtime.toml`
- supported maintained hosts: Linux/macOS on `x86_64` and `aarch64`
- unsupported-host Java fallback remains explicit: `jcim system setup --java-bin /path/to/java`
- GP key material is env-derived and must not appear in logs, JSON errors, or persisted managed
  files

The canonical Rust consumer of this contract is:

- [`crates/jcim-sdk`](../crates/jcim-sdk/README.md)

The maintained Rust runtime-callable APDU surface is the SDK-level unified connection API:

- `JcimClient::open_card_connection(...)`
- `CardConnectionTarget::{Reader, ExistingSimulation, StartSimulation}`
- `CardConnection::{transmit, transmit_raw, session_state, reset_summary, close}`

This is an SDK abstraction over the existing service-first stack. APDUs are the message unit, and
the connection can target either one real reader or one virtual simulation without changing the
underlying gRPC contract.

## Services

- `WorkspaceService`
  - `GetOverview`
  - `ListProjects`
  - `ListSimulations`
- `ProjectService`
  - `CreateProject`
  - `GetProject`
  - `CleanProject`
- `BuildService`
  - `BuildProject`
  - `GetArtifacts`
  - `StreamBuildEvents`
- `SimulatorService`
  - `StartSimulation`
  - `StopSimulation`
  - `GetSimulation`
  - `GetSessionState`
  - `StreamSimulationEvents`
  - `TransmitApdu`
  - `TransmitRawApdu`
  - `ManageChannel`
  - `OpenSecureMessaging`
  - `AdvanceSecureMessaging`
  - `CloseSecureMessaging`
  - `OpenGpSecureChannel`
  - `CloseGpSecureChannel`
  - `ResetSimulation`
- `CardService`
  - `ListReaders`
  - `GetCardStatus`
  - `InstallCap`
  - `DeleteItem`
  - `ListPackages`
  - `ListApplets`
  - `GetSessionState`
  - `TransmitApdu`
  - `TransmitRawApdu`
  - `ManageChannel`
  - `OpenSecureMessaging`
  - `AdvanceSecureMessaging`
  - `CloseSecureMessaging`
  - `OpenGpSecureChannel`
  - `CloseGpSecureChannel`
  - `ResetCard`
- `SystemService`
  - `SetupToolchains`
  - `Doctor`
  - `GetServiceStatus`

## Source of truth

The protobuf source lives at:

- [`crates/jcim-api/proto/jcim/v0_3/service.proto`](../crates/jcim-api/proto/jcim/v0_3/service.proto)
- descriptor set constant: `jcim_api::JCIM_V0_3_DESCRIPTOR_SET`
- compatibility test: [`crates/jcim-api/tests/descriptor_contract.rs`](../crates/jcim-api/tests/descriptor_contract.rs)
- Migration notes: [`migration-0.3.md`](migration-0.3.md)

The descriptor set is exported so CI can assert package name, service membership, and selected
field numbers without maintaining a separate hand-written schema snapshot.

For JCIM 0.3, that protobuf source intentionally remains one governed file rather than being split
across multiple proto files.

## Notable request and response shapes

- `StartSimulationRequest` takes a project selector.
- Project-backed startup is the maintained simulator input.
- `SimulationInfo` is project-backed and reports:
  - simulation id
  - owning project id and path
  - lifecycle status
  - reader and health details
  - ATR, protocol, ISO capability, and session-state summaries
  - installed package metadata
  - recent retained events
- `GetSimulationSessionStateResponse` and `GetCardSessionStateResponse` return typed ISO/IEC 7816
  session state, including channel, selection, and secure-messaging summaries.
- `TransmitRawApdu*` RPCs preserve the raw escape hatch, but the maintained path is `TransmitApdu`
  plus the typed ISO/GP helpers layered above it.
- `ManageChannel*` and secure-messaging RPCs expose the maintained typed session controls for both
  simulation and real-card targets.
- `OpenGpSecureChannel*` and `CloseGpSecureChannel*` expose the automated typed GlobalPlatform
  auth flow for SCP02 and SCP03.
- Simulator-side GP RPCs remain available for advanced backends and diagnostics, but the
  maintained simulator lifecycle is still project-backed start plus simulator lifecycle, ISO, log,
  and APDU operations.
- `InstallCapRequest` also uses a `oneof` input:
  - `project`
  - `cap_path`
- `InstallCapResponse` is typed:
  - effective reader name
  - installed CAP path
  - package name and AID
  - applet list
  - raw diagnostic lines
- `ListPackagesResponse` and `ListAppletsResponse` return both:
  - parsed typed items
  - raw diagnostic lines for operator troubleshooting
- Physical-card flows are intended to be consumed through these typed shapes rather than by parsing helper output text.
- Real-card session state is observational: responses reflect what the adapter can directly observe
  plus JCIM-tracked effects of commands JCIM itself sent.
- The unified Rust `CardConnection` API lives above these service methods; it does not replace or
  alter the `CardService` and `SimulatorService` RPCs.
- `GetServiceStatusResponse` now includes both the socket path and the startup-captured daemon
  binary identity, which the SDK uses to avoid attaching to stale `jcimd` instances after rebuilds.
