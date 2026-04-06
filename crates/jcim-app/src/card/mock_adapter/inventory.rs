#![allow(clippy::missing_docs_in_private_items)]

use super::*;

use super::state::{lock_poisoned, mock_reset_session_state};

pub(super) fn mock_list_readers(
    adapter: &MockPhysicalCardAdapter,
) -> Result<Vec<CardReaderSummary>> {
    Ok(adapter.state.lock().map_err(lock_poisoned)?.readers.clone())
}

pub(super) fn mock_card_status(
    adapter: &MockPhysicalCardAdapter,
    reader_name: Option<&str>,
) -> Result<CardStatusSummary> {
    let state = adapter.state.lock().map_err(lock_poisoned)?;
    let reader = reader_name_or_default(&state, reader_name);
    let status_reader = state
        .readers
        .first()
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| reader.clone());
    Ok(CardStatusSummary {
        reader_name: reader,
        card_present: true,
        atr: state.session_state.atr.clone(),
        active_protocol: state.session_state.active_protocol.clone(),
        iso_capabilities: state.iso_capabilities.clone(),
        session_state: state.session_state.clone(),
        lines: vec![
            format!("Reader: {status_reader}"),
            "Card present: yes".to_string(),
            format!("Protocol: {}", state.protocol),
            format!("ATR: {}", state.atr_hex),
        ],
    })
}

pub(super) fn mock_install_cap(
    adapter: &MockPhysicalCardAdapter,
    reader_name: Option<&str>,
    cap_path: &Path,
) -> Result<Vec<String>> {
    let cap = CapPackage::from_path(cap_path)?;
    let mut state = adapter.state.lock().map_err(lock_poisoned)?;
    state
        .packages
        .retain(|package| package.aid != cap.package_aid.to_hex());
    state.packages.push(CardPackageSummary {
        aid: cap.package_aid.to_hex(),
        description: format!(
            "{} {}.{}",
            cap.package_name, cap.package_major, cap.package_minor
        ),
    });
    for applet in cap.applets {
        let aid = applet.aid.to_hex();
        state.applets.retain(|existing| existing.aid != aid);
        state.applets.push(CardAppletSummary {
            aid,
            description: applet.name.unwrap_or_else(|| "InstalledApplet".to_string()),
        });
    }
    state.pending_get_status = None;
    Ok(vec![format!(
        "Installed CAP {} on {}",
        cap_path.display(),
        reader_name_or_default(&state, reader_name)
    )])
}

pub(super) fn mock_delete_item(
    adapter: &MockPhysicalCardAdapter,
    reader_name: Option<&str>,
    aid: &str,
) -> Result<Vec<String>> {
    let mut state = adapter.state.lock().map_err(lock_poisoned)?;
    state.packages.retain(|package| package.aid != aid);
    state.applets.retain(|applet| applet.aid != aid);
    state.locked_aids.remove(aid);
    if state
        .session_state
        .selected_aid
        .as_ref()
        .is_some_and(|selected| selected.to_hex() == aid)
    {
        state.session_state.selected_aid = None;
        state.session_state.current_file = None;
        for channel in &mut state.session_state.open_channels {
            channel.selected_aid = None;
            channel.current_file = None;
        }
    }
    state.pending_get_status = None;
    Ok(vec![format!(
        "Deleted {aid} from {}",
        reader_name_or_default(&state, reader_name)
    )])
}

pub(super) fn mock_list_packages(
    adapter: &MockPhysicalCardAdapter,
    reader_name: Option<&str>,
) -> Result<CardPackageInventory> {
    let state = adapter.state.lock().map_err(lock_poisoned)?;
    Ok(CardPackageInventory {
        reader_name: reader_name_or_default(&state, reader_name),
        packages: state.packages.clone(),
        output_lines: state
            .packages
            .iter()
            .map(|package| format!("PKG: {} {}", package.aid, package.description))
            .collect(),
    })
}

pub(super) fn mock_list_applets(
    adapter: &MockPhysicalCardAdapter,
    reader_name: Option<&str>,
) -> Result<CardAppletInventory> {
    let state = adapter.state.lock().map_err(lock_poisoned)?;
    Ok(CardAppletInventory {
        reader_name: reader_name_or_default(&state, reader_name),
        applets: state.applets.clone(),
        output_lines: state
            .applets
            .iter()
            .map(|applet| format!("APP: {} {}", applet.aid, applet.description))
            .collect(),
    })
}

pub(super) fn mock_reset_card(adapter: &MockPhysicalCardAdapter) -> Result<String> {
    let mut state = adapter.state.lock().map_err(lock_poisoned)?;
    state.pending_response.clear();
    state.pending_get_status = None;
    state.pending_gp_auth = None;
    state.retry_counters = state.retry_limits.clone();
    state.session_state = mock_reset_session_state(&state.atr_hex, &state.protocol);
    Ok(state.atr_hex.clone())
}

fn reader_name_or_default(state: &MockCardState, reader_name: Option<&str>) -> String {
    reader_name
        .map(str::to_string)
        .or_else(|| state.readers.first().map(|reader| reader.name.clone()))
        .unwrap_or_else(|| "Mock Reader 0".to_string())
}
