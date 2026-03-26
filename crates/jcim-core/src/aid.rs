//! Application identifier helpers.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{JcimError, Result};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
/// Java Card application identifier.
pub struct Aid(Vec<u8>);

impl Aid {
    /// Create an AID from raw bytes.
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > 16 {
            return Err(JcimError::InvalidAid(format!(
                "AID length must be between 1 and 16 bytes, got {}",
                bytes.len()
            )));
        }
        Ok(Self(bytes))
    }

    /// Create an AID from a borrowed byte slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        Self::new(bytes.to_vec())
    }

    /// Parse an AID from uppercase or lowercase hexadecimal.
    pub fn from_hex(value: &str) -> Result<Self> {
        let cleaned = value
            // Vendor-generated manifests often spell AIDs as `0xAA:0xBB:...`. Stripping the byte
            // prefix first lets JCIM accept those archives without relaxing the underlying hex
            // decoding rules for ordinary callers.
            .replace("0x", "")
            .replace("0X", "")
            .chars()
            .filter(|ch| !ch.is_whitespace() && *ch != ':')
            .collect::<String>();
        let bytes = hex::decode(&cleaned)
            .map_err(|error| JcimError::InvalidAid(format!("invalid hex AID: {error}")))?;
        Self::new(bytes)
    }

    /// Borrow the encoded AID bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Render the AID as uppercase hexadecimal.
    pub fn to_hex(&self) -> String {
        hex::encode_upper(&self.0)
    }
}

impl FromStr for Aid {
    type Err = JcimError;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_hex(s)
    }
}

impl TryFrom<&[u8]> for Aid {
    type Error = JcimError;

    fn try_from(value: &[u8]) -> Result<Self> {
        Self::from_slice(value)
    }
}

impl Display for Aid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Serialize for Aid {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Aid {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Aid::from_hex(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::Aid;

    #[test]
    fn constructs_and_formats_aids() {
        let aid = Aid::new(vec![0xA0, 0x00, 0x00, 0x01]).expect("aid");
        assert_eq!(aid.as_bytes(), &[0xA0, 0x00, 0x00, 0x01]);
        assert_eq!(aid.to_hex(), "A0000001");
        assert_eq!(aid.to_string(), "A0000001");
    }

    #[test]
    fn builds_from_slice_and_try_from() {
        let from_slice = Aid::from_slice(&[0x01, 0x02, 0x03]).expect("from slice");
        let try_from = Aid::try_from(&[0x01, 0x02, 0x03][..]).expect("try_from");
        assert_eq!(from_slice, try_from);
    }

    #[test]
    fn parses_hex_with_whitespace_and_colons() {
        let aid = Aid::from_hex("A0 00:00 00 62 03 01 0C 01").expect("aid");
        assert_eq!(aid.to_hex(), "A00000006203010C01");
        assert_eq!(Aid::from_str("a00000006203010c01").expect("from_str"), aid);
    }

    #[test]
    fn parses_hex_with_0x_prefixed_bytes() {
        let aid =
            Aid::from_hex("0xD0:0x00:0x00:0x00:0x01:0x01:0x01:0x01").expect("aid with prefixes");
        assert_eq!(aid.to_hex(), "D000000001010101");
    }

    #[test]
    fn rejects_invalid_lengths_and_hex() {
        assert!(Aid::new(Vec::new()).is_err());
        assert!(Aid::new(vec![0; 17]).is_err());
        assert!(Aid::from_hex("xyz").is_err());
    }

    #[test]
    fn serializes_and_deserializes_as_uppercase_hex() {
        let aid = Aid::from_hex("A0000000620001").expect("aid");
        let json = serde_json::to_string(&aid).expect("serialize");
        assert_eq!(json, "\"A0000000620001\"");
        let decoded: Aid = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, aid);
    }
}
