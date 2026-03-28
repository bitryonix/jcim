use super::*;

impl AppState {
    pub(crate) fn user_config_snapshot(&self) -> Result<UserConfig> {
        self.user_config
            .read()
            .map_err(lock_poisoned)
            .map(|cfg| cfg.clone())
    }

    pub(crate) fn persist_user_config(
        &self,
        update: impl FnOnce(&mut UserConfig),
    ) -> Result<UserConfig> {
        let mut user_config = self.user_config.write().map_err(lock_poisoned)?;
        update(&mut user_config);
        user_config.save_to_path(&self.managed_paths.config_path)?;
        Ok(user_config.clone())
    }

    pub(crate) fn default_reader(&self) -> Result<Option<String>> {
        self.user_config
            .read()
            .map_err(lock_poisoned)
            .map(|cfg| cfg.default_reader.clone())
    }
}
