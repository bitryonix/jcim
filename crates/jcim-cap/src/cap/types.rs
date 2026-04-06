use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use jcim_core::aid::Aid;
use jcim_core::error::Result;
use jcim_core::model::CardProfile;

use super::{parser, validation};

/// CAP file version extracted from the manifest or `Header.cap`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CapFileVersion {
    /// Major CAP version component.
    pub major: u8,
    /// Minor CAP version component.
    pub minor: u8,
}

impl CapFileVersion {
    /// Return whether the parsed CAP version is supported by the current parser.
    pub fn is_supported(&self) -> bool {
        (self.major, self.minor) == (2, 1) || (self.major, self.minor) == (2, 2)
    }
}

/// Package import entry from `Import.cap`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ImportedPackage {
    /// Imported package AID.
    pub aid: Aid,
    /// Imported major version.
    pub major: u8,
    /// Imported minor version.
    pub minor: u8,
}

/// Applet metadata discovered in a CAP package.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CapApplet {
    /// Applet AID declared by the CAP package.
    pub aid: Aid,
    /// Offset of the install method in the Method component.
    pub install_method_offset: u16,
    /// Optional human-readable applet name sourced from the manifest.
    pub name: Option<String>,
}

/// Parsed CAP package and selected metadata components.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CapPackage {
    /// CAP file format version.
    pub version: CapFileVersion,
    /// Package AID declared by `Header.cap`.
    pub package_aid: Aid,
    /// Package name sourced from the manifest or inferred from paths.
    pub package_name: String,
    /// Package major version.
    pub package_major: u8,
    /// Package minor version.
    pub package_minor: u8,
    /// Imported package dependencies.
    pub imports: Vec<ImportedPackage>,
    /// Applets declared by the CAP package.
    pub applets: Vec<CapApplet>,
    /// Parsed manifest key-value pairs.
    pub manifest: BTreeMap<String, String>,
    /// Original archive bytes for persistence and hashing.
    pub raw_bytes: Vec<u8>,
}

impl CapPackage {
    /// Read and parse a CAP archive from disk.
    pub fn from_path(path: &Path) -> Result<Self> {
        Self::from_bytes(std::fs::read(path)?)
    }

    /// Parse a CAP archive from in-memory bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        parser::parse_cap_package(bytes)
    }

    /// Validate the CAP against a selected built-in card profile.
    pub fn validate_for_profile(&self, profile: &CardProfile) -> Result<()> {
        validation::validate_for_profile(self, profile)
    }
}
