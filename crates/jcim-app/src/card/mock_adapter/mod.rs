//! In-memory mock physical-card adapter used by tests and documentation flows.

use super::*;

/// Public mock adapter type and `PhysicalCardAdapter` implementation.
mod adapter;
/// Mock APDU dispatch entrypoint that routes commands to ISO or GP handlers.
mod dispatch;
#[path = "globalplatform.rs"]
/// Mock GlobalPlatform command handlers and secure-channel state updates.
mod gp;
/// Mock inventory and reader-status helpers.
mod inventory;
/// Mock ISO/IEC 7816 command handlers.
mod iso;
/// Shared in-memory mock card state and deterministic test helpers.
mod state;

#[cfg(test)]
mod tests;

pub use self::adapter::MockPhysicalCardAdapter;
use self::state::MockCardState;
