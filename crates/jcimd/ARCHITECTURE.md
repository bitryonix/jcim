# jcimd Architecture

## Intent

`jcimd` is the local transport layer for JCIM 0.3.

## Main modules

- `lib.rs`: crate wiring and public server exports
- `main.rs`: binary bootstrap
- `server.rs`: Unix-domain-socket bootstrap, runtime metadata ownership, and shutdown cleanup
- `rpc/`: one tonic service implementation module per public service
- `blocking.rs`: shared `spawn_blocking` bridge for sync app operations
- `translate.rs`: proto/domain translation, selector mapping, and transport-edge status mapping

## Dependency direction

- `jcimd` depends on:
  - `jcim-api` for the contract
  - `jcim-app` for application behavior
- transport details stay here
- project/build/sim/card/system policy stays in `jcim-app`

## Design notes

- one local service manages many projects and many simulations
- the service contract is task-oriented rather than transport-internal
- Unix-domain sockets are the local transport on macOS and Linux
- startup writes a runtime metadata file next to the managed socket so stale-socket cleanup and
  binary-identity checks can fail closed instead of unlinking blindly
- service impls stay transport-focused while project/build/sim/card/system policy remains in
  `jcim-app`
