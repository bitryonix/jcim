# jcim-backends Architecture

## Intent

`jcim-backends` separates simulator execution policy from the local service transport layer.

## Main modules

- `backend`: facade that re-exports the backend surface
- `backend/handle.rs`: public backend trait and actor handle
- `backend/external.rs`: simulator backend supervision and IPC
- `backend/manifest.rs`: bundle manifest parsing
- `backend/reply.rs`: external reply normalization
- `backend/actor.rs`: bounded actor loop
- `prelude`: convenience re-exports

## Dependency direction

- Depends on `jcim-core` and `jcim-config`.
- Does not depend on Unix sockets, CLI code, or PC/SC code.

## Design notes

- A bounded actor channel isolates the single-threaded backend state from async callers.
- The simulator backend is launched through a bundle manifest rather than hard-coded paths.
- The backend manifest carries protocol version, classpath, startup timeout, CAP input, and supported profile metadata.
- The simulator is supervised through a newline-delimited JSON control stream so startup, typed APDU, session-state, power, channel, secure-messaging, and snapshot operations all share one explicit contract.
- The external backend is authoritative for simulator session ownership: each stateful reply returns the updated ISO session state rather than leaving `jcim-app` to infer it from APDU traffic.
