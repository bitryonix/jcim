# jcim-core Architecture

## Intent

`jcim-core` is the innermost crate. It defines the shared language of the workspace and avoids
transport, filesystem, process, and simulator-policy concerns.

## Main modules

- `aid`: AID parsing, formatting, and serde support
- `apdu`: short-form command/response APDU parsing helpers
- `error`: shared error type
- `model`: public shared value types used across configuration, simulation, backend, and protocol
- `prelude`: convenience re-exports for consumers that prefer a flatter import surface

## Dependency direction

- Nothing in this crate depends on `tokio`, `clap`, process spawning, config file loading, CAP
  parsing, wire framing, or any specific simulator implementation.
- All outer crates may depend on this crate.
- This crate is intentionally boring because it is the shared dependency root.

## Design notes

- `#[non_exhaustive]` is used selectively on growth-prone enums, not broadly on data structs that
  callers need to construct.
- Public types are chosen to encode invariants without pulling in application-layer dependencies.
