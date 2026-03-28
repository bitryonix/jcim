use super::*;

impl JcimApp {
    /// List physical PC/SC readers.
    pub async fn list_readers(&self) -> Result<Vec<CardReaderSummary>> {
        let user_config = self.effective_user_config()?;
        self.state.card_adapter.list_readers(&user_config).await
    }

    /// Return physical-card status for one reader.
    pub async fn card_status(&self, reader_name: Option<&str>) -> Result<CardStatusSummary> {
        let user_config = self.effective_user_config()?;
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let status = self
            .state
            .card_adapter
            .card_status(&user_config, effective_reader.as_deref())
            .await?;
        let _ = self
            .state
            .sync_card_status(&status.reader_name, &status.session_state);
        Ok(status)
    }

    /// Install one project's CAP onto a physical card.
    pub async fn install_project_cap(
        &self,
        selector: &ProjectSelectorInput,
        reader_name: Option<&str>,
    ) -> Result<CardInstallSummary> {
        let effective_cap = self.resolve_install_cap_path(selector)?;
        self.install_cap_from_path(&effective_cap, reader_name, Some(selector))
            .await
    }

    /// Install one explicit CAP onto a physical card.
    pub async fn install_cap_from_path(
        &self,
        cap_path: &Path,
        reader_name: Option<&str>,
        selector: Option<&ProjectSelectorInput>,
    ) -> Result<CardInstallSummary> {
        let effective_cap = self.resolve_input_path(cap_path)?;
        let effective_reader = self.effective_card_reader(reader_name, selector)?;
        let cap_package = CapPackage::from_path(&effective_cap)?;
        let user_config = self.effective_user_config()?;
        let output_lines = self
            .state
            .card_adapter
            .install_cap(&user_config, effective_reader.as_deref(), &effective_cap)
            .await?;
        Ok(CardInstallSummary {
            reader_name: effective_reader.unwrap_or_default(),
            cap_path: effective_cap,
            package_name: cap_package.package_name,
            package_aid: cap_package.package_aid.to_hex(),
            applets: cap_package
                .applets
                .into_iter()
                .map(|applet| AppletSummary {
                    class_name: applet.name.unwrap_or_else(|| "InstalledApplet".to_string()),
                    aid: applet.aid.to_hex(),
                })
                .collect(),
            output_lines,
        })
    }

    /// Delete one package from a physical card.
    pub async fn delete_item(
        &self,
        reader_name: Option<&str>,
        aid: &str,
    ) -> Result<CardDeleteSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let output_lines = self
            .state
            .card_adapter
            .delete_item(&user_config, effective_reader.as_deref(), aid)
            .await?;
        Ok(CardDeleteSummary {
            reader_name: effective_reader.unwrap_or_default(),
            aid: aid.to_string(),
            deleted: true,
            output_lines,
        })
    }

    /// List packages visible on a physical card.
    pub async fn list_packages(&self, reader_name: Option<&str>) -> Result<CardPackageInventory> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let mut inventory = self
            .state
            .card_adapter
            .list_packages(&user_config, effective_reader.as_deref())
            .await?;
        if inventory.reader_name.is_empty() {
            inventory.reader_name = effective_reader.unwrap_or_default();
        }
        Ok(inventory)
    }

    /// List applets visible on a physical card.
    pub async fn list_applets(&self, reader_name: Option<&str>) -> Result<CardAppletInventory> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let mut inventory = self
            .state
            .card_adapter
            .list_applets(&user_config, effective_reader.as_deref())
            .await?;
        if inventory.reader_name.is_empty() {
            inventory.reader_name = effective_reader.unwrap_or_default();
        }
        Ok(inventory)
    }

    /// Reset a physical card and return the ATR.
    pub async fn reset_card_summary(&self, reader_name: Option<&str>) -> Result<ResetSummary> {
        let effective_reader = self.effective_card_reader(reader_name, None)?;
        let user_config = self.effective_user_config()?;
        let summary = self
            .state
            .card_adapter
            .reset_card_summary(&user_config, effective_reader.as_deref())
            .await?;
        let _ = self.state.reset_card_session(
            &effective_reader.unwrap_or_default(),
            &summary.session_state,
        );
        Ok(summary)
    }

    fn resolve_install_cap_path(&self, selector: &ProjectSelectorInput) -> Result<PathBuf> {
        let resolved = self.resolve_project(selector)?;
        if let Some(cap_path) = &resolved.config.card.default_cap_path {
            return Ok(resolve_project_path(&resolved.project_root, cap_path));
        }

        let metadata = if resolved.config.card.auto_build_before_install {
            let request = artifact_metadata_from_project(&resolved.project_root, &resolved.config)?;
            let toolchain = build_toolchain_layout()?;
            let java_runtime = self.resolved_java_runtime()?;
            build_project_artifacts_if_stale_with_java_bin(
                &request,
                &toolchain,
                &java_runtime.java_bin,
            )?
            .metadata
        } else {
            load_artifact_metadata(&resolved.project_root)?.ok_or_else(|| {
                JcimError::Unsupported(
                    "no CAP artifact is recorded for this project and automatic card builds are disabled".to_string(),
                )
            })?
        };

        required_artifact_path(
            &resolved.project_root,
            metadata.cap_path.as_ref(),
            "the selected project does not provide a CAP artifact for card install",
        )
    }
}
