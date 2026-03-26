# jcimd Architecture

## Intent

`jcimd` is the local transport layer for JCIM 0.2.

## Main modules

- `lib.rs`: gRPC service implementation and Unix-domain-socket server bootstrap
- `main.rs`: binary bootstrap

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
