use crate::apdu::CommandApdu;

use super::constants::*;

/// Build one `GET RESPONSE` command.
pub fn get_response(expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_GET_RESPONSE,
        0x00,
        0x00,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `MANAGE CHANNEL` open command.
pub fn manage_channel_open() -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_MANAGE_CHANNEL,
        0x00,
        0x00,
        Vec::new(),
        Some(1),
    )
}

/// Build one `MANAGE CHANNEL` close command.
pub fn manage_channel_close(channel_number: u8) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_MANAGE_CHANNEL,
        0x80,
        channel_number,
        Vec::new(),
        None,
    )
}

/// Build one `READ BINARY` command using one short file offset.
pub fn read_binary(offset: u16, expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_READ_BINARY,
        (offset >> 8) as u8,
        offset as u8,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `WRITE BINARY` command.
pub fn write_binary(offset: u16, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_WRITE_BINARY,
        (offset >> 8) as u8,
        offset as u8,
        data.to_vec(),
        None,
    )
}

/// Build one `UPDATE BINARY` command.
pub fn update_binary(offset: u16, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_UPDATE_BINARY,
        (offset >> 8) as u8,
        offset as u8,
        data.to_vec(),
        None,
    )
}

/// Build one `ERASE BINARY` command.
pub fn erase_binary(offset: u16, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_ERASE_BINARY,
        (offset >> 8) as u8,
        offset as u8,
        data.to_vec(),
        None,
    )
}

/// Build one `READ RECORD` command.
pub fn read_record(
    record_number: u8,
    reference_control: u8,
    expected_length: usize,
) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_READ_RECORD,
        record_number,
        reference_control,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `UPDATE RECORD` command.
pub fn update_record(record_number: u8, reference_control: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_UPDATE_RECORD,
        record_number,
        reference_control,
        data.to_vec(),
        None,
    )
}

/// Build one `APPEND RECORD` command.
pub fn append_record(record_number: u8, reference_control: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_APPEND_RECORD,
        record_number,
        reference_control,
        data.to_vec(),
        None,
    )
}

/// Build one `SEARCH RECORD` command.
pub fn search_record(
    record_number: u8,
    reference_control: u8,
    data: &[u8],
    expected_length: usize,
) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_SEARCH_RECORD,
        record_number,
        reference_control,
        data.to_vec(),
        Some(expected_length),
    )
}

/// Build one `GET DATA` command.
pub fn get_data(p1: u8, p2: u8, expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_GET_DATA,
        p1,
        p2,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `PUT DATA` command.
pub fn put_data(p1: u8, p2: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(CLA_ISO7816, INS_PUT_DATA, p1, p2, data.to_vec(), None)
}

/// Build one `VERIFY` command.
pub fn verify(reference: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_VERIFY,
        0x00,
        reference,
        data.to_vec(),
        None,
    )
}

/// Build one `CHANGE REFERENCE DATA` command.
pub fn change_reference_data(p1: u8, reference: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_CHANGE_REFERENCE_DATA,
        p1,
        reference,
        data.to_vec(),
        None,
    )
}

/// Build one `RESET RETRY COUNTER` command.
pub fn reset_retry_counter(p1: u8, reference: u8, data: &[u8]) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_RESET_RETRY_COUNTER,
        p1,
        reference,
        data.to_vec(),
        None,
    )
}

/// Build one `INTERNAL AUTHENTICATE` command.
pub fn internal_authenticate(p1: u8, p2: u8, data: &[u8], expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_INTERNAL_AUTHENTICATE,
        p1,
        p2,
        data.to_vec(),
        Some(expected_length),
    )
}

/// Build one `EXTERNAL AUTHENTICATE` command.
pub fn external_authenticate(
    p1: u8,
    p2: u8,
    data: &[u8],
    expected_length: Option<usize>,
) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_EXTERNAL_AUTHENTICATE,
        p1,
        p2,
        data.to_vec(),
        expected_length,
    )
}

/// Build one `GET CHALLENGE` command.
pub fn get_challenge(expected_length: usize) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_GET_CHALLENGE,
        0x00,
        0x00,
        Vec::new(),
        Some(expected_length),
    )
}

/// Build one `ENVELOPE` command.
pub fn envelope(p1: u8, p2: u8, data: &[u8], expected_length: Option<usize>) -> CommandApdu {
    CommandApdu::new(
        CLA_ISO7816,
        INS_ENVELOPE,
        p1,
        p2,
        data.to_vec(),
        expected_length,
    )
}
