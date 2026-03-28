use serde::{Deserialize, Serialize};

/// Card life cycle state coding used with `SET STATUS` for the issuer security domain.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CardLifeCycle {
    /// OP_READY (`01`).
    OpReady,
    /// INITIALIZED (`07`).
    Initialized,
    /// SECURED (`0F`).
    #[default]
    Secured,
    /// CARD_LOCKED (`7F`).
    CardLocked,
    /// TERMINATED (`FF`).
    Terminated,
}

impl CardLifeCycle {
    pub(crate) fn state_control(self) -> u8 {
        match self {
            Self::OpReady => 0x01,
            Self::Initialized => 0x07,
            Self::Secured => 0x0F,
            Self::CardLocked => 0x7F,
            Self::Terminated => 0xFF,
        }
    }
}

/// Lock transition used for Applications or Security Domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LockTransition {
    /// Transition to the locked state.
    Lock,
    /// Transition from the locked state back to the previous state.
    Unlock,
}

impl LockTransition {
    pub(crate) fn state_control(self) -> u8 {
        match self {
            Self::Lock => 0x80,
            Self::Unlock => 0x00,
        }
    }
}
