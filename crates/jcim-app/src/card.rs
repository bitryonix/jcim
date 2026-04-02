//! Physical-card adapter boundary for JCIM.

/// Adapter trait and shared helper types.
mod adapter;
/// GP keyset resolution helpers for helper-tool and GPPro workflows.
mod gp_keyset;
/// Java helper/GPPro process invocation helpers.
mod helper_tool;
/// Text parser for helper-tool package and applet inventory output.
mod inventory_parser;
/// Java-backed physical-card adapter implementation.
mod java_adapter;
/// In-memory mock physical-card adapter used by tests and docs flows.
mod mock_adapter;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::process::Command;

use jcim_cap::prelude::CapPackage;
use jcim_config::project::UserConfig;
use jcim_core::aid::Aid;
use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::error::{JcimError, Result};
use jcim_core::iso7816::{
    Atr, IsoCapabilities, IsoSessionState, ProtocolParameters, SecureMessagingProtocol,
    TransportProtocol, apply_response_to_session,
};
use jcim_core::{globalplatform, iso7816};

use crate::model::{
    CardAppletInventory, CardAppletSummary, CardPackageInventory, CardPackageSummary,
    CardReaderSummary, CardStatusSummary, ResetSummary,
};

pub use self::adapter::PhysicalCardAdapter;
pub(crate) use self::gp_keyset::ResolvedGpKeyset;
pub(crate) use self::helper_tool::{gppro_jar_path, helper_jar_path};
pub(crate) use self::java_adapter::JavaPhysicalCardAdapter;
pub use self::mock_adapter::MockPhysicalCardAdapter;
