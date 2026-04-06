#![allow(clippy::missing_docs_in_private_items)]

use super::*;

use super::state::{
    PendingGpAuthState, lock_poisoned, mock_card_life_cycle_state, mock_deterministic_bytes,
};

pub(super) fn open_mock_gp_secure_channel(
    adapter: &MockPhysicalCardAdapter,
    keyset: &ResolvedGpKeyset,
    security_level: u8,
) -> Result<()> {
    let mut state = adapter.state.lock().map_err(lock_poisoned)?;
    let session_id = format!("mock-gp-helper-{}", state.challenge_counter);
    let selected_aid = Some(Aid::from_slice(
        &globalplatform::ISSUER_SECURITY_DOMAIN_AID,
    )?);
    state.session_state.selected_aid = selected_aid.clone();
    state.session_state.current_file = None;
    if let Some(channel) = state
        .session_state
        .open_channels
        .iter_mut()
        .find(|channel| channel.channel_number == 0)
    {
        channel.selected_aid = selected_aid;
        channel.current_file = None;
    }
    state.session_state.secure_messaging.active = true;
    state.session_state.secure_messaging.protocol = Some(keyset.protocol());
    state.session_state.secure_messaging.security_level = Some(security_level);
    state.session_state.secure_messaging.session_id = Some(session_id);
    state.session_state.secure_messaging.command_counter = 0;
    state.pending_gp_auth = None;
    Ok(())
}

pub(super) fn apply_pending_gp_external_auth(
    state: &mut MockCardState,
    apdu: &CommandApdu,
    response: &ResponseApdu,
) {
    if let Some(protocol) = state
        .pending_gp_auth
        .as_ref()
        .map(|auth| auth.protocol.clone())
        && apdu.cla == 0x80
        && apdu.ins == 0x82
        && response.is_success()
    {
        state.session_state.secure_messaging.active = true;
        state.session_state.secure_messaging.protocol = Some(protocol);
        state.session_state.secure_messaging.security_level = Some(apdu.p1);
        state.session_state.secure_messaging.session_id = state
            .pending_gp_auth
            .as_ref()
            .map(|auth| auth.session_id.clone());
        state.session_state.secure_messaging.command_counter = 1;
        state.pending_gp_auth = None;
    }
}

pub(super) fn mock_get_status_response(
    state: &mut MockCardState,
    p1: u8,
    p2: u8,
) -> Result<ResponseApdu> {
    if p2 == 0x03 {
        if let Some(remaining) = state.pending_get_status.take() {
            return Ok(mock_chunk_registry_response(state, remaining));
        }
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16(),
        ));
    }
    if p2 != 0x02 {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::INCORRECT_P1_P2.as_u16(),
        ));
    }

    let mut data = Vec::new();
    match p1 {
        0x80 => {
            data.extend(mock_registry_entry(
                &Aid::from_slice(&globalplatform::ISSUER_SECURITY_DOMAIN_AID)?,
                mock_card_life_cycle_state(state.card_life_cycle),
                Some([0x9E, 0x00, 0x00]),
            )?);
        }
        0x40 => {
            for applet in &state.applets {
                let aid = Aid::from_hex(&applet.aid)?;
                let life_cycle_state = if state.locked_aids.contains(&applet.aid) {
                    0x83
                } else {
                    0x07
                };
                data.extend(mock_registry_entry(&aid, life_cycle_state, None)?);
            }
        }
        0x20 | 0x10 => {
            for package in &state.packages {
                let aid = Aid::from_hex(&package.aid)?;
                data.extend(mock_registry_entry(&aid, 0x01, None)?);
            }
        }
        _ => {
            return Ok(ResponseApdu::status(
                iso7816::StatusWord::INCORRECT_P1_P2.as_u16(),
            ));
        }
    }
    Ok(mock_chunk_registry_response(state, data))
}

pub(super) fn mock_set_status_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> Result<ResponseApdu> {
    mock_apply_set_status(state, apdu)?;
    Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()))
}

pub(super) fn mock_initialize_update_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> Result<ResponseApdu> {
    if apdu.data.len() != 8 {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::WRONG_LENGTH.as_u16(),
        ));
    }
    let protocol = SecureMessagingProtocol::Scp03;
    let sequence = state.challenge_counter as u16;
    let card_challenge = mock_deterministic_bytes(&mut state.challenge_counter, 6);
    let mut data = vec![0x00; 10];
    data.push(0x01);
    data.push(0x03);
    data.extend_from_slice(&sequence.to_be_bytes());
    data.extend_from_slice(&card_challenge);
    data.extend_from_slice(&[0x00; 8]);
    state.pending_gp_auth = Some(PendingGpAuthState {
        protocol,
        session_id: format!("mock-gp-{}", sequence),
    });
    Ok(ResponseApdu::success(data))
}

pub(super) fn mock_gp_external_authenticate_response(
    state: &mut MockCardState,
    apdu: &CommandApdu,
) -> Result<ResponseApdu> {
    if state.pending_gp_auth.is_none() {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::CONDITIONS_NOT_SATISFIED.as_u16(),
        ));
    }
    if apdu.data.len() != 8 {
        return Ok(ResponseApdu::status(
            iso7816::StatusWord::WRONG_LENGTH.as_u16(),
        ));
    }
    Ok(ResponseApdu::status(iso7816::StatusWord::SUCCESS.as_u16()))
}

fn mock_registry_entry(
    aid: &Aid,
    life_cycle_state: u8,
    privileges: Option<[u8; 3]>,
) -> Result<Vec<u8>> {
    let mut nested = vec![0x4F, aid.as_bytes().len() as u8];
    nested.extend_from_slice(aid.as_bytes());
    nested.extend_from_slice(&[0x9F, 0x70, 0x01, life_cycle_state]);
    if let Some(privileges) = privileges {
        nested.extend_from_slice(&[0xC5, 0x03]);
        nested.extend_from_slice(&privileges);
    }

    let mut entry = vec![0xE3];
    if nested.len() > usize::from(u8::MAX) {
        return Err(JcimError::Gp(
            "mock registry entry exceeded short-form BER-TLV length".to_string(),
        ));
    }
    entry.push(nested.len() as u8);
    entry.extend(nested);
    Ok(entry)
}

fn mock_apply_set_status(state: &mut MockCardState, apdu: &CommandApdu) -> Result<()> {
    match apdu.p1 {
        0x80 => {
            state.card_life_cycle = match apdu.p2 {
                0x01 => globalplatform::CardLifeCycle::OpReady,
                0x07 => globalplatform::CardLifeCycle::Initialized,
                0x0F => globalplatform::CardLifeCycle::Secured,
                0x7F => globalplatform::CardLifeCycle::CardLocked,
                0xFF => globalplatform::CardLifeCycle::Terminated,
                other => {
                    return Err(JcimError::Gp(format!(
                        "unsupported mock card life cycle transition {:02X}",
                        other
                    )));
                }
            };
        }
        0x40 | 0x60 => {
            let aid = Aid::from_slice(&apdu.data)?.to_hex();
            match apdu.p2 {
                0x80 => {
                    state.locked_aids.insert(aid);
                }
                0x00 => {
                    state.locked_aids.remove(&aid);
                }
                other => {
                    return Err(JcimError::Gp(format!(
                        "unsupported mock application/security-domain state control {:02X}",
                        other
                    )));
                }
            }
        }
        other => {
            return Err(JcimError::Gp(format!(
                "unsupported mock SET STATUS target {:02X}",
                other
            )));
        }
    }
    state.pending_get_status = None;
    Ok(())
}

fn mock_chunk_registry_response(state: &mut MockCardState, data: Vec<u8>) -> ResponseApdu {
    const PAGE_BYTES: usize = 96;
    if data.len() > PAGE_BYTES {
        state.pending_get_status = Some(data[PAGE_BYTES..].to_vec());
        ResponseApdu {
            data: data[..PAGE_BYTES].to_vec(),
            sw: iso7816::StatusWord::MORE_DATA_AVAILABLE.as_u16(),
        }
    } else {
        state.pending_get_status = None;
        ResponseApdu::success(data)
    }
}
