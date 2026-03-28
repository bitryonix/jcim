use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::error::{JcimError, Result};

/// Transmission convention declared by the ATR TS byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransmissionConvention {
    /// Direct convention.
    Direct,
    /// Inverse convention.
    Inverse,
}

impl Display for TransmissionConvention {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => f.write_str("direct"),
            Self::Inverse => f.write_str("inverse"),
        }
    }
}

/// Transport protocol advertised by an ATR or active session.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum TransportProtocol {
    /// T=0 byte-oriented half duplex.
    T0,
    /// T=1 block-oriented half duplex.
    T1,
    /// T=2 reserved historical protocol identifier.
    T2,
    /// T=3 reserved historical protocol identifier.
    T3,
    /// T=14 proprietary transport.
    T14,
    /// One explicit protocol number not covered by the named variants.
    Other(u8),
}

impl TransportProtocol {
    /// Parse one protocol code.
    pub const fn from_code(code: u8) -> Self {
        match code {
            0x00 => Self::T0,
            0x01 => Self::T1,
            0x02 => Self::T2,
            0x03 => Self::T3,
            0x0E => Self::T14,
            value => Self::Other(value),
        }
    }

    /// Return the wire protocol code.
    pub const fn code(self) -> u8 {
        match self {
            Self::T0 => 0x00,
            Self::T1 => 0x01,
            Self::T2 => 0x02,
            Self::T3 => 0x03,
            Self::T14 => 0x0E,
            Self::Other(value) => value,
        }
    }

    /// Parse one `T=...` string reported by a card stack.
    pub fn from_status_text(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        let number = trimmed.strip_prefix("T=")?;
        number.parse::<u8>().ok().map(Self::from_code)
    }
}

impl Display for TransportProtocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "T={}", self.code())
    }
}

/// Parsed interface-byte group from an ATR.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AtrInterfaceGroup {
    /// One-based group index.
    pub index: u8,
    /// TAi when present.
    pub ta: Option<u8>,
    /// TBi when present.
    pub tb: Option<u8>,
    /// TCi when present.
    pub tc: Option<u8>,
    /// TDi when present.
    pub td: Option<u8>,
    /// Protocol announced by TDi when present.
    pub protocol: Option<TransportProtocol>,
}

/// Parsed ATR plus retained raw bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Atr {
    /// Raw ATR bytes.
    pub raw: Vec<u8>,
    /// Direct or inverse convention.
    pub convention: TransmissionConvention,
    /// Parsed interface-byte groups.
    pub interface_groups: Vec<AtrInterfaceGroup>,
    /// Historical bytes.
    pub historical_bytes: Vec<u8>,
    /// TCK checksum byte when present.
    pub checksum_tck: Option<u8>,
    /// Protocols declared by the ATR.
    pub protocols: Vec<TransportProtocol>,
}

impl Atr {
    /// Parse one ATR.
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 2 {
            return Err(JcimError::InvalidApdu(
                "ATR must be at least 2 bytes".to_string(),
            ));
        }

        let convention = match raw[0] {
            0x3B => TransmissionConvention::Direct,
            0x3F => TransmissionConvention::Inverse,
            other => {
                return Err(JcimError::InvalidApdu(format!(
                    "unsupported ATR convention byte {:02X}",
                    other
                )));
            }
        };

        let t0 = raw[1];
        let mut y = t0 >> 4;
        let historical_len = usize::from(t0 & 0x0F);
        let mut index = 2usize;
        let mut group_number = 1u8;
        let mut groups = Vec::new();
        let mut protocols = Vec::new();

        loop {
            let mut group = AtrInterfaceGroup {
                index: group_number,
                ta: None,
                tb: None,
                tc: None,
                td: None,
                protocol: None,
            };
            if y & 0x01 != 0 {
                group.ta = Some(required_atr_byte(raw, &mut index, "TAi")?);
            }
            if y & 0x02 != 0 {
                group.tb = Some(required_atr_byte(raw, &mut index, "TBi")?);
            }
            if y & 0x04 != 0 {
                group.tc = Some(required_atr_byte(raw, &mut index, "TCi")?);
            }
            if y & 0x08 != 0 {
                let td = required_atr_byte(raw, &mut index, "TDi")?;
                let protocol = TransportProtocol::from_code(td & 0x0F);
                group.td = Some(td);
                group.protocol = Some(protocol);
                protocols.push(protocol);
                y = td >> 4;
                groups.push(group);
                group_number += 1;
                continue;
            }
            groups.push(group);
            break;
        }

        if protocols.is_empty() {
            protocols.push(TransportProtocol::T0);
        }

        let historical_end = index + historical_len;
        if historical_end > raw.len() {
            return Err(JcimError::InvalidApdu(
                "ATR historical bytes exceeded available input".to_string(),
            ));
        }
        let historical_bytes = raw[index..historical_end].to_vec();
        index = historical_end;

        let checksum_tck = if protocols
            .iter()
            .any(|protocol| *protocol != TransportProtocol::T0)
        {
            Some(required_atr_byte(raw, &mut index, "TCK")?)
        } else {
            None
        };

        if index != raw.len() {
            return Err(JcimError::InvalidApdu(format!(
                "ATR had {} trailing bytes after parsing",
                raw.len() - index
            )));
        }

        Ok(Self {
            raw: raw.to_vec(),
            convention,
            interface_groups: groups,
            historical_bytes,
            checksum_tck,
            protocols,
        })
    }

    /// Return the first protocol declared by the ATR.
    pub fn default_protocol(&self) -> Option<TransportProtocol> {
        self.protocols.first().copied()
    }

    /// Convert the ATR to uppercase hexadecimal.
    pub fn to_hex(&self) -> String {
        hex::encode_upper(&self.raw)
    }
}

/// Read the next ATR byte or report which logical ATR field ran out of input.
fn required_atr_byte(raw: &[u8], index: &mut usize, label: &str) -> Result<u8> {
    let value = raw
        .get(*index)
        .copied()
        .ok_or_else(|| JcimError::InvalidApdu(format!("ATR ended before {}", label)))?;
    *index += 1;
    Ok(value)
}
