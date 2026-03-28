# JCIM 0.3

> [!WARNING]
> **JCIM is an internal workbench for Boomlet development, not a finished product.**
> This repository is a work in progress. It is not final, canonical, complete, normative, or
> production-ready. JCIM exists to support the development, simulation, testing, and card
> operations workflow for **Boomlet**, the Java Card applet used in **Boomerang**.

JCIM 0.3 is a local Java Card simulator workbench built around one user-local gRPC service, a
transport-neutral application core, a managed class-backed simulator pipeline, a canonical Rust
lifecycle API, and a thin task-oriented CLI.

## Compatibility baseline

- protobuf package: `jcim.v0_3`
- CLI JSON schema: `jcim-cli.v2`
- managed files: `jcim.toml`, `config.toml`, `projects.toml`, `jcimd.runtime.toml`
- supported maintained hosts: Linux/macOS on `x86_64` and `aarch64`
- unsupported-host Java fallback: `jcim system setup --java-bin /path/to/java`
- GP key material must come from environment variables, and JCIM must not log those values or
  snapshot them into JSON errors

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

Hardware-gated physical-card commands require a real PC/SC reader and present card:

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

Machine-local settings live outside the project under a split managed layout:

- macOS:
  - config: `~/Library/Application Support/jcim/config/`
  - durable state: `~/Library/Application Support/jcim/state/`
  - runtime socket/state: `~/Library/Application Support/jcim/run/`
  - logs: `~/Library/Logs/jcim/`
  - cache: `~/Library/Caches/jcim/`
  - extracted runtime assets: `~/Library/Application Support/jcim/data/bundled/`
- Linux:
  - config: `$XDG_CONFIG_HOME/jcim` or `~/.config/jcim/`
  - durable data/assets: `$XDG_DATA_HOME/jcim` or `~/.local/share/jcim/`
  - durable state/logs: `$XDG_STATE_HOME/jcim` or `~/.local/state/jcim/`
  - runtime socket/state: `$XDG_RUNTIME_DIR/jcim` or `~/.local/state/jcim/run/`
  - cache: `$XDG_CACHE_HOME/jcim` or `~/.cache/jcim/`

## Current posture

- The maintained simulator path is project-backed, bundled, and class-backed through `jcardsim`.
- On supported macOS and Linux hosts, JCIM uses a bundled Temurin 11 runtime for builds,
  simulator startup, and bundled helper jars. No Docker, `JCIM_SIMULATOR_CONTAINER_CMD`, or host
  Java install is required for the maintained path.
- Java source support means: build the project, then start the managed simulator from the emitted
  classes, runtime classpath, and simulator metadata.
- CAP artifacts remain first-class build outputs for card install, artifact inspection, and
  debugging, but not as a maintained simulator startup input.
- Physical-card utilities stay available through `jcim card ...`, but they are secondary to the
  simulator, hardware-gated, and limited to directly observed plus JCIM-tracked state.
- The Rust SDK includes typed ISO/IEC 7816 and GlobalPlatform helpers, but real-card GP admin
  commands still depend on the card accepting the caller's authenticated state or secure channel.
- CLI `--json` is a versioned automation surface for maintained task-oriented commands. Success and
  error envelopes now carry `schema_version = "jcim-cli.v2"` plus a stable `kind` marker.
- The maintained local service contract stays in the single governed file
  `crates/jcim-api/proto/jcim/v0_3/service.proto` for the 0.3 cycle.
- Expert simulator control paths such as `jcim sim gp ...` remain available, but they are not part
  of the current automation compatibility guarantee.

## Verification gates

- Pull requests run the Rust correctness matrix on Linux and macOS across the supported `x86_64`
  and `aarch64` host tuples, plus an Ubuntu supply-chain job that executes `cargo audit` and
  `cargo deny check`.
- Pull requests also run the targeted docs/contract/governance smoke tests directly so published
  command snippets, protobuf descriptors, `jcim-app` behavior, JSON envelopes, and bundled-asset
  manifests stay review-blocking.
- Release preflight runs on manual dispatch and version tags from a clean Cargo target directory
  and reruns fmt, clippy, tests, rustdoc, `cargo audit`, `cargo deny check`, and the targeted
  third-party governance tests.
- Local `cargo audit`, `cargo deny`, and raw `cargo metadata` checks can still depend on network
  access and preinstalled subcommands. CI remains the canonical release gate when local sandboxes
  deny crates.io access.
- Bundled and vendored runtime updates under `third_party/` or `bundled-backends/` must update
  `third_party/THIRD_PARTY.toml` in the same change set so provenance, license, checksum, and
  cadence data stay in sync with the shipped artifacts.
- Temporary advisory or license exceptions belong in `deny.toml` with a short reason and an expiry
  date or follow-up issue. Keep that file empty by default and remove exceptions before release.

## Reference docs

- Manifest reference: [`docs/manifest-reference.md`](docs/manifest-reference.md)
- API reference: [`docs/api-reference.md`](docs/api-reference.md)
- CLI reference: [`docs/cli-reference.md`](docs/cli-reference.md)
- System setup: [`docs/system-setup.md`](docs/system-setup.md)
- Improvement roadmap: [`docs/improvement-roadmap.md`](docs/improvement-roadmap.md)
- Architecture overview: [`docs/architecture-overview.md`](docs/architecture-overview.md)
- Contributor guide: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Security policy: [`SECURITY.md`](SECURITY.md)
- Daemon troubleshooting: [`docs/troubleshooting-daemon.md`](docs/troubleshooting-daemon.md)
- Third-party refresh process: [`docs/third-party-refresh.md`](docs/third-party-refresh.md)
- Release/versioning notes: [`docs/release-versioning.md`](docs/release-versioning.md)
- Publish review: [`docs/publish-review.md`](docs/publish-review.md)
- Satochip example: [`examples/satochip/README.md`](examples/satochip/README.md)
- Rust SDK: [`crates/jcim-sdk/README.md`](crates/jcim-sdk/README.md)
- Limitations: [`LIMITATIONS.md`](LIMITATIONS.md)
- Design decisions: [`DESIGNDECISIONS.md`](DESIGNDECISIONS.md)
- Migration notes: [`docs/migration-0.3.md`](docs/migration-0.3.md)
- ADR index: [`docs/adr/README.md`](docs/adr/README.md)
