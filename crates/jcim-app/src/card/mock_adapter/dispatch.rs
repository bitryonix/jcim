#![allow(clippy::missing_docs_in_private_items)]

use super::*;

use super::gp::{
    mock_get_status_response, mock_gp_external_authenticate_response,
    mock_initialize_update_response, mock_set_status_response,
};
use super::iso::mock_iso_response;

pub(super) fn mock_dispatch_apdu(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> Result<ResponseApdu> {
    if !mock_supported_cla(apdu.cla) {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::CLASS_NOT_SUPPORTED.as_u16(),
        ));
    }
    let logical_channel = iso7816::logical_channel_from_cla(apdu.cla);
    if apdu.ins != iso7816::INS_MANAGE_CHANNEL
        && logical_channel != 0
        && state
            .session_state
            .open_channels
            .iter()
            .all(|entry| entry.channel_number != logical_channel)
    {
        return Ok(ResponseApdu::status(0x6881));
    }

    match (apdu.cla, apdu.ins) {
        (0x80, 0xF2) => mock_get_status_response(state, apdu.p1, apdu.p2),
        (0x80, 0xF0) => mock_set_status_response(state, apdu),
        (0x80, 0x50) => mock_initialize_update_response(state, apdu),
        (0x80, 0x82) => mock_gp_external_authenticate_response(state, apdu),
        _ => mock_iso_response(state, apdu),
    }
}

fn mock_supported_cla(cla: u8) -> bool {
    cla == 0x80 || cla & 0x80 == 0
}
