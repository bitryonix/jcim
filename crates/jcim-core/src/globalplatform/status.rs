use serde::{Deserialize, Serialize};

use crate::aid::Aid;

/// Registry subset selected by `GET STATUS`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RegistryKind {
    /// Issuer Security Domain only.
    IssuerSecurityDomain,
    /// Applications, including Security Domains.
    Applications,
    /// Executable Load Files only.
    ExecutableLoadFiles,
    /// Executable Load Files and their Executable Modules.
    ExecutableLoadFilesAndModules,
}

impl RegistryKind {
    /// Return the `P1` selector byte used by GlobalPlatform `GET STATUS`.
    pub(crate) fn p1(self) -> u8 {
        match self {
            Self::IssuerSecurityDomain => 0x80,
            Self::Applications => 0x40,
            Self::ExecutableLoadFiles => 0x20,
            Self::ExecutableLoadFilesAndModules => 0x10,
        }
    }
}

/// Page selector used with `GET STATUS`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GetStatusOccurrence {
    /// Retrieve the first or all matching entries.
    FirstOrAll,
    /// Continue after a prior `FirstOrAll` call that returned `63 10`.
    Next,
}

impl GetStatusOccurrence {
    /// Return the `P2` occurrence byte used by GlobalPlatform `GET STATUS`.
    pub(crate) fn p2(self) -> u8 {
        match self {
            Self::FirstOrAll => 0x02,
            Self::Next => 0x03,
        }
    }
}

/// One parsed GlobalPlatform registry entry from `GET STATUS`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Registry subset that produced this entry.
    pub kind: RegistryKind,
    /// Entry AID.
    pub aid: Aid,
    /// Raw life cycle state byte.
    pub life_cycle_state: u8,
    /// Privilege bytes when present.
    pub privileges: Option<[u8; 3]>,
    /// Executable Load File AID when present.
    pub executable_load_file_aid: Option<Aid>,
    /// Associated Security Domain AID when present.
    pub associated_security_domain_aid: Option<Aid>,
    /// Executable Module AIDs when present.
    pub executable_module_aids: Vec<Aid>,
    /// Load File version bytes when present.
    pub load_file_version: Option<Vec<u8>>,
    /// Implicit selection parameters when present.
    pub implicit_selection_parameters: Vec<u8>,
}

/// Parsed result of one `GET STATUS` APDU.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GetStatusResponse {
    /// Requested registry subset.
    pub kind: RegistryKind,
    /// Parsed registry entries.
    pub entries: Vec<RegistryEntry>,
    /// Whether the card indicated `63 10` and expects a follow-up `Next` call.
    pub more_data_available: bool,
}
