//! Shared domain primitives for the JCIM workspace.
//!
//! # Why this exists
//! `jcim-core` carries the small set of value types and shared errors that every other crate
//! depends on. Keeping these types here prevents the local service, runtime, CLI, and card
//! adapters from inventing incompatible copies of the same protocol and card concepts.
//!
//! # Role in the system
//! Reach for [`aid`], [`apdu`], [`iso7816`], [`globalplatform`], [`error`], and [`model`] when
//! another crate needs a stable representation of Java Card identifiers, APDU frames, typed card
//! administration commands, or JCIM control-plane state.
//!
//! # Examples
//! A CLI command can parse a package AID and build an install request without pulling in runtime
//! or transport code:
//!
//! ```rust
//! use jcim_core::aid::Aid;
//! use jcim_core::model::{InstallDisposition, InstallRequest};
//!
//! let package_aid = Aid::from_hex("A00000006203010C01")?;
//! let install = InstallRequest::new(vec![0xCA, 0xFE], InstallDisposition::KeepUnselectable);
//! assert_eq!(package_aid.to_hex(), "A00000006203010C01");
//! assert!(!install.make_selectable());
//! # Ok::<(), jcim_core::error::JcimError>(())
//! ```

pub mod aid;
pub mod apdu;
pub mod error;
pub mod globalplatform;
pub mod iso7816;
pub mod model;
pub mod prelude;
