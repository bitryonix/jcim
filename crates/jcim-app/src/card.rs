//! Physical-card adapter boundary for JCIM.

#![allow(clippy::missing_docs_in_private_items)]

mod adapter;
mod gp_keyset;
mod helper_tool;
mod inventory_parser;
mod java_adapter;
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
