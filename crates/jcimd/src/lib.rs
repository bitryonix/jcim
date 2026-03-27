//! Local gRPC control plane for JCIM 0.3.
//!
//! # Why this exists
//! JCIM 0.3 uses one user-local service as its control plane. This crate hosts the local gRPC
//! server that exposes the task-oriented API consumed by the CLI and future desktop UI.
#![allow(clippy::missing_docs_in_private_items)]
#![forbid(unsafe_code)]

mod blocking;
mod rpc;
mod server;
mod translate;

pub use server::{serve_local_service, serve_local_service_until_shutdown};
