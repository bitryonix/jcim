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

    /// Resolve the CAP path to install, rebuilding artifacts first when project policy allows it.
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::app::testsupport::{
        load_test_app, load_test_app_with_adapter, project_selector, read_project_config,
        temp_root, write_project_config,
    };
    use crate::model::{CardAppletSummary, CardPackageSummary};
    use crate::registry::normalize_project_root;

    #[derive(Default)]
    struct RecordingAdapter {
        seen_readers: Mutex<Vec<Option<String>>>,
    }

    #[async_trait]
    impl PhysicalCardAdapter for RecordingAdapter {
        async fn list_readers(&self, _user_config: &UserConfig) -> Result<Vec<CardReaderSummary>> {
            Ok(vec![CardReaderSummary {
                name: "Adapter Reader".to_string(),
                card_present: true,
            }])
        }

        async fn card_status(
            &self,
            _user_config: &UserConfig,
            reader_name: Option<&str>,
        ) -> Result<CardStatusSummary> {
            self.seen_readers
                .lock()
                .expect("seen readers")
                .push(reader_name.map(str::to_string));
            let selected_aid = Aid::from_hex("A000000003000000").expect("aid");
            Ok(CardStatusSummary {
                reader_name: reader_name.unwrap_or_default().to_string(),
                card_present: true,
                atr: Some(Atr::parse(&hex::decode("3B800100").expect("atr")).expect("parse atr")),
                active_protocol: Some(ProtocolParameters::from_atr(
                    &Atr::parse(&hex::decode("3B800100").expect("atr")).expect("parse atr"),
                )),
                iso_capabilities: IsoCapabilities::default(),
                session_state: IsoSessionState {
                    selected_aid: Some(selected_aid),
                    ..IsoSessionState::default()
                },
                lines: vec!["status".to_string()],
            })
        }

        async fn install_cap(
            &self,
            _user_config: &UserConfig,
            _reader_name: Option<&str>,
            _cap_path: &Path,
        ) -> Result<Vec<String>> {
            Ok(vec!["installed".to_string()])
        }

        async fn delete_item(
            &self,
            _user_config: &UserConfig,
            _reader_name: Option<&str>,
            _aid: &str,
        ) -> Result<Vec<String>> {
            Ok(vec!["deleted".to_string()])
        }

        async fn list_packages(
            &self,
            _user_config: &UserConfig,
            _reader_name: Option<&str>,
        ) -> Result<CardPackageInventory> {
            Ok(CardPackageInventory {
                reader_name: String::new(),
                packages: vec![CardPackageSummary {
                    aid: "A000000001".to_string(),
                    description: "demo".to_string(),
                }],
                output_lines: vec!["pkg".to_string()],
            })
        }

        async fn list_applets(
            &self,
            _user_config: &UserConfig,
            _reader_name: Option<&str>,
        ) -> Result<CardAppletInventory> {
            Ok(CardAppletInventory {
                reader_name: String::new(),
                applets: vec![CardAppletSummary {
                    aid: "A000000002".to_string(),
                    description: "demo".to_string(),
                }],
                output_lines: vec!["app".to_string()],
            })
        }

        async fn transmit_apdu(
            &self,
            _user_config: &UserConfig,
            _reader_name: Option<&str>,
            _apdu_hex: &str,
        ) -> Result<String> {
            Ok("9000".to_string())
        }

        async fn reset_card(
            &self,
            _user_config: &UserConfig,
            _reader_name: Option<&str>,
        ) -> Result<String> {
            Ok("3B800100".to_string())
        }
    }

    #[tokio::test]
    async fn card_service_helpers_use_default_readers_and_fill_inventory_names() {
        let root = temp_root("inventory-reader");
        let adapter = Arc::new(RecordingAdapter::default());
        let app = load_test_app_with_adapter(&root, adapter.clone());
        app.state
            .persist_user_config(|config| {
                config.default_reader = Some("Configured Reader".to_string());
            })
            .expect("persist user config");

        let status = app.card_status(None).await.expect("card status");
        let packages = app.list_packages(None).await.expect("packages");
        let applets = app.list_applets(None).await.expect("applets");

        assert_eq!(status.reader_name, "Configured Reader");
        assert_eq!(packages.reader_name, "Configured Reader");
        assert_eq!(applets.reader_name, "Configured Reader");
        assert_eq!(
            adapter
                .seen_readers
                .lock()
                .expect("seen readers")
                .as_slice(),
            &[Some("Configured Reader".to_string())]
        );
        assert_eq!(
            app.card_session_state(None)
                .expect("session state")
                .selected_aid,
            status.session_state.selected_aid
        );

        let reset = app.reset_card_summary(None).await.expect("reset");
        assert!(reset.atr.is_some());
        assert_eq!(
            app.card_session_state(None).expect("reset session"),
            reset.session_state
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_install_cap_path_prefers_explicit_card_cap_path() {
        let root = temp_root("inventory-cap-path");
        let app = load_test_app(&root);
        let project_root = root.join("demo");
        app.create_project("Demo", &project_root)
            .expect("create project");
        let selector = project_selector(&project_root);
        let (_, artifacts, _) = app.build_project(&selector).expect("build project");
        let cap_artifact = artifacts
            .iter()
            .find(|artifact| artifact.kind == "cap")
            .expect("cap artifact");
        let normalized_root = normalize_project_root(&project_root).expect("normalize root");
        let relative_cap = cap_artifact
            .path
            .strip_prefix(&normalized_root)
            .expect("relative cap path")
            .to_path_buf();

        let mut config = read_project_config(&project_root);
        config.card.default_cap_path = Some(relative_cap.clone());
        write_project_config(&project_root, &config);

        let resolved = app
            .resolve_install_cap_path(&selector)
            .expect("resolve explicit cap path");
        assert_eq!(resolved, normalized_root.join(relative_cap));

        let _ = std::fs::remove_dir_all(root);
    }
}
