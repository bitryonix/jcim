use serde::{Deserialize, Serialize};

use crate::aid::Aid;
use crate::apdu::CommandApdu;
use crate::error::Result;

use super::commands::{CLA_ISO7816, INS_SELECT};

/// Selected file or application reference.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileSelection {
    /// Selection by DF name or application identifier.
    ByName(Vec<u8>),
    /// Selection by file identifier.
    FileId(u16),
    /// Selection by path bytes.
    Path(Vec<u8>),
}

/// Structured `SELECT` command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SelectCommand {
    /// First selection-parameter byte controlling the selection method.
    pub p1: u8,
    /// Second selection-parameter byte controlling occurrence and return data.
    pub p2: u8,
    /// Decoded file or application target requested by the command.
    pub target: FileSelection,
    /// Optional expected response length advertised by the command.
    pub ne: Option<usize>,
}

/// Decode one `SELECT` APDU into the maintained typed selection model.
pub(super) fn decode_select(apdu: &CommandApdu) -> Result<SelectCommand> {
    let target = match (apdu.p1, apdu.data.len()) {
        (0x04, _) => FileSelection::ByName(apdu.data.clone()),
        (0x08, _) => FileSelection::Path(apdu.data.clone()),
        (_, 2) => FileSelection::FileId(u16::from_be_bytes([apdu.data[0], apdu.data[1]])),
        _ => FileSelection::ByName(apdu.data.clone()),
    };
    Ok(SelectCommand {
        p1: apdu.p1,
        p2: apdu.p2,
        target,
        ne: apdu.ne,
    })
}

/// Build one `SELECT` by DF name or application identifier command.
pub fn select_by_name(aid: &Aid) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_SELECT,
        0x04,
        0x00,
        aid.as_bytes().to_vec(),
        Some(256),
    )
}

/// Build one `SELECT FILE` by file identifier.
pub fn select_file(file_id: u16) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_SELECT,
        0x00,
        0x00,
        file_id.to_be_bytes().to_vec(),
        Some(256),
    )
}

/// Build one `SELECT FILE` by path.
pub fn select_path(path: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_SELECT,
        0x08,
        0x00,
        path.to_vec(),
        Some(256),
    )
}
