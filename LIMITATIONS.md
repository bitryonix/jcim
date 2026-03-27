# Limitations And Next Steps

## Current limitations

- The maintained simulator path depends on the bundled `jcardsim` backend and the vendored Temurin
  runtime archives for supported macOS and Linux hosts.
- CAP artifacts remain build/install artifacts, not a maintained simulator startup input.
- Physical-card flows still depend on PC/SC availability and the bundled helper jars. Unsupported
  hosts may still need an operator-configured Java runtime override.
- The Rust SDK is the supported Rust-facing surface, but it still depends on the local `jcimd` service rather than exposing a direct in-process execution mode.
- Local-service integration coverage depends on an environment that allows Unix-domain socket
  listeners. Repository CI runs those paths on managed Linux and macOS hosts, while some local
  sandboxes may skip them when the OS denies socket creation.
- Typed ISO/IEC 7816 and GlobalPlatform flows are the maintained path, including automated SCP02 and SCP03 secure-channel setup from env-resolved keysets.
- CLI `--json` is stable for the maintained task-oriented command families. Expert simulator
  surfaces such as `jcim sim gp ...` remain available but are not part of the compatibility
  promise until they have deterministic managed-backend coverage.
- Real-card GP secure-channel validation is hardware-gated on purpose. It requires `JCIM_HARDWARE_TESTS=1`, an operator-provided reader via `JCIM_TEST_CARD_READER` when needed, and complete `JCIM_GP_*` keyset environment variables when `JCIM_HARDWARE_GP_TESTS=1` is enabled.
- The GUI is not implemented yet. The gRPC contract is designed for it, but the desktop shell itself is not part of this workspace.
- The project registry is local and file-based. There is no remote sync or multi-user control plane.
- Release readiness currently stops at verification gates. PR CI and release preflight now cover
  dependency, license, and bundled-asset governance, but packaging/publishing automation is still
  intentionally out of scope.

## Near-term next steps

- Add deeper crash-leftover, partial-startup, and repeated cross-process recovery coverage across
  the managed macOS and Linux path.
- Tighten restart and concurrent-simulation validation around the local service boundary with more
  deterministic multi-client and event-stream scenarios.
- Decide whether the simulator GP expert surface should graduate into the stable automation
  contract or remain explicitly outside it.
- Add a desktop shell on top of the local gRPC contract.
