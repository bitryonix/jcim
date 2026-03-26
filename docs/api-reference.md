# API Reference

JCIM 0.2 exposes a local gRPC API over a Unix-domain socket.

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

- [`crates/jcim-api/proto/jcim/v0_2/service.proto`](../crates/jcim-api/proto/jcim/v0_2/service.proto)

## Notable request and response shapes

- `StartSimulationRequest` takes a project selector.
- Project-backed startup is the maintained simulator input.
- `GetSimulationSessionStateResponse` and `GetCardSessionStateResponse` return typed ISO/IEC 7816
  session state, including channel, selection, and secure-messaging summaries.
- `TransmitRawApdu*` RPCs preserve the raw escape hatch, but the maintained path is `TransmitApdu`
  plus the typed ISO/GP helpers layered above it.
- `ManageChannel*` and secure-messaging RPCs expose the maintained typed session controls for both
  simulation and real-card targets.
- `OpenGpSecureChannel*` and `CloseGpSecureChannel*` expose the automated typed GlobalPlatform
  auth flow for SCP02 and SCP03.
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
- New simulations report `managed_java` as the maintained engine mode. Older `native` and
  `container` values remain decode-compatible for compatibility.
- `GetServiceStatusResponse` now includes both the socket path and the startup-captured daemon
  binary identity, which the SDK uses to avoid attaching to stale `jcimd` instances after rebuilds.
