# API Reference

JCIM 0.2 exposes a local gRPC API over a Unix-domain socket.

The canonical Rust consumer of this contract is:

- [`crates/jcim-sdk`](../crates/jcim-sdk/README.md)

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
  - `StreamSimulationEvents`
  - `TransmitApdu`
  - `ResetSimulation`
- `CardService`
  - `ListReaders`
  - `GetCardStatus`
  - `InstallCap`
  - `DeleteItem`
  - `ListPackages`
  - `ListApplets`
  - `TransmitApdu`
  - `ResetCard`
- `SystemService`
  - `SetupToolchains`
  - `Doctor`
  - `GetServiceStatus`

## Source of truth

The protobuf source lives at:

- [`crates/jcim-api/proto/jcim/v0_2/service.proto`](../crates/jcim-api/proto/jcim/v0_2/service.proto)

## Notable request and response shapes

- `StartSimulationRequest` uses a `oneof` input:
  - `project`
  - `cap_path`
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
