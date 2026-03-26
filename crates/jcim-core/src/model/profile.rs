//! Card profile types shared by configuration, runtime, and diagnostics code.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{JcimError, Result};

/// Supported Java Card Classic platform versions.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum JavaCardClassicVersion {
    /// Java Card 2.1.
    V2_1,
    /// Java Card 2.1.1.
    V2_1_1,
    /// Java Card 2.2.
    V2_2,
    /// Java Card 2.2.1.
    V2_2_1,
    /// Java Card 2.2.2.
    V2_2_2,
    /// Java Card 3.0.1 Classic Edition.
    V3_0_1,
    /// Java Card 3.0.4 Classic Edition.
    V3_0_4,
    /// Java Card 3.0.5 Classic Edition.
    V3_0_5,
}

impl JavaCardClassicVersion {
    /// Return the operator-facing version string.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::V2_1 => "2.1",
            Self::V2_1_1 => "2.1.1",
            Self::V2_2 => "2.2",
            Self::V2_2_1 => "2.2.1",
            Self::V2_2_2 => "2.2.2",
            Self::V3_0_1 => "3.0.1",
            Self::V3_0_4 => "3.0.4",
            Self::V3_0_5 => "3.0.5",
        }
    }

    /// Report whether this Java Card version accepts the given CAP minor version.
    pub fn supports_cap_minor(self, minor: u8) -> bool {
        match self {
            Self::V2_1 | Self::V2_1_1 => minor == 1,
            Self::V2_2
            | Self::V2_2_1
            | Self::V2_2_2
            | Self::V3_0_1
            | Self::V3_0_4
            | Self::V3_0_5 => minor == 1 || minor == 2,
        }
    }
}

impl Display for JavaCardClassicVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

impl FromStr for JavaCardClassicVersion {
    type Err = JcimError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "2.1" => Ok(Self::V2_1),
            "2.1.1" => Ok(Self::V2_1_1),
            "2.2" => Ok(Self::V2_2),
            "2.2.1" => Ok(Self::V2_2_1),
            "2.2.2" => Ok(Self::V2_2_2),
            "3.0.1" => Ok(Self::V3_0_1),
            "3.0.4" => Ok(Self::V3_0_4),
            "3.0.5" => Ok(Self::V3_0_5),
            _ => Err(JcimError::Unsupported(format!(
                "unsupported Java Card version: {value}"
            ))),
        }
    }
}

/// Stable identifiers for builtin JCIM card profiles.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum CardProfileId {
    /// Builtin 2.1 profile family.
    Classic21,
    /// Builtin 2.1.1 profile family.
    Classic211,
    /// Builtin 2.2 profile family.
    Classic22,
    /// Builtin 2.2.1 profile family.
    Classic221,
    /// Builtin 2.2.2 profile family.
    Classic222,
    /// Builtin 3.0.1 profile family.
    Classic301,
    /// Builtin 3.0.4 profile family.
    Classic304,
    /// Builtin 3.0.5 profile family.
    Classic305,
}

impl CardProfileId {
    /// Return the Java Card version implemented by this builtin profile.
    pub fn version(self) -> JavaCardClassicVersion {
        match self {
            Self::Classic21 => JavaCardClassicVersion::V2_1,
            Self::Classic211 => JavaCardClassicVersion::V2_1_1,
            Self::Classic22 => JavaCardClassicVersion::V2_2,
            Self::Classic221 => JavaCardClassicVersion::V2_2_1,
            Self::Classic222 => JavaCardClassicVersion::V2_2_2,
            Self::Classic301 => JavaCardClassicVersion::V3_0_1,
            Self::Classic304 => JavaCardClassicVersion::V3_0_4,
            Self::Classic305 => JavaCardClassicVersion::V3_0_5,
        }
    }

    /// Return the operator-facing display name used in CLIs and docs.
    pub fn display_name(self) -> &'static str {
        self.version().display_name()
    }
}

impl Display for CardProfileId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Classic21 => "classic21",
            Self::Classic211 => "classic211",
            Self::Classic22 => "classic22",
            Self::Classic221 => "classic221",
            Self::Classic222 => "classic222",
            Self::Classic301 => "classic301",
            Self::Classic304 => "classic304",
            Self::Classic305 => "classic305",
        };
        f.write_str(value)
    }
}

impl FromStr for CardProfileId {
    type Err = JcimError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "classic21" => Ok(Self::Classic21),
            "classic211" => Ok(Self::Classic211),
            "classic22" => Ok(Self::Classic22),
            "classic221" => Ok(Self::Classic221),
            "classic222" => Ok(Self::Classic222),
            "classic301" => Ok(Self::Classic301),
            "classic304" => Ok(Self::Classic304),
            "classic305" => Ok(Self::Classic305),
            _ => Err(JcimError::Unsupported(format!(
                "unsupported profile id: {value}"
            ))),
        }
    }
}

/// Memory budget and wear characteristics for a card profile.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct MemoryLimits {
    /// Persistent EEPROM or flash budget in bytes.
    pub persistent_bytes: usize,
    /// CLEAR_ON_RESET transient memory in bytes.
    pub transient_reset_bytes: usize,
    /// CLEAR_ON_DESELECT transient memory in bytes.
    pub transient_deselect_bytes: usize,
    /// Maximum APDU buffer size in bytes.
    pub apdu_buffer_bytes: usize,
    /// Commit buffer size in bytes.
    pub commit_buffer_bytes: usize,
    /// Scratch space reserved for installation workflows.
    pub install_scratch_bytes: usize,
    /// Approximate stack budget in bytes.
    pub stack_bytes: usize,
    /// Flash or EEPROM page size in bytes.
    pub page_bytes: usize,
    /// Erase block size in bytes.
    pub erase_block_bytes: usize,
    /// Journal or transaction log budget in bytes.
    pub journal_bytes: usize,
    /// Optional wear limit for persistent storage erase cycles.
    pub wear_limit: Option<u64>,
}

/// Hardware-facing characteristics exposed by a card profile.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HardwareProfile {
    /// Human-readable hardware family name.
    pub name: String,
    /// ATR returned after reset or power on.
    pub atr: Vec<u8>,
    /// Memory and wear characteristics for the profile.
    pub memory: MemoryLimits,
    /// Maximum supported APDU size in bytes.
    pub max_apdu_size: usize,
    /// Whether the profile is expected to support SCP02.
    pub supports_scp02: bool,
    /// Whether the profile is expected to support SCP03.
    pub supports_scp03: bool,
}

/// Builtin card profile description used by runtimes, CLIs, and diagnostics.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CardProfile {
    /// Stable profile identifier.
    pub id: CardProfileId,
    /// Java Card platform version represented by this profile.
    pub version: JavaCardClassicVersion,
    /// Reader name reported by default for this profile.
    pub reader_name: String,
    /// Hardware-facing properties for the emulated card.
    pub hardware: HardwareProfile,
}

impl CardProfile {
    /// Build one of the maintained builtin JCIM profiles.
    pub fn builtin(id: CardProfileId) -> Self {
        match id {
            // The 2.1 family shares the same conservative memory shape because older cards had
            // tighter transient-memory budgets and no SCP03 support.
            CardProfileId::Classic21 | CardProfileId::Classic211 => Self {
                id,
                version: id.version(),
                reader_name: format!("JCIM Classic {} Reader", id.display_name()),
                hardware: HardwareProfile {
                    name: "180 KB dev card".to_string(),
                    atr: vec![0x3B, 0x80, 0x80, 0x01, 0x21],
                    memory: MemoryLimits {
                        persistent_bytes: 180 * 1024,
                        transient_reset_bytes: 8 * 1024,
                        transient_deselect_bytes: 4 * 1024,
                        apdu_buffer_bytes: 261,
                        commit_buffer_bytes: 2 * 1024,
                        install_scratch_bytes: 12 * 1024,
                        stack_bytes: 8 * 1024,
                        page_bytes: 128,
                        erase_block_bytes: 512,
                        journal_bytes: 2 * 1024,
                        wear_limit: Some(100_000),
                    },
                    max_apdu_size: 261,
                    supports_scp02: true,
                    supports_scp03: false,
                },
            },
            // The 2.2 family keeps APDU sizes small but increases persistent and commit budgets to
            // match later GlobalPlatform-era developer cards.
            CardProfileId::Classic22 | CardProfileId::Classic221 | CardProfileId::Classic222 => {
                Self {
                    id,
                    version: id.version(),
                    reader_name: format!("JCIM Classic {} Reader", id.display_name()),
                    hardware: HardwareProfile {
                        name: "256 KB dev card".to_string(),
                        atr: vec![0x3B, 0x80, 0x80, 0x01, 0x22],
                        memory: MemoryLimits {
                            persistent_bytes: 256 * 1024,
                            transient_reset_bytes: 16 * 1024,
                            transient_deselect_bytes: 4 * 1024,
                            apdu_buffer_bytes: 261,
                            commit_buffer_bytes: 4 * 1024,
                            install_scratch_bytes: 24 * 1024,
                            stack_bytes: 12 * 1024,
                            page_bytes: 256,
                            erase_block_bytes: 1024,
                            journal_bytes: 4 * 1024,
                            wear_limit: Some(150_000),
                        },
                        max_apdu_size: 261,
                        supports_scp02: true,
                        supports_scp03: true,
                    },
                }
            }
            // The 3.x family models newer cards with larger APDU buffers and more generous install
            // scratch space so CAP and snapshot workflows can exercise higher-fidelity limits.
            CardProfileId::Classic301 | CardProfileId::Classic304 | CardProfileId::Classic305 => {
                Self {
                    id,
                    version: id.version(),
                    reader_name: format!("JCIM Classic {} Reader", id.display_name()),
                    hardware: HardwareProfile {
                        name: "512 KB dev card".to_string(),
                        atr: vec![0x3B, 0x80, 0x80, 0x01, 0x30, 0x05],
                        memory: MemoryLimits {
                            persistent_bytes: 512 * 1024,
                            transient_reset_bytes: 32 * 1024,
                            transient_deselect_bytes: 8 * 1024,
                            apdu_buffer_bytes: 2048,
                            commit_buffer_bytes: 8 * 1024,
                            install_scratch_bytes: 64 * 1024,
                            stack_bytes: 32 * 1024,
                            page_bytes: 512,
                            erase_block_bytes: 2048,
                            journal_bytes: 8 * 1024,
                            wear_limit: Some(250_000),
                        },
                        max_apdu_size: 2048,
                        supports_scp02: true,
                        supports_scp03: true,
                    },
                }
            }
        }
    }

    /// Return the profile identifier while keeping examples readable.
    pub const fn profile_id(&self) -> CardProfileId {
        self.id
    }

    /// Report whether this profile accepts the given CAP minor version.
    pub fn supports_cap_minor(&self, minor: u8) -> bool {
        self.version.supports_cap_minor(minor)
    }
}
