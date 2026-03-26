# jcim-api Architecture

## Intent

`jcim-api` owns the public local-service contract for JCIM 0.2.

## Structure

- `proto/jcim/v0_2/service.proto`: source-of-truth protobuf schema
- `build.rs`: protobuf/gRPC code generation
- `src/lib.rs`: generated-code façade

## Design notes

- The API is task-oriented and GUI-ready.
- It is intentionally coarse-grained so product shells do not have to reconstruct low-level
  transport flows.
- CLI and future desktop shells should build on this crate instead of inventing separate command
  or transport models.
