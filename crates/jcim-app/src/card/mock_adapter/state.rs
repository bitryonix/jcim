#![allow(clippy::missing_docs_in_private_items)]

use super::*;

#[derive(Default)]
pub(super) struct MockCardState {
    pub(super) readers: Vec<CardReaderSummary>,
    pub(super) protocol: String,
    pub(super) atr_hex: String,
    pub(super) iso_capabilities: IsoCapabilities,
    pub(super) session_state: IsoSessionState,
    pub(super) card_life_cycle: globalplatform::CardLifeCycle,
    pub(super) packages: Vec<CardPackageSummary>,
    pub(super) applets: Vec<CardAppletSummary>,
    pub(super) locked_aids: HashSet<String>,
    pub(super) pending_response: Vec<u8>,
    pub(super) pending_get_status: Option<Vec<u8>>,
    pub(super) binary_files: BTreeMap<u16, Vec<u8>>,
    pub(super) record_files: BTreeMap<u16, Vec<Vec<u8>>>,
    pub(super) data_objects: BTreeMap<(u8, u8), Vec<u8>>,
    pub(super) reference_data: BTreeMap<u8, Vec<u8>>,
    pub(super) retry_limits: BTreeMap<u8, u8>,
    pub(super) retry_counters: BTreeMap<u8, u8>,
    pub(super) challenge_counter: u32,
    pub(super) pending_gp_auth: Option<PendingGpAuthState>,
}

#[derive(Clone)]
pub(super) struct PendingGpAuthState {
    pub(super) protocol: SecureMessagingProtocol,
    pub(super) session_id: String,
}

impl MockCardState {
    pub(super) fn new() -> Self {
        let protocol = "T=1".to_string();
        let atr_hex = "3B800100".to_string();
        let iso_capabilities = IsoCapabilities {
            protocols: vec![TransportProtocol::T1],
            extended_length: true,
            logical_channels: true,
            max_logical_channels: 4,
            secure_messaging: true,
            file_model_visibility: true,
            raw_apdu: true,
        };
        Self {
            readers: vec![CardReaderSummary {
                name: "Mock Reader 0".to_string(),
                card_present: true,
            }],
            session_state: mock_reset_session_state(&atr_hex, &protocol),
            protocol,
            atr_hex,
            iso_capabilities,
            card_life_cycle: globalplatform::CardLifeCycle::Secured,
            packages: Vec::new(),
            applets: Vec::new(),
            locked_aids: HashSet::new(),
            pending_response: Vec::new(),
            pending_get_status: None,
            binary_files: BTreeMap::from([(0x0101, b"JCIM mock EF".to_vec())]),
            record_files: BTreeMap::from([(
                0x0201,
                vec![b"record-1".to_vec(), b"record-2".to_vec()],
            )]),
            data_objects: BTreeMap::from([((0x00, 0x42), b"JCIM".to_vec())]),
            reference_data: BTreeMap::from([(0x80, b"1234".to_vec())]),
            retry_limits: BTreeMap::from([(0x80, 3)]),
            retry_counters: BTreeMap::from([(0x80, 3)]),
            challenge_counter: 0,
            pending_gp_auth: None,
        }
    }
}

pub(super) fn mock_reset_session_state(atr_hex: &str, protocol: &str) -> IsoSessionState {
    let atr = hex::decode(atr_hex)
        .ok()
        .and_then(|raw| Atr::parse(&raw).ok());
    let active_protocol =
        TransportProtocol::from_status_text(protocol).map(|protocol| ProtocolParameters {
            protocol: Some(protocol),
            ..ProtocolParameters::default()
        });
    IsoSessionState::reset(atr, active_protocol)
}

pub(super) fn mock_selected_file_id(state: &MockCardState) -> Option<u16> {
    match state.session_state.current_file.clone() {
        Some(iso7816::FileSelection::FileId(file_id)) => Some(file_id),
        Some(iso7816::FileSelection::Path(path)) if path.len() >= 2 => {
            let end = path.len();
            Some(u16::from_be_bytes([path[end - 2], path[end - 1]]))
        }
        _ => None,
    }
}

pub(super) fn mock_deterministic_bytes(counter: &mut u32, len: usize) -> Vec<u8> {
    let seed = *counter;
    *counter = counter.saturating_add(1);
    (0..len)
        .map(|offset| seed.wrapping_add(offset as u32) as u8)
        .collect()
}

pub(super) fn mock_card_life_cycle_state(state: globalplatform::CardLifeCycle) -> u8 {
    match state {
        globalplatform::CardLifeCycle::OpReady => 0x01,
        globalplatform::CardLifeCycle::Initialized => 0x07,
        globalplatform::CardLifeCycle::Secured => 0x0F,
        globalplatform::CardLifeCycle::CardLocked => 0x7F,
        globalplatform::CardLifeCycle::Terminated => 0xFF,
    }
}

pub(super) fn lock_poisoned<T>(_: T) -> JcimError {
    JcimError::Unsupported("physical-card adapter state lock was poisoned".to_string())
}
