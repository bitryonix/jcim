# JCIM 0.2

> [!WARNING]
> **JCIM is an internal workbench for Boomlet development, not a finished product.**
> This repository is a work in progress. It is not final, canonical, complete, normative, or
> production-ready. JCIM exists to support the development, simulation, testing, and card
> operations workflow for **Boomlet**, the Java Card applet used in **Boomerang**.

JCIM 0.2 is a local Java Card simulator workbench built around one user-local gRPC service, a
transport-neutral application core, a managed class-backed simulator pipeline, a canonical Rust
lifecycle API, and a thin task-oriented CLI.

## Product shape

- `jcimd`: one local gRPC control-plane service over a Unix-domain socket
- `jcim-app`: transport-neutral application core for project, build, simulator, card, and system flows
- `jcim-api`: protobuf/gRPC contract for the CLI and future GUI
- `jcim-sdk`: canonical Rust lifecycle client over the local service
- `jcim-cli`: thin client shell for the local service

## Active workspace crates

- `jcim-core`: shared errors, AIDs, APDU types, and model values
- `jcim-cap`: CAP parsing and export validation
- `jcim-config`: project manifests and machine-local configuration
- `jcim-build`: Java Card source discovery, CAP builds, and artifact metadata
- `jcim-backends`: managed simulator backend launching and control-stream supervision
- `jcim-api`: local gRPC contract
- `jcim-app`: application services
- `jcim-sdk`: Rust lifecycle API for build, simulator, and real-card workflows
- `jcimd`: local gRPC service
- `jcim-cli`: task-oriented CLI

## Quick start

Create a project:

```sh
cargo run -p jcim-cli -- project new demo --directory ./demo
```

Persist machine-local settings and inspect the environment:

```sh
cargo run -p jcim-cli -- system setup
cargo run -p jcim-cli -- system doctor
```

Build the project to a CAP:

```sh
cargo run -p jcim-cli -- build --project ./demo
cargo run -p jcim-cli -- build artifacts --project ./demo
```

Start a simulator from the project:

```sh
cargo run -p jcim-cli -- sim start --project ./demo
cargo run -p jcim-cli -- sim status
```

Use the maintained typed ISO path against the default demo applet:

```sh
cargo run -p jcim-cli -- sim iso select --aid F00000000101
cargo run -p jcim-cli -- sim reset
cargo run -p jcim-cli -- sim stop
```

Use the raw APDU escape hatch only when you really need exact bytes:

```sh
cargo run -p jcim-cli -- sim apdu 00A4040006F0000000010100
```

When you have a connected PC/SC reader and card, inspect hardware with:

```sh
cargo run -p jcim-cli -- card readers
cargo run -p jcim-cli -- card status --reader "Your Reader Name"
cargo run -p jcim-cli -- card install --project ./demo --reader "Your Reader Name"
```

Inspect the local service:

```sh
cargo run -p jcim-cli -- system service status
```

Run the Rust lifecycle demo against the source-backed Satochip example:

```sh
cargo run -p jcim-sdk --example satochip_lifecycle
```

Run the Rust wallet/bootstrap/signing demo against a fresh virtual Satochip:

```sh
cargo run -p jcim-sdk --example satochip_wallet
```

Notes:

- These commands are written to be run from the repository root.
- Simulation commands omit `--simulation` only because the quick start creates exactly one running
  simulation. If you have more than one, pass the id from `cargo run -p jcim-cli -- sim status`.
- Physical-card commands require a real PC/SC reader, a present card, and a reader name from
  `cargo run -p jcim-cli -- card readers`.
- If your real-card install path requires authenticated GP administration, configure
  `JCIM_GP_DEFAULT_KEYSET` plus the matching `JCIM_GP_<NAME>_{MODE,ENC,MAC,DEK}` environment
  variables before running the install or reader-backed wallet flows.

From Rust, `jcim-sdk` exposes one unified `CardConnection` surface for APDU traffic against either
real readers or virtual simulations, plus the existing typed ISO/IEC 7816 and GlobalPlatform
helpers on `JcimClient` for higher-level admin workflows.

## Manifest model

`jcim.toml` is the project-facing manifest.

- `[project]`: name, profile, package metadata, and applets
- `[source]`: source roots
- `[build]`: `native` or `command`, CAP output, version, and dependencies
- `[simulator]`: auto-build and reset defaults for simulator startup
- `[card]`: physical-card defaults

Machine-local settings live outside the project under the managed JCIM root:

- macOS: `~/Library/Application Support/jcim/`
- Linux: `$XDG_DATA_HOME/jcim` or `~/.local/share/jcim/`

## Current posture

- The maintained simulator path is project-backed, bundled, and class-backed through `jcardsim`.
- On macOS and Linux, JCIM uses a bundled Temurin 11 runtime for builds, simulator startup, and
  bundled helper jars. No Docker, `JCIM_SIMULATOR_CONTAINER_CMD`, or host Java install is required
  for the maintained path.
- Java source support means: build the project, then start the managed simulator from the emitted
  classes, runtime classpath, and simulator metadata.
- CAP artifacts remain first-class build outputs for card install, artifact inspection, and
  debugging, but not as a maintained simulator startup input.
- Physical-card utilities stay available through `jcim card ...`, but they are secondary to the
  simulator, hardware-gated, and limited to directly observed plus JCIM-tracked state.
- The Rust SDK includes typed ISO/IEC 7816 and GlobalPlatform helpers, but real-card GP admin
  commands still depend on the card accepting the caller's authenticated state or secure channel.

## Reference docs

- Manifest reference: [`docs/manifest-reference.md`](docs/manifest-reference.md)
- API reference: [`docs/api-reference.md`](docs/api-reference.md)
- CLI reference: [`docs/cli-reference.md`](docs/cli-reference.md)
- System setup: [`docs/system-setup.md`](docs/system-setup.md)
- Satochip example: [`examples/satochip/README.md`](examples/satochip/README.md)
- Rust SDK: [`crates/jcim-sdk/README.md`](crates/jcim-sdk/README.md)
- Limitations: [`LIMITATIONS.md`](LIMITATIONS.md)
- Design decisions: [`DESIGNDECISIONS.md`](DESIGNDECISIONS.md)
- ADR index: [`docs/adr/README.md`](docs/adr/README.md)
