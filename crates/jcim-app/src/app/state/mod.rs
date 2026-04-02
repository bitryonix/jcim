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

/// Build-event retention helpers for per-project build logs.
mod build_events;
/// Physical-card session tracking helpers keyed by reader identity.
mod card_sessions;
/// User-config snapshot and persistence helpers.
mod config;
/// Project-registry lookup and persistence helpers.
mod registry;
/// Managed-simulation record and session tracking helpers.
mod simulations;

use self::card_sessions::CardSessionRecord;

pub(crate) use self::registry::ResolvedProject;
pub(crate) use self::simulations::{PreparedSimulation, SimulationRecord};

/// Shared mutable state for local JCIM application services.
pub(super) struct AppState {
    /// Managed filesystem layout used for config, registry, and runtime records.
    pub(super) managed_paths: ManagedPaths,
    /// Resolved service binary path recorded in local runtime metadata.
    pub(super) service_binary_path: PathBuf,
    /// Stable fingerprint for the currently running service binary.
    pub(super) service_binary_fingerprint: String,
    // Lock-scoped state helpers below own the policy for these stores. Callers must clone any
    // handle or summary they need before `.await` and commit mutations in a separate short step.
    /// Persisted user configuration snapshot.
    pub(super) user_config: RwLock<UserConfig>,
    /// Local registry of known JCIM projects.
    pub(super) registry: RwLock<ProjectRegistry>,
    /// Live simulation records keyed by simulation id.
    pub(super) simulations: Mutex<HashMap<String, SimulationRecord>>,
    /// Retained build events keyed by project id.
    pub(super) build_events: Mutex<HashMap<String, VecDeque<EventLine>>>,
    /// Tracked physical-card session state keyed by reader identity.
    pub(super) card_sessions: Mutex<HashMap<String, CardSessionRecord>>,
    /// Physical-card adapter used by card service helpers.
    pub(super) card_adapter: Arc<dyn PhysicalCardAdapter>,
    /// Monotonic seed used when allocating new managed simulation ids.
    pub(super) next_simulation_id: AtomicU64,
}

impl AppState {
    /// Construct one application state snapshot with explicit stores and adapter dependencies.
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

/// Convert a poisoned lock into a stable local application error.
pub(crate) fn lock_poisoned<T>(_: T) -> JcimError {
    JcimError::Unsupported("internal application state lock was poisoned".to_string())
}
