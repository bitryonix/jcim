//! Local gRPC control plane for JCIM 0.3.
//!
//! # Why this exists
//! JCIM 0.3 uses one user-local service as its control plane. This crate hosts the local gRPC
//! server that exposes the task-oriented API consumed by the CLI and future desktop UI.
//!
//! # Verification
//! Runtime ownership, stale-socket cleanup, and direct binary startup are guarded by
//! `crates/jcimd/tests/runtime_cleanup.rs` and `crates/jcimd/tests/binary_smoke.rs`.
#![forbid(unsafe_code)]

/// Small blocking helpers used by the daemon runtime.
mod blocking;
/// In-process gRPC adapter implementations for the local app façade.
mod rpc;
/// Unix-domain socket binding and runtime-metadata ownership helpers.
mod server;
/// Translation helpers between protobuf transport types and domain models.
mod translate;

pub use server::{serve_local_service, serve_local_service_until_shutdown};
