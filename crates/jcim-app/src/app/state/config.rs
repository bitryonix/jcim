use super::*;

impl AppState {
    /// Return a cloned snapshot of the current persisted user configuration.
    pub(crate) fn user_config_snapshot(&self) -> Result<UserConfig> {
        self.user_config
            .read()
            .map_err(lock_poisoned)
            .map(|cfg| cfg.clone())
    }

    /// Update and persist the managed user configuration in one lock-scoped operation.
    pub(crate) fn persist_user_config(
        &self,
        update: impl FnOnce(&mut UserConfig),
    ) -> Result<UserConfig> {
        let mut user_config = self.user_config.write().map_err(lock_poisoned)?;
        update(&mut user_config);
        user_config.save_to_path(&self.managed_paths.config_path)?;
        Ok(user_config.clone())
    }

    /// Return the configured default card reader, if one is currently set.
    pub(crate) fn default_reader(&self) -> Result<Option<String>> {
        self.user_config
            .read()
            .map_err(lock_poisoned)
            .map(|cfg| cfg.default_reader.clone())
    }
}

#[cfg(test)]
mod tests {
    use jcim_config::project::UserConfig;

    use crate::app::testsupport::{load_test_app, temp_root};

    #[test]
    fn user_config_snapshot_persist_and_default_reader_round_trip() {
        let root = temp_root("state-config");
        let app = load_test_app(&root);

        let original = app.state.user_config_snapshot().expect("snapshot config");
        let updated = app
            .state
            .persist_user_config(|config| {
                config.default_reader = Some("Reader One".to_string());
                config.java_bin = "/custom/java".to_string();
            })
            .expect("persist user config");

        assert_eq!(original.default_reader, None);
        assert_eq!(updated.default_reader.as_deref(), Some("Reader One"));
        assert_eq!(updated.java_bin, "/custom/java");
        assert_eq!(
            app.state
                .default_reader()
                .expect("default reader")
                .as_deref(),
            Some("Reader One")
        );

        let saved = UserConfig::load_or_default(&app.state.managed_paths.config_path)
            .expect("reload saved config");
        assert_eq!(saved.default_reader.as_deref(), Some("Reader One"));
        assert_eq!(saved.java_bin, "/custom/java");

        let _ = std::fs::remove_dir_all(root);
    }
}
