//! Unified Rust APDU connection surface for real and virtual cards.
#![allow(clippy::missing_docs_in_private_items)]
// This module is the thin internal dispatcher behind the public `CardConnection` API.
// The public surface is documented; private storage and helper glue stay intentionally compact.

use std::fmt;

use jcim_core::apdu::{CommandApdu, ResponseApdu};
use jcim_core::iso7816::IsoSessionState;

use crate::client::JcimClient;
use crate::error::Result;
use crate::types::{ApduExchangeSummary, CardConnectionKind, CardConnectionLocator, ResetSummary};

/// One unified APDU connection to a real reader or one virtual simulation.
pub struct CardConnection {
    client: JcimClient,
    locator: CardConnectionLocator,
}

impl CardConnection {
    pub(crate) fn new(client: JcimClient, locator: CardConnectionLocator) -> Self {
        Self { client, locator }
    }

    /// Return the target kind for this connection.
    pub fn kind(&self) -> CardConnectionKind {
        match &self.locator {
            CardConnectionLocator::Reader { .. } => CardConnectionKind::Reader,
            CardConnectionLocator::Simulation { .. } => CardConnectionKind::Simulation,
        }
    }

    /// Return the resolved location behind this connection.
    pub fn locator(&self) -> &CardConnectionLocator {
        &self.locator
    }

    /// Send one typed APDU over this connection.
    pub async fn transmit(&self, command: &CommandApdu) -> Result<ResponseApdu> {
        match &self.locator {
            CardConnectionLocator::Reader { reader_name } => {
                self.client
                    .transmit_card_apdu_on(command, crate::ReaderRef::named(reader_name.clone()))
                    .await
            }
            CardConnectionLocator::Simulation { simulation, .. } => {
                self.client
                    .transmit_sim_apdu(simulation.clone(), command)
                    .await
            }
        }
    }

    /// Send one raw APDU byte sequence over this connection.
    pub async fn transmit_raw(&self, apdu: &[u8]) -> Result<ApduExchangeSummary> {
        match &self.locator {
            CardConnectionLocator::Reader { reader_name } => {
                self.client
                    .transmit_raw_card_apdu_on(apdu, crate::ReaderRef::named(reader_name.clone()))
                    .await
            }
            CardConnectionLocator::Simulation { simulation, .. } => {
                self.client
                    .transmit_raw_sim_apdu(simulation.clone(), apdu)
                    .await
            }
        }
    }

    /// Fetch the current tracked ISO/IEC 7816 session state for this connection.
    pub async fn session_state(&self) -> Result<IsoSessionState> {
        match &self.locator {
            CardConnectionLocator::Reader { reader_name } => {
                self.client
                    .get_card_session_state_on(crate::ReaderRef::named(reader_name.clone()))
                    .await
            }
            CardConnectionLocator::Simulation { simulation, .. } => {
                self.client
                    .get_simulation_session_state(simulation.clone())
                    .await
            }
        }
    }

    /// Reset the target behind this connection and return the typed reset summary.
    pub async fn reset_summary(&self) -> Result<ResetSummary> {
        match &self.locator {
            CardConnectionLocator::Reader { reader_name } => {
                self.client
                    .reset_card_summary_on(crate::ReaderRef::named(reader_name.clone()))
                    .await
            }
            CardConnectionLocator::Simulation { simulation, .. } => {
                self.client
                    .reset_simulation_summary(simulation.clone())
                    .await
            }
        }
    }

    /// Close this connection, stopping owned virtual simulations when applicable.
    pub async fn close(self) -> Result<()> {
        match self.locator {
            CardConnectionLocator::Reader { .. } => Ok(()),
            CardConnectionLocator::Simulation {
                simulation: _,
                owned: false,
            } => Ok(()),
            CardConnectionLocator::Simulation {
                simulation,
                owned: true,
            } => {
                self.client.stop_simulation(simulation).await?;
                Ok(())
            }
        }
    }
}

impl fmt::Debug for CardConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CardConnection")
            .field("kind", &self.kind())
            .field("locator", &self.locator)
            .finish()
    }
}
