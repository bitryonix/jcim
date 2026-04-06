#![allow(clippy::missing_docs_in_private_items)]

use super::*;

use super::state::{mock_deterministic_bytes, mock_selected_file_id};

pub(super) fn mock_iso_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> Result<ResponseApdu> {
    Ok(match apdu.ins {
        iso7816::INS_SELECT => mock_select_response(state, apdu)?,
        iso7816::INS_MANAGE_CHANNEL => mock_manage_channel_response(state, apdu),
        iso7816::INS_GET_RESPONSE => mock_get_response_response(state, apdu),
        iso7816::INS_READ_BINARY => mock_read_binary_response(state, apdu),
        iso7816::INS_WRITE_BINARY | iso7816::INS_UPDATE_BINARY => {
            mock_write_binary_response(state, apdu)
        }
        iso7816::INS_ERASE_BINARY => mock_erase_binary_response(state, apdu),
        iso7816::INS_READ_RECORD => mock_read_record_response(state, apdu),
        iso7816::INS_UPDATE_RECORD => mock_update_record_response(state, apdu),
        iso7816::INS_APPEND_RECORD => mock_append_record_response(state, apdu),
        iso7816::INS_SEARCH_RECORD => mock_search_record_response(state, apdu),
        iso7816::INS_GET_DATA => mock_get_data_response(state, apdu),
        iso7816::INS_PUT_DATA => mock_put_data_response(state, apdu),
        iso7816::INS_VERIFY => mock_verify_response(state, apdu),
        iso7816::INS_CHANGE_REFERENCE_DATA => mock_change_reference_data_response(state, apdu),
        iso7816::INS_RESET_RETRY_COUNTER => mock_reset_retry_counter_response(state, apdu),
        iso7816::INS_INTERNAL_AUTHENTICATE => mock_internal_authenticate_response(state, apdu),
        iso7816::INS_EXTERNAL_AUTHENTICATE => mock_external_authenticate_response(apdu),
        iso7816::INS_GET_CHALLENGE => mock_get_challenge_response(state, apdu),
        iso7816::INS_ENVELOPE => mock_envelope_response(state, apdu),
        _ => ResponseApdu::status(iso7816::StatusWord::INSTRUCTION_NOT_SUPPORTED.as_u16()),
    })
}

fn mock_select_response(state: &MockCardState, apdu: &CommandApdu) -> Result<ResponseApdu> {
    match apdu.p1 {
        0x04 => {
            let requested_aid = hex::encode_upper(&apdu.data);
            if requested_aid == hex::encode_upper(globalplatform::ISSUER_SECURITY_DOMAIN_AID) {
                return Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()));
            }
            if matches!(
                state.card_life_cycle,
                globalplatform::CardLifeCycle::CardLocked
                    | globalplatform::CardLifeCycle::Terminated
            ) {
                return Ok(ResponseApdu::status(
                    iso7816::StatusWord::COMMAND_NOT_ALLOWED.as_u16(),
                ));
            }
            if state.locked_aids.contains(&requested_aid) {
                return Ok(ResponseApdu::status(
                    iso7816::StatusWord::WARNING_SELECTED_FILE_INVALIDATED.as_u16(),
                ));
            }
            if state
                .applets
                .iter()
                .any(|applet| applet.aid == requested_aid)
            {
                return Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()));
            }
            Ok(ResponseApdu::status(
                iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16(),
            ))
        }
        0x00 => {
            if apdu.data.len() != 2 {
                return Ok(ResponseApdu::status(
                    iso7816::StatusWord::WRONG_LENGTH.as_u16(),
                ));
            }
            let file_id = u16::from_be_bytes([apdu.data[0], apdu.data[1]]);
            if state.binary_files.contains_key(&file_id)
                || state.record_files.contains_key(&file_id)
            {
                Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()))
            } else {
                Ok(ResponseApdu::status(
                    iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16(),
                ))
            }
        }
        0x08 => {
            if apdu.data.len() < 2 || !apdu.data.len().is_multiple_of(2) {
                return Ok(ResponseApdu::status(
                    iso7816::StatusWord::WRONG_LENGTH.as_u16(),
                ));
            }
            let end = apdu.data.len();
            let file_id = u16::from_be_bytes([apdu.data[end - 2], apdu.data[end - 1]]);
            if state.binary_files.contains_key(&file_id)
                || state.record_files.contains_key(&file_id)
            {
                Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()))
            } else {
                Ok(ResponseApdu::status(
                    iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16(),
                ))
            }
        }
        _ => Ok(ResponseApdu::status(
            iso7816::StatusWord::INCORRECT_P1_P2.as_u16(),
        )),
    }
}

fn mock_manage_channel_response(state: &MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    if !state.iso_capabilities.logical_channels {
        return ResponseApdu::status(0x6881);
    }
    match apdu.p1 {
        0x00 if apdu.p2 == 0 && apdu.data.is_empty() => {
            for candidate in 1..state.iso_capabilities.max_logical_channels {
                if state
                    .session_state
                    .open_channels
                    .iter()
                    .all(|entry| entry.channel_number != candidate)
                {
                    return ResponseApdu::success(vec![candidate]);
                }
            }
            ResponseApdu::status(0x6A81)
        }
        0x80 if apdu.p2 != 0 && apdu.data.is_empty() => {
            if state
                .session_state
                .open_channels
                .iter()
                .any(|entry| entry.channel_number == apdu.p2)
            {
                ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
            } else {
                ResponseApdu::status(0x6881)
            }
        }
        _ => ResponseApdu::status(iso7816::StatusWord::INCORRECT_P1_P2.as_u16()),
    }
}

fn mock_get_response_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    if apdu.p1 != 0x00 || apdu.p2 != 0x00 {
        return ResponseApdu::status(iso7816::StatusWord::INCORRECT_P1_P2.as_u16());
    }
    if state.pending_response.is_empty() {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    }
    let expected_length = apdu.ne.unwrap_or(256);
    let take = expected_length.min(state.pending_response.len());
    let remaining = state.pending_response.split_off(take);
    let current = std::mem::replace(&mut state.pending_response, remaining);
    if state.pending_response.is_empty() {
        ResponseApdu::success(current)
    } else {
        let hinted = state.pending_response.len().min(256);
        ResponseApdu {
            data: current,
            sw: 0x6100 | if hinted == 256 { 0 } else { hinted as u16 },
        }
    }
}

fn mock_read_binary_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let Some(contents) = state.binary_files.get(&file_id) else {
        return ResponseApdu::status(iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16());
    };
    let offset = u16::from_be_bytes([apdu.p1, apdu.p2]) as usize;
    if offset > contents.len() {
        return ResponseApdu::status(iso7816::StatusWord::INCORRECT_P1_P2.as_u16());
    }
    mock_chunk_response(state, contents[offset..].to_vec(), apdu.ne)
}

fn mock_write_binary_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let offset = u16::from_be_bytes([apdu.p1, apdu.p2]) as usize;
    let entry = state.binary_files.entry(file_id).or_default();
    if entry.len() < offset {
        entry.resize(offset, 0x00);
    }
    let required_len = offset.saturating_add(apdu.data.len());
    if entry.len() < required_len {
        entry.resize(required_len, 0x00);
    }
    entry[offset..required_len].copy_from_slice(&apdu.data);
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_erase_binary_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let offset = u16::from_be_bytes([apdu.p1, apdu.p2]) as usize;
    let entry = state.binary_files.entry(file_id).or_default();
    if offset > entry.len() {
        return ResponseApdu::status(iso7816::StatusWord::INCORRECT_P1_P2.as_u16());
    }
    let erase_len = apdu.data.len().max(1);
    let end = offset.saturating_add(erase_len).min(entry.len());
    for byte in &mut entry[offset..end] {
        *byte = 0x00;
    }
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_read_record_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let Some(records) = state.record_files.get(&file_id) else {
        return ResponseApdu::status(iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16());
    };
    let record_number = usize::from(apdu.p1);
    if record_number == 0 || record_number > records.len() {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    mock_chunk_response(state, records[record_number - 1].clone(), apdu.ne)
}

fn mock_update_record_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let Some(records) = state.record_files.get_mut(&file_id) else {
        return ResponseApdu::status(iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16());
    };
    let record_number = usize::from(apdu.p1);
    if record_number == 0 || record_number > records.len() {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    records[record_number - 1] = apdu.data.clone();
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_append_record_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    state
        .record_files
        .entry(file_id)
        .or_default()
        .push(apdu.data.clone());
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_search_record_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let Some(file_id) = mock_selected_file_id(state) else {
        return ResponseApdu::status(iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16());
    };
    let Some(records) = state.record_files.get(&file_id) else {
        return ResponseApdu::status(iso7816::StatusWord::FILE_OR_APPLICATION_NOT_FOUND.as_u16());
    };
    let matches = records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            record
                .windows(apdu.data.len())
                .any(|window| window == apdu.data)
                .then_some((index + 1) as u8)
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    mock_chunk_response(state, matches, apdu.ne)
}

fn mock_get_data_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let key = (apdu.p1, apdu.p2);
    if let Some(data) = state.data_objects.get(&key).cloned() {
        return mock_chunk_response(state, data, apdu.ne);
    }
    ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16())
}

fn mock_put_data_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    state
        .data_objects
        .insert((apdu.p1, apdu.p2), apdu.data.clone());
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_verify_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let reference = apdu.p2;
    let Some(expected) = state.reference_data.get(&reference) else {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    };
    let remaining = *state.retry_counters.get(&reference).unwrap_or(&0);
    if remaining == 0 {
        return ResponseApdu::status(iso7816::StatusWord::AUTH_METHOD_BLOCKED.as_u16());
    }
    if apdu.data.is_empty() {
        return ResponseApdu::status(0x63C0 | u16::from(remaining));
    }
    if &apdu.data == expected {
        if let Some(limit) = state.retry_limits.get(&reference).copied() {
            state.retry_counters.insert(reference, limit);
        }
        ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
    } else {
        let updated = remaining.saturating_sub(1);
        state.retry_counters.insert(reference, updated);
        if updated == 0 {
            ResponseApdu::status(iso7816::StatusWord::AUTH_METHOD_BLOCKED.as_u16())
        } else {
            ResponseApdu::status(0x63C0 | u16::from(updated))
        }
    }
}

fn mock_change_reference_data_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> ResponseApdu {
    let reference = apdu.p2;
    if !state.reference_data.contains_key(&reference) {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    if !state.session_state.verified_references.contains(&reference) {
        return ResponseApdu::status(iso7816::StatusWord::SECURITY_STATUS_NOT_SATISFIED.as_u16());
    }
    if apdu.data.is_empty() {
        return ResponseApdu::status(iso7816::StatusWord::WRONG_LENGTH.as_u16());
    }
    state.reference_data.insert(reference, apdu.data.clone());
    if let Some(limit) = state.retry_limits.get(&reference).copied() {
        state.retry_counters.insert(reference, limit);
    }
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_reset_retry_counter_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> ResponseApdu {
    let reference = apdu.p2;
    if !state.reference_data.contains_key(&reference) {
        return ResponseApdu::status(iso7816::StatusWord::DATA_NOT_FOUND.as_u16());
    }
    let isd_selected = state
        .session_state
        .selected_aid
        .as_ref()
        .is_some_and(|aid| aid.as_bytes() == globalplatform::ISSUER_SECURITY_DOMAIN_AID);
    if !isd_selected && !state.session_state.verified_references.contains(&reference) {
        return ResponseApdu::status(iso7816::StatusWord::SECURITY_STATUS_NOT_SATISFIED.as_u16());
    }
    if apdu.data.is_empty() {
        return ResponseApdu::status(iso7816::StatusWord::WRONG_LENGTH.as_u16());
    }
    state.reference_data.insert(reference, apdu.data.clone());
    if let Some(limit) = state.retry_limits.get(&reference).copied() {
        state.retry_counters.insert(reference, limit);
    }
    ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
}

fn mock_get_challenge_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let expected_length = apdu.ne.unwrap_or(8).clamp(1, 32);
    let challenge = mock_deterministic_bytes(&mut state.challenge_counter, expected_length);
    mock_chunk_response(state, challenge, apdu.ne)
}

fn mock_internal_authenticate_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> ResponseApdu {
    let expected_length = apdu.ne.unwrap_or(apdu.data.len().max(8));
    let mut output = apdu.data.iter().rev().copied().collect::<Vec<_>>();
    while output.len() < expected_length {
        output.extend_from_slice(&apdu.data);
        if apdu.data.is_empty() {
            output.extend_from_slice(&mock_deterministic_bytes(&mut state.challenge_counter, 8));
        }
    }
    output.truncate(expected_length);
    mock_chunk_response(state, output, apdu.ne)
}

fn mock_external_authenticate_response(apdu: &CommandApdu) -> ResponseApdu {
    if apdu.data.is_empty() {
        ResponseApdu::status(iso7816::StatusWord::WRONG_LENGTH.as_u16())
    } else {
        ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16())
    }
}

fn mock_envelope_response(state: &mut MockCardState, apdu: &CommandApdu) -> ResponseApdu {
    let payload = apdu.data.iter().rev().copied().collect::<Vec<_>>();
    mock_chunk_response(state, payload, apdu.ne)
}

fn mock_chunk_response(
    state: &mut MockCardState,
    data: Vec<u8>,
    expected_length: Option<usize>,
) -> ResponseApdu {
    let Some(expected_length) = expected_length else {
        state.pending_response.clear();
        return ResponseApdu::success(data);
    };
    if data.len() <= expected_length {
        state.pending_response.clear();
        return ResponseApdu::success(data);
    }
    state.pending_response = data[expected_length..].to_vec();
    let hinted = state.pending_response.len().min(256);
    ResponseApdu {
        data: data[..expected_length].to_vec(),
        sw: 0x6100 | if hinted == 256 { 0 } else { hinted as u16 },
    }
}
