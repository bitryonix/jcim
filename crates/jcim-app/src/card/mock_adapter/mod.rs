use super::*;

mod dispatch;
#[path = "globalplatform.rs"]
mod gp;
mod inventory;
mod iso;
mod state;

#[cfg(test)]
mod tests;

use self::dispatch::mock_dispatch_apdu;
use self::gp::{apply_pending_gp_external_auth, open_mock_gp_secure_channel};
use self::inventory::{
    mock_card_status, mock_delete_item, mock_install_cap, mock_list_applets, mock_list_packages,
    mock_list_readers, mock_reset_card,
};
use self::state::{MockCardState, lock_poisoned};

/// Deterministic in-memory test adapter for service and SDK integration tests.
#[derive(Clone)]
pub struct MockPhysicalCardAdapter {
    state: Arc<Mutex<MockCardState>>,
}

impl MockPhysicalCardAdapter {
    /// Build a mock adapter with one default reader and blank card state.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockCardState::new())),
        }
    }
}

impl Default for MockPhysicalCardAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PhysicalCardAdapter for MockPhysicalCardAdapter {
    async fn list_readers(&self, _user_config: &UserConfig) -> Result<Vec<CardReaderSummary>> {
        mock_list_readers(self)
    }

    async fn card_status(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardStatusSummary> {
        mock_card_status(self, reader_name)
    }

    async fn install_cap(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
        cap_path: &Path,
    ) -> Result<Vec<String>> {
        mock_install_cap(self, reader_name, cap_path)
    }

    async fn delete_item(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
        aid: &str,
    ) -> Result<Vec<String>> {
        mock_delete_item(self, reader_name, aid)
    }

    async fn list_packages(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardPackageInventory> {
        mock_list_packages(self, reader_name)
    }

    async fn list_applets(
        &self,
        _user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardAppletInventory> {
        mock_list_applets(self, reader_name)
    }

    async fn transmit_apdu(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
        apdu_hex: &str,
    ) -> Result<String> {
        let apdu = CommandApdu::parse(&hex::decode(apdu_hex)?)?;
        let mut state = self.state.lock().map_err(lock_poisoned)?;
        let response = mock_dispatch_apdu(&mut state, &apdu)?;
        let _ = apply_response_to_session(&mut state.session_state, &apdu, &response);
        apply_pending_gp_external_auth(&mut state, &apdu, &response);
        Ok(hex::encode_upper(response.to_bytes()))
    }

    async fn reset_card(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
    ) -> Result<String> {
        mock_reset_card(self)
    }

    async fn open_gp_secure_channel(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
        keyset: &ResolvedGpKeyset,
        security_level: u8,
    ) -> Result<()> {
        open_mock_gp_secure_channel(self, keyset, security_level)
    }

    async fn transmit_gp_secure_command(
        &self,
        _user_config: &UserConfig,
        _reader_name: Option<&str>,
        _keyset: &ResolvedGpKeyset,
        _security_level: u8,
        command: &CommandApdu,
    ) -> Result<ResponseApdu> {
        let mut state = self.state.lock().map_err(lock_poisoned)?;
        if !state.session_state.secure_messaging.active {
            return Err(JcimError::Unsupported(
                "mock card GP secure channel is not open".to_string(),
            ));
        }
        let response = mock_dispatch_apdu(&mut state, command)?;
        let _ = apply_response_to_session(&mut state.session_state, command, &response);
        state.session_state.secure_messaging.command_counter = state
            .session_state
            .secure_messaging
            .command_counter
            .saturating_add(1);
        Ok(response)
    }
}
