# JCIM 0.2

> [!WARNING]
> **JCIM is an internal workbench for Boomlet development, not a finished product.**
> This repository is a work in progress. It is not final, canonical, complete, normative, or
> production-ready. JCIM exists to support the development, simulation, testing, and card
> operations workflow for **Boomlet**, the Java Card applet used in **Boomerang**.

JCIM 0.2 is a local Java Card simulator workbench built around one user-local gRPC service, a
transport-neutral application core, a CAP-first build pipeline, a canonical Rust lifecycle API,
and a thin task-oriented CLI.

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
- `jcim-backends`: official-simulator backend launching and control-stream supervision
- `jcim-api`: local gRPC contract
- `jcim-app`: application services
- `jcim-sdk`: Rust lifecycle API for build, simulator, and real-card workflows
- `jcimd`: local gRPC service
- `jcim-cli`: task-oriented CLI

## Quick start

Create a project:

```sh
cargo run -p jcim-cli -- project new demo --directory ./demo
cd demo
```

Persist machine-local settings and inspect the environment:

```sh
cargo run -p jcim-cli -- system setup --java-bin java
cargo run -p jcim-cli -- system doctor
```

Build the project to a CAP:

```sh
cargo run -p jcim-cli -- build --project .
cargo run -p jcim-cli -- build artifacts --project .
```

Start a simulator from the project:

```sh
cargo run -p jcim-cli -- sim start --project .
cargo run -p jcim-cli -- sim status
```

Start a simulator from a raw CAP:

```sh
cargo run -p jcim-cli -- sim start --cap ./target.cap
```

Send APDUs or reset the running simulation:

```sh
cargo run -p jcim-cli -- sim apdu 00A4040000 --simulation sim-...
cargo run -p jcim-cli -- sim reset --simulation sim-...
```

Inspect physical readers and cards:

```sh
cargo run -p jcim-cli -- card readers
cargo run -p jcim-cli -- card status
cargo run -p jcim-cli -- card install --project .
```

Inspect the local service:

```sh
cargo run -p jcim-cli -- system service status
```

Run the Rust lifecycle demo against the source-backed Satochip example:

```sh
cargo run -p jcim-sdk --example satochip_lifecycle
```

From Rust, `jcim-sdk` also exposes typed ISO/IEC 7816 and GlobalPlatform helpers on top of the
existing card and simulation APDU channels, including typed `SELECT`, `GET STATUS`, and `SET STATUS`
workflows for GP administration.

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

- The maintained simulator path is CAP-first and uses the official Java Card simulator tooling.
- Java source support means: build sources to a CAP, then install that CAP into the same simulator.
- Raw `.cap` files are first-class simulator inputs through the gRPC API and `jcim sim start --cap`.
- Physical-card utilities stay available through `jcim card ...`, but they are secondary to the simulator.
- The Rust SDK includes typed ISO/IEC 7816 and GlobalPlatform helpers, but real-card GP admin
  commands still depend on the card accepting the caller's authenticated state or secure channel.
- On macOS, the official simulator path requires a managed Linux container command through
  `JCIM_SIMULATOR_CONTAINER_CMD`. Linux and Windows use native official-simulator binaries.

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
