//! Backend adapters for the maintained JCIM simulator process.
//!
//! # Why this exists
//! The local service and embedded callers both need a uniform control surface for the maintained
//! CAP-first simulator bundle. This crate isolates that orchestration from transport code and from
//! the backend implementation itself.
//!
//! # Role in the system
//! Start with [`backend::BackendHandle`] when a caller needs an async façade over the external
//! simulator backend process.

pub mod backend;
pub mod prelude;
