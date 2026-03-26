//! Backend adapters and the actor handle used by the local service and embedded callers.
//!
//! # Why this exists
//! JCIM has to present the same async control surface whether it is talking to a native official
//! simulator or a managed wrapper process. This module keeps that orchestration separate from both
//! transport code and backend-launch internals.
//!
//! # Role in the system
//! [`BackendHandle`] is the primary entry point for the local service and embedded callers. The
//! rest of the submodules keep external bundle launching, reply parsing, and actor wiring
//! discoverable without changing the public import path.

mod actor;
mod external;
mod handle;
mod manifest;
mod reply;

pub use handle::{BackendHandle, CardBackend};

#[cfg(test)]
mod tests;
