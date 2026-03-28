/// Interindustry class byte for ordinary ISO/IEC 7816 commands on the basic channel.
pub const CLA_ISO7816: u8 = 0x00;

/// `SELECT` instruction.
pub const INS_SELECT: u8 = 0xA4;
/// `MANAGE CHANNEL` instruction.
pub const INS_MANAGE_CHANNEL: u8 = 0x70;
/// `GET RESPONSE` instruction.
pub const INS_GET_RESPONSE: u8 = 0xC0;
/// `READ BINARY` instruction.
pub const INS_READ_BINARY: u8 = 0xB0;
/// `WRITE BINARY` instruction.
pub const INS_WRITE_BINARY: u8 = 0xD0;
/// `UPDATE BINARY` instruction.
pub const INS_UPDATE_BINARY: u8 = 0xD6;
/// `ERASE BINARY` instruction.
pub const INS_ERASE_BINARY: u8 = 0x0E;
/// `READ RECORD` instruction.
pub const INS_READ_RECORD: u8 = 0xB2;
/// `UPDATE RECORD` instruction.
pub const INS_UPDATE_RECORD: u8 = 0xDC;
/// `APPEND RECORD` instruction.
pub const INS_APPEND_RECORD: u8 = 0xE2;
/// `SEARCH RECORD` instruction.
pub const INS_SEARCH_RECORD: u8 = 0xA2;
/// `GET DATA` instruction.
pub const INS_GET_DATA: u8 = 0xCA;
/// `PUT DATA` instruction.
pub const INS_PUT_DATA: u8 = 0xDA;
/// `VERIFY` instruction.
pub const INS_VERIFY: u8 = 0x20;
/// `CHANGE REFERENCE DATA` instruction.
pub const INS_CHANGE_REFERENCE_DATA: u8 = 0x24;
/// `RESET RETRY COUNTER` instruction.
pub const INS_RESET_RETRY_COUNTER: u8 = 0x2C;
/// `INTERNAL AUTHENTICATE` instruction.
pub const INS_INTERNAL_AUTHENTICATE: u8 = 0x88;
/// `EXTERNAL AUTHENTICATE` instruction.
pub const INS_EXTERNAL_AUTHENTICATE: u8 = 0x82;
/// `GET CHALLENGE` instruction.
pub const INS_GET_CHALLENGE: u8 = 0x84;
/// `ENVELOPE` instruction.
pub const INS_ENVELOPE: u8 = 0xC2;
