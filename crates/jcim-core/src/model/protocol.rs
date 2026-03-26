//! Protocol version model shared by the local service and backend control paths.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{JcimError, Result};

/// Version identifier for the local JCIM service and backend control protocols.
///
/// # Why this exists
/// The local service socket and external backend control stream need an explicit compatibility
/// contract.
/// Encoding that contract as a type prevents stringly-typed version checks from drifting.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct ProtocolVersion {
    /// Major protocol version. A different major version is considered incompatible.
    pub major: u16,
    /// Minor protocol version. Minor versions remain compatible within the same major line.
    pub minor: u16,
}

impl ProtocolVersion {
    /// Build a protocol version from explicit major and minor components.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Return the protocol version implemented by the current workspace.
    pub const fn current() -> Self {
        Self::new(1, 0)
    }

    /// Determine whether two protocol versions can speak to each other.
    ///
    /// # Why this exists
    /// Both the local service and the external backend adapter need one shared compatibility rule so that
    /// startup failures and handshake checks remain consistent.
    pub const fn is_compatible_with(self, other: Self) -> bool {
        self.major == other.major
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::current()
    }
}

impl Display for ProtocolVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for ProtocolVersion {
    type Err = JcimError;

    fn from_str(value: &str) -> Result<Self> {
        let Some((major, minor)) = value.split_once('.') else {
            return Err(JcimError::Unsupported(format!(
                "unsupported protocol version: {value}"
            )));
        };
        let major = major
            .parse()
            .map_err(|_| JcimError::Unsupported(format!("invalid protocol version: {value}")))?;
        let minor = minor
            .parse()
            .map_err(|_| JcimError::Unsupported(format!("invalid protocol version: {value}")))?;
        Ok(Self::new(major, minor))
    }
}

impl From<ProtocolVersion> for String {
    fn from(value: ProtocolVersion) -> Self {
        value.to_string()
    }
}

impl TryFrom<String> for ProtocolVersion {
    type Error = JcimError;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}
