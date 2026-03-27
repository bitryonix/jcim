# jcim-sdk Architecture

## Intent

`jcim-sdk` is the canonical Rust lifecycle API for the JCIM 0.3 service-first platform.

## Main modules

- `src/client/bootstrap.rs`: local-service discovery, startup, restart preparation, and binary identity checks
- `src/client/workspace.rs`, `projects.rs`, `build.rs`, `simulations.rs`, `cards.rs`, `system.rs`: per-domain client methods on `JcimClient`
- `src/client/proto.rs`: selector construction plus protobuf-to-domain and domain-to-protobuf translation helpers
- `src/client/mod.rs`: `JcimClient`, shared connection entrypoints, and module wiring

## Dependency direction

- the SDK depends on `jcim-api` for the maintained local contract
- the SDK hides transport/bootstrap details behind `JcimClient`
- typed ISO/IEC 7816 and GlobalPlatform helpers stay in `jcim-core`

## Design notes

- the SDK remains service-first rather than embedding application logic locally
- `JcimClient` stays as the stable public surface while the implementation is split internally by domain
- bootstrap/restart logic is isolated from routine project/build/sim/card/system workflows
- protobuf and transport-edge translation stay in one helper module so the public Rust types remain transport-neutral
