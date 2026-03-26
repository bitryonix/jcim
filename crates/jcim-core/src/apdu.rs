//! ISO/IEC 7816 command and response APDU transport types.

use serde::{Deserialize, Serialize};

use crate::error::{JcimError, Result};
use crate::iso7816::StatusWord;

/// Command APDU body encoding mode.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ApduEncoding {
    /// Short-length APDU encoding.
    Short,
    /// Extended-length APDU encoding.
    Extended,
}

impl ApduEncoding {
    /// Return the canonical lowercase label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Extended => "extended",
        }
    }
}

/// Parsed APDU case after length decoding.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CommandApduCase {
    /// Header only.
    Case1,
    /// Header plus expected length in short form.
    Case2Short,
    /// Header plus command data in short form.
    Case3Short,
    /// Header plus command data and expected length in short form.
    Case4Short,
    /// Header plus expected length in extended form.
    Case2Extended,
    /// Header plus command data in extended form.
    Case3Extended,
    /// Header plus command data and expected length in extended form.
    Case4Extended,
}

impl CommandApduCase {
    /// Return the encoding used by this APDU case, if any.
    pub const fn encoding(self) -> Option<ApduEncoding> {
        match self {
            Self::Case1 => None,
            Self::Case2Short | Self::Case3Short | Self::Case4Short => Some(ApduEncoding::Short),
            Self::Case2Extended | Self::Case3Extended | Self::Case4Extended => {
                Some(ApduEncoding::Extended)
            }
        }
    }
}

/// Parsed command APDU with strict short and extended length support.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CommandApdu {
    /// APDU class byte.
    pub cla: u8,
    /// APDU instruction byte.
    pub ins: u8,
    /// First instruction parameter byte.
    pub p1: u8,
    /// Second instruction parameter byte.
    pub p2: u8,
    /// Command data field.
    pub data: Vec<u8>,
    /// Expected response length, when present.
    pub ne: Option<usize>,
    /// Chosen body encoding mode.
    pub encoding: ApduEncoding,
}

impl CommandApdu {
    /// Build one command APDU and infer the narrowest valid encoding.
    pub fn new(cla: u8, ins: u8, p1: u8, p2: u8, data: Vec<u8>, ne: Option<usize>) -> Self {
        let encoding = infer_encoding(data.len(), ne);
        Self {
            cla,
            ins,
            p1,
            p2,
            data,
            ne,
            encoding,
        }
    }

    /// Build one command APDU using one explicit encoding mode.
    pub fn new_with_encoding(
        cla: u8,
        ins: u8,
        p1: u8,
        p2: u8,
        data: Vec<u8>,
        ne: Option<usize>,
        encoding: ApduEncoding,
    ) -> Result<Self> {
        validate_lengths(data.len(), ne, encoding)?;
        Ok(Self {
            cla,
            ins,
            p1,
            p2,
            data,
            ne,
            encoding,
        })
    }

    /// Return the parsed APDU case implied by the current body.
    pub fn apdu_case(&self) -> CommandApduCase {
        match (self.encoding, self.data.is_empty(), self.ne.is_some()) {
            (ApduEncoding::Short, true, false) | (ApduEncoding::Extended, true, false) => {
                CommandApduCase::Case1
            }
            (ApduEncoding::Short, true, true) => CommandApduCase::Case2Short,
            (ApduEncoding::Short, false, false) => CommandApduCase::Case3Short,
            (ApduEncoding::Short, false, true) => CommandApduCase::Case4Short,
            (ApduEncoding::Extended, true, true) => CommandApduCase::Case2Extended,
            (ApduEncoding::Extended, false, false) => CommandApduCase::Case3Extended,
            (ApduEncoding::Extended, false, true) => CommandApduCase::Case4Extended,
        }
    }

    /// Parse one raw command APDU.
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 4 {
            return Err(JcimError::InvalidApdu(
                "command APDU must be at least 4 bytes".to_string(),
            ));
        }

        let cla = raw[0];
        let ins = raw[1];
        let p1 = raw[2];
        let p2 = raw[3];

        if raw.len() == 4 {
            return Ok(Self::new(cla, ins, p1, p2, Vec::new(), None));
        }

        if raw.len() == 5 {
            return parse_short_apdu(cla, ins, p1, p2, raw);
        }

        if raw[4] != 0x00 {
            return parse_short_apdu(cla, ins, p1, p2, raw);
        }

        parse_extended_apdu(cla, ins, p1, p2, raw)
    }

    /// Encode the command APDU to raw bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![self.cla, self.ins, self.p1, self.p2];
        match self.apdu_case() {
            CommandApduCase::Case1 => {}
            CommandApduCase::Case2Short => {
                out.push(short_le(self.ne));
            }
            CommandApduCase::Case3Short => {
                out.push(self.data.len() as u8);
                out.extend_from_slice(&self.data);
            }
            CommandApduCase::Case4Short => {
                out.push(self.data.len() as u8);
                out.extend_from_slice(&self.data);
                out.push(short_le(self.ne));
            }
            CommandApduCase::Case2Extended => {
                out.push(0x00);
                let le = extended_length(self.ne);
                out.extend_from_slice(&le.to_be_bytes());
            }
            CommandApduCase::Case3Extended => {
                out.push(0x00);
                out.extend_from_slice(&(self.data.len() as u16).to_be_bytes());
                out.extend_from_slice(&self.data);
            }
            CommandApduCase::Case4Extended => {
                out.push(0x00);
                out.extend_from_slice(&(self.data.len() as u16).to_be_bytes());
                out.extend_from_slice(&self.data);
                let le = extended_length(self.ne);
                out.extend_from_slice(&le.to_be_bytes());
            }
        }
        out
    }
}

/// Response APDU payload and trailing status word.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ResponseApdu {
    /// Response data bytes returned before the status word.
    pub data: Vec<u8>,
    /// Two-byte status word appended to the response.
    pub sw: u16,
}

impl ResponseApdu {
    /// Parse one raw response APDU.
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 2 {
            return Err(JcimError::InvalidApdu(
                "response APDU must be at least 2 bytes".to_string(),
            ));
        }
        let data = raw[..raw.len() - 2].to_vec();
        let sw = u16::from_be_bytes([raw[raw.len() - 2], raw[raw.len() - 1]]);
        Ok(Self { data, sw })
    }

    /// Build a `0x9000` success response.
    pub fn success(data: Vec<u8>) -> Self {
        Self { data, sw: 0x9000 }
    }

    /// Build one status-only response.
    pub fn status(sw: u16) -> Self {
        Self {
            data: Vec::new(),
            sw,
        }
    }

    /// Return the parsed ISO/IEC 7816 status helper.
    pub fn status_word(&self) -> StatusWord {
        StatusWord::new(self.sw)
    }

    /// Report whether the response completed successfully.
    pub fn is_success(&self) -> bool {
        self.status_word().is_success()
    }

    /// Encode response bytes followed by the trailing status word.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.data.clone();
        out.extend_from_slice(&self.sw.to_be_bytes());
        out
    }
}

/// Parse one short-length command APDU body after the four-byte header.
fn parse_short_apdu(cla: u8, ins: u8, p1: u8, p2: u8, raw: &[u8]) -> Result<CommandApdu> {
    let body = &raw[4..];
    if body.len() == 1 {
        let ne = Some(if body[0] == 0 { 256 } else { body[0] as usize });
        return CommandApdu::new_with_encoding(
            cla,
            ins,
            p1,
            p2,
            Vec::new(),
            ne,
            ApduEncoding::Short,
        );
    }

    let lc = body[0] as usize;
    if lc == 0 {
        return Err(JcimError::InvalidApdu(
            "short APDU with zero Lc is invalid".to_string(),
        ));
    }

    match body.len() {
        len if len == 1 + lc => CommandApdu::new_with_encoding(
            cla,
            ins,
            p1,
            p2,
            body[1..].to_vec(),
            None,
            ApduEncoding::Short,
        ),
        len if len == 2 + lc => {
            let le = body[1 + lc];
            CommandApdu::new_with_encoding(
                cla,
                ins,
                p1,
                p2,
                body[1..1 + lc].to_vec(),
                Some(if le == 0 { 256 } else { le as usize }),
                ApduEncoding::Short,
            )
        }
        _ => Err(JcimError::InvalidApdu(format!(
            "unsupported short APDU length {}",
            raw.len()
        ))),
    }
}

/// Parse one extended-length command APDU body after the five-byte sentinel/header prefix.
fn parse_extended_apdu(cla: u8, ins: u8, p1: u8, p2: u8, raw: &[u8]) -> Result<CommandApdu> {
    if raw.len() < 7 {
        return Err(JcimError::InvalidApdu(
            "extended APDU must include a two-byte length field".to_string(),
        ));
    }

    let ext = &raw[5..];
    if ext.len() == 2 {
        let le = u16::from_be_bytes([ext[0], ext[1]]);
        return CommandApdu::new_with_encoding(
            cla,
            ins,
            p1,
            p2,
            Vec::new(),
            Some(if le == 0 { 65_536 } else { le as usize }),
            ApduEncoding::Extended,
        );
    }

    let lc = u16::from_be_bytes([ext[0], ext[1]]) as usize;
    if lc == 0 {
        return Err(JcimError::InvalidApdu(
            "extended APDU with zero Lc is invalid".to_string(),
        ));
    }

    if ext.len() == 2 + lc {
        return CommandApdu::new_with_encoding(
            cla,
            ins,
            p1,
            p2,
            ext[2..].to_vec(),
            None,
            ApduEncoding::Extended,
        );
    }

    if ext.len() == 4 + lc {
        let le_index = 2 + lc;
        let le = u16::from_be_bytes([ext[le_index], ext[le_index + 1]]);
        return CommandApdu::new_with_encoding(
            cla,
            ins,
            p1,
            p2,
            ext[2..2 + lc].to_vec(),
            Some(if le == 0 { 65_536 } else { le as usize }),
            ApduEncoding::Extended,
        );
    }

    Err(JcimError::InvalidApdu(format!(
        "unsupported extended APDU length {}",
        raw.len()
    )))
}

/// Validate one APDU payload length pair against the selected short or extended encoding.
fn validate_lengths(data_len: usize, ne: Option<usize>, encoding: ApduEncoding) -> Result<()> {
    match encoding {
        ApduEncoding::Short => {
            if data_len > usize::from(u8::MAX) {
                return Err(JcimError::InvalidApdu(format!(
                    "short APDU data length {} exceeds 255 bytes",
                    data_len
                )));
            }
            if let Some(ne) = ne
                && !(1..=256).contains(&ne)
            {
                return Err(JcimError::InvalidApdu(format!(
                    "short APDU Le {} must be in the range 1..=256",
                    ne
                )));
            }
        }
        ApduEncoding::Extended => {
            if data_len > usize::from(u16::MAX) {
                return Err(JcimError::InvalidApdu(format!(
                    "extended APDU data length {} exceeds 65535 bytes",
                    data_len
                )));
            }
            if let Some(ne) = ne
                && !(1..=65_536).contains(&ne)
            {
                return Err(JcimError::InvalidApdu(format!(
                    "extended APDU Le {} must be in the range 1..=65536",
                    ne
                )));
            }
        }
    }
    Ok(())
}

/// Infer the smallest encoding that can carry the requested payload and response lengths.
fn infer_encoding(data_len: usize, ne: Option<usize>) -> ApduEncoding {
    if data_len > usize::from(u8::MAX) || ne.is_some_and(|value| value > 256) {
        ApduEncoding::Extended
    } else {
        ApduEncoding::Short
    }
}

/// Encode one short-form Le value, using `0x00` for the full 256-byte sentinel.
fn short_le(ne: Option<usize>) -> u8 {
    match ne.unwrap_or(256) {
        256 => 0x00,
        value => value as u8,
    }
}

/// Encode one extended-form length value, using `0x0000` for the full 65536-byte sentinel.
fn extended_length(ne: Option<usize>) -> u16 {
    match ne.unwrap_or(65_536) {
        65_536 => 0x0000,
        value => value as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::{ApduEncoding, CommandApdu, CommandApduCase, ResponseApdu};

    #[test]
    fn parses_case_one_and_short_forms() {
        let header_only = CommandApdu::parse(&[0x00, 0xA4, 0x04, 0x00]).expect("parse");
        assert_eq!(header_only.apdu_case(), CommandApduCase::Case1);

        let case_two = CommandApdu::parse(&[0x00, 0xB0, 0x00, 0x00, 0x00]).expect("parse");
        assert_eq!(case_two.encoding, ApduEncoding::Short);
        assert_eq!(case_two.ne, Some(256));

        let case_four = CommandApdu::parse(&[0x80, 0xCA, 0x00, 0x00, 0x03, 0x01, 0x02, 0x03, 0x00])
            .expect("parse");
        assert_eq!(case_four.apdu_case(), CommandApduCase::Case4Short);
        assert_eq!(
            case_four.to_bytes(),
            vec![0x80, 0xCA, 0x00, 0x00, 0x03, 0x01, 0x02, 0x03, 0x00]
        );
    }

    #[test]
    fn parses_extended_apdus() {
        let case_two =
            CommandApdu::parse(&[0x00, 0xC0, 0x00, 0x00, 0x00, 0x02, 0x00]).expect("parse");
        assert_eq!(case_two.apdu_case(), CommandApduCase::Case2Extended);
        assert_eq!(case_two.ne, Some(512));

        let mut case_three_bytes = vec![0x00, 0xDA, 0x00, 0x00, 0x00, 0x01, 0x2C];
        case_three_bytes.extend(std::iter::repeat_n(0xAA, 300));
        let case_three = CommandApdu::parse(&case_three_bytes).expect("parse");
        assert_eq!(case_three.apdu_case(), CommandApduCase::Case3Extended);
        assert_eq!(case_three.data.len(), 300);
        assert_eq!(case_three.to_bytes(), case_three_bytes);

        let mut case_four_bytes = vec![0x00, 0xDB, 0x00, 0x00, 0x00, 0x01, 0x00];
        case_four_bytes.extend(std::iter::repeat_n(0xBB, 256));
        case_four_bytes.extend_from_slice(&[0x01, 0x00]);
        let case_four = CommandApdu::parse(&case_four_bytes).expect("parse");
        assert_eq!(case_four.apdu_case(), CommandApduCase::Case4Extended);
        assert_eq!(case_four.ne, Some(256));
        assert_eq!(case_four.to_bytes(), case_four_bytes);
    }

    #[test]
    fn infers_extended_encoding_when_needed() {
        let apdu = CommandApdu::new(0x00, 0xDA, 0x00, 0x00, vec![0xCC; 300], Some(1024));
        assert_eq!(apdu.encoding, ApduEncoding::Extended);
    }

    #[test]
    fn rejects_invalid_lengths() {
        assert!(
            CommandApdu::new_with_encoding(
                0x00,
                0xA4,
                0x04,
                0x00,
                vec![0x00; 256],
                None,
                ApduEncoding::Short,
            )
            .is_err()
        );
        assert!(CommandApdu::parse(&[0x00, 0xDA, 0x00, 0x00, 0x00, 0x00]).is_err());
        assert!(CommandApdu::parse(&[0x00, 0xDA, 0x00, 0x00, 0x00, 0x01]).is_err());
    }

    #[test]
    fn response_helpers_round_trip() {
        let success = ResponseApdu::success(vec![0xDE, 0xAD]);
        assert!(success.is_success());
        assert_eq!(success.to_bytes(), vec![0xDE, 0xAD, 0x90, 0x00]);
        assert_eq!(
            ResponseApdu::parse(&success.to_bytes()).expect("parse"),
            success
        );
    }
}
