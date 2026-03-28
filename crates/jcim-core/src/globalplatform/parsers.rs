use crate::aid::Aid;
use crate::apdu::ResponseApdu;
use crate::error::{JcimError, Result};
use crate::iso7816::StatusWord;

use super::status::{GetStatusResponse, RegistryEntry, RegistryKind};

/// Parse the response to `GET STATUS`.
pub fn parse_get_status(kind: RegistryKind, response: &ResponseApdu) -> Result<GetStatusResponse> {
    let status = response.status_word();
    if status != StatusWord::SUCCESS && status != StatusWord::MORE_DATA_AVAILABLE {
        return Err(JcimError::Gp(format!(
            "GET STATUS returned status word {}",
            status
        )));
    }

    let entries = parse_registry_entries(kind, &response.data)?;
    Ok(GetStatusResponse {
        kind,
        entries,
        more_data_available: status == StatusWord::MORE_DATA_AVAILABLE,
    })
}

fn parse_registry_entries(kind: RegistryKind, input: &[u8]) -> Result<Vec<RegistryEntry>> {
    let top_level = parse_tlvs(input)?;
    let mut entries = Vec::new();
    for tlv in top_level {
        if tlv.tag != 0xE3 {
            return Err(JcimError::Gp(format!(
                "GET STATUS returned unexpected top-level tag {:X}",
                tlv.tag
            )));
        }
        let nested = parse_tlvs(&tlv.value)?;
        let mut aid = None;
        let mut life_cycle_state = None;
        let mut privileges = None;
        let mut executable_load_file_aid = None;
        let mut associated_security_domain_aid = None;
        let mut executable_module_aids = Vec::new();
        let mut load_file_version = None;
        let mut implicit_selection_parameters = Vec::new();
        for child in nested {
            match child.tag {
                0x4F => aid = Some(Aid::from_slice(&child.value)?),
                0x9F70 => {
                    let Some(value) = child.value.first().copied() else {
                        return Err(JcimError::Gp(
                            "GET STATUS entry returned an empty life cycle state".to_string(),
                        ));
                    };
                    life_cycle_state = Some(value);
                }
                0xC5 => {
                    if child.value.len() == 3 {
                        privileges = Some([child.value[0], child.value[1], child.value[2]]);
                    }
                }
                0xC4 => executable_load_file_aid = Some(Aid::from_slice(&child.value)?),
                0xCC => associated_security_domain_aid = Some(Aid::from_slice(&child.value)?),
                0x84 => executable_module_aids.push(Aid::from_slice(&child.value)?),
                0xCE => load_file_version = Some(child.value),
                0xCF => {
                    let Some(value) = child.value.first().copied() else {
                        continue;
                    };
                    implicit_selection_parameters.push(value);
                }
                _ => {}
            }
        }
        entries.push(RegistryEntry {
            kind,
            aid: aid.ok_or_else(|| {
                JcimError::Gp("GET STATUS entry omitted the mandatory AID".to_string())
            })?,
            life_cycle_state: life_cycle_state.ok_or_else(|| {
                JcimError::Gp("GET STATUS entry omitted the mandatory life cycle state".to_string())
            })?,
            privileges,
            executable_load_file_aid,
            associated_security_domain_aid,
            executable_module_aids,
            load_file_version,
            implicit_selection_parameters,
        });
    }
    Ok(entries)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BerTlv {
    tag: u32,
    value: Vec<u8>,
}

fn parse_tlvs(input: &[u8]) -> Result<Vec<BerTlv>> {
    let mut offset = 0;
    let mut tlvs = Vec::new();
    while offset < input.len() {
        let (tlv, consumed) = parse_tlv(&input[offset..])?;
        tlvs.push(tlv);
        offset += consumed;
    }
    Ok(tlvs)
}

fn parse_tlv(input: &[u8]) -> Result<(BerTlv, usize)> {
    if input.len() < 2 {
        return Err(JcimError::Gp(
            "BER-TLV input is too short to contain a tag and length".to_string(),
        ));
    }
    let mut offset = 0;
    let (tag, tag_length) = parse_tag(&input[offset..])?;
    offset += tag_length;
    let (length, length_length) = parse_length(&input[offset..])?;
    offset += length_length;
    if input.len() < offset + length {
        return Err(JcimError::Gp(
            "BER-TLV input ended before the declared value length".to_string(),
        ));
    }
    let value = input[offset..offset + length].to_vec();
    offset += length;
    Ok((BerTlv { tag, value }, offset))
}

fn parse_tag(input: &[u8]) -> Result<(u32, usize)> {
    let mut tag = u32::from(
        *input
            .first()
            .ok_or_else(|| JcimError::Gp("BER-TLV input is missing a tag byte".to_string()))?,
    );
    let mut consumed = 1;
    if tag & 0x1F == 0x1F {
        loop {
            let byte = *input
                .get(consumed)
                .ok_or_else(|| JcimError::Gp("BER-TLV tag was truncated".to_string()))?;
            tag = (tag << 8) | u32::from(byte);
            consumed += 1;
            if byte & 0x80 == 0 {
                break;
            }
        }
    }
    Ok((tag, consumed))
}

fn parse_length(input: &[u8]) -> Result<(usize, usize)> {
    let first = *input
        .first()
        .ok_or_else(|| JcimError::Gp("BER-TLV input is missing a length byte".to_string()))?;
    if first & 0x80 == 0 {
        return Ok((usize::from(first), 1));
    }

    let byte_count = usize::from(first & 0x7F);
    if byte_count == 0 || byte_count > 2 || input.len() < 1 + byte_count {
        return Err(JcimError::Gp(
            "BER-TLV long-form length is unsupported or truncated".to_string(),
        ));
    }

    let mut length = 0usize;
    for byte in &input[1..=byte_count] {
        length = (length << 8) | usize::from(*byte);
    }
    Ok((length, 1 + byte_count))
}
