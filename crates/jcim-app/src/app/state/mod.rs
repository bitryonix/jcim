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
/// Shared state storage and lock error translation.
mod store;

use self::card_sessions::CardSessionRecord;

pub(crate) use self::registry::ResolvedProject;
pub(crate) use self::simulations::{PreparedSimulation, SimulationRecord};

pub(super) use self::store::AppState;
pub(crate) use self::store::lock_poisoned;
