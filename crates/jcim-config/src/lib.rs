//! Runtime and project configuration types and loaders for JCIM 0.2.
//!
//! # Why this exists
//! Configuration needs to stay transport-agnostic so the local service, CLI shell, runtime
//! adapters, and future desktop surfaces can resolve the same project and runtime inputs from one
//! maintained source of truth.
//!
//! # Role in the system
//! [`config::RuntimeConfig`] remains the lower-level backend-facing runtime contract.
//! [`project::ProjectConfig`] and [`project::UserConfig`] form the product-facing manifest and
//! machine-local configuration layers above it.

pub mod config;
pub mod prelude;
pub mod project;
