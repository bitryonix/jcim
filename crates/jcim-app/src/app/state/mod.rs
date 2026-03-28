use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, RwLock};

use jcim_backends::backend::BackendHandle;
use jcim_config::config::RuntimeConfig;
use jcim_config::project::{ManagedPaths, ProjectConfig, UserConfig};
use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::error::{JcimError, Result};
use jcim_core::globalplatform;
use jcim_core::iso7816::{
    Atr, IsoCapabilities, IsoSessionState, ProtocolParameters, SecureMessagingProtocol,
    SecureMessagingState, apply_response_to_session,
};

use super::events::remember_event;
use crate::card::PhysicalCardAdapter;
use crate::model::{EventLine, SimulationStatusKind, SimulationSummary};
use crate::registry::ProjectRegistry;

mod build_events;
mod card_sessions;
mod config;
mod registry;
mod simulations;

use self::card_sessions::CardSessionRecord;

pub(crate) use self::registry::ResolvedProject;
pub(crate) use self::simulations::{PreparedSimulation, SimulationRecord};

pub(super) struct AppState {
    pub(super) managed_paths: ManagedPaths,
    pub(super) service_binary_path: PathBuf,
    pub(super) service_binary_fingerprint: String,
    // Lock-scoped state helpers below own the policy for these stores. Callers must clone any
    // handle or summary they need before `.await` and commit mutations in a separate short step.
    pub(super) user_config: RwLock<UserConfig>,
    pub(super) registry: RwLock<ProjectRegistry>,
    pub(super) simulations: Mutex<HashMap<String, SimulationRecord>>,
    pub(super) build_events: Mutex<HashMap<String, VecDeque<EventLine>>>,
    pub(super) card_sessions: Mutex<HashMap<String, CardSessionRecord>>,
    pub(super) card_adapter: Arc<dyn PhysicalCardAdapter>,
    pub(super) next_simulation_id: AtomicU64,
}

impl AppState {
    pub(super) fn new(
        managed_paths: ManagedPaths,
        service_binary_path: PathBuf,
        service_binary_fingerprint: String,
        user_config: UserConfig,
        registry: ProjectRegistry,
        card_adapter: Arc<dyn PhysicalCardAdapter>,
        next_simulation_id: u64,
    ) -> Self {
        Self {
            managed_paths,
            service_binary_path,
            service_binary_fingerprint,
            user_config: RwLock::new(user_config),
            registry: RwLock::new(registry),
            simulations: Mutex::new(HashMap::new()),
            build_events: Mutex::new(HashMap::new()),
            card_sessions: Mutex::new(HashMap::new()),
            card_adapter,
            next_simulation_id: AtomicU64::new(next_simulation_id),
        }
    }
}

pub(crate) fn lock_poisoned<T>(_: T) -> JcimError {
    JcimError::Unsupported("internal application state lock was poisoned".to_string())
}
