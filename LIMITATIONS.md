# Limitations And Next Steps

## Current limitations

- The maintained simulator path depends on the official Java Card simulator tooling bundled in the repo.
- On macOS, the official simulator path requires `JCIM_SIMULATOR_CONTAINER_CMD` so JCIM can launch a Linux-hosted simulator process. Without that command, simulator startup is unavailable on macOS.
- Physical-card flows still depend on the local Java runtime, PC/SC availability, and the bundled helper jars.
- The Rust SDK is the supported Rust-facing surface, but it still depends on the local `jcimd` service rather than exposing a direct in-process execution mode.
- Typed ISO/IEC 7816 and GlobalPlatform flows are the maintained path, including automated SCP02 and SCP03 secure-channel setup from env-resolved keysets.
- Real-card GP secure-channel validation is hardware-gated on purpose. It requires `JCIM_HARDWARE_TESTS=1`, an operator-provided reader via `JCIM_TEST_CARD_READER` when needed, and complete `JCIM_GP_*` keyset environment variables when `JCIM_HARDWARE_GP_TESTS=1` is enabled.
- The GUI is not implemented yet. The gRPC contract is designed for it, but the desktop shell itself is not part of this workspace.
- The project registry is local and file-based. There is no remote sync or multi-user control plane.

## Near-term next steps

- Add broader end-to-end simulator coverage in Linux-capable environments where the official simulator can actually run.
- Tighten restart and concurrent-simulation validation around the local service boundary.
- Expand structured output coverage for CLI automation and SDK examples.
- Add a desktop shell on top of the local gRPC contract.
