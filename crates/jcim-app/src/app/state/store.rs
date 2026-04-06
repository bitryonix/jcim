use super::*;

/// Shared mutable state for local JCIM application services.
pub(in crate::app) struct AppState {
    /// Managed filesystem layout used for config, registry, and runtime records.
    pub(in crate::app) managed_paths: ManagedPaths,
    /// Resolved service binary path recorded in local runtime metadata.
    pub(in crate::app) service_binary_path: PathBuf,
    /// Stable fingerprint for the currently running service binary.
    pub(in crate::app) service_binary_fingerprint: String,
    // Lock-scoped state helpers below own the policy for these stores. Callers must clone any
    // handle or summary they need before `.await` and commit mutations in a separate short step.
    /// Persisted user configuration snapshot.
    pub(in crate::app) user_config: RwLock<UserConfig>,
    /// Local registry of known JCIM projects.
    pub(in crate::app) registry: RwLock<ProjectRegistry>,
    /// Live simulation records keyed by simulation id.
    pub(in crate::app) simulations: Mutex<HashMap<String, SimulationRecord>>,
    /// Retained build events keyed by project id.
    pub(in crate::app) build_events: Mutex<HashMap<String, VecDeque<EventLine>>>,
    /// Tracked physical-card session state keyed by reader identity.
    pub(in crate::app) card_sessions: Mutex<HashMap<String, CardSessionRecord>>,
    /// Physical-card adapter used by card service helpers.
    pub(in crate::app) card_adapter: Arc<dyn PhysicalCardAdapter>,
    /// Monotonic seed used when allocating new managed simulation ids.
    pub(in crate::app) next_simulation_id: AtomicU64,
}

impl AppState {
    /// Construct one application state snapshot with explicit stores and adapter dependencies.
    pub(in crate::app) fn new(
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
