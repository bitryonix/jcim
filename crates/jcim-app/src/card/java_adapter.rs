use super::helper_tool::{reader_arg_list, run_card_helper, run_card_helper_with_env, run_gppro};
use super::inventory_parser::{parse_applet_inventory, parse_package_inventory};
use super::*;

/// Default adapter backed by the bundled card helper and GPPro.
#[derive(Default)]
pub(crate) struct JavaPhysicalCardAdapter;

#[async_trait]
impl PhysicalCardAdapter for JavaPhysicalCardAdapter {
    async fn list_readers(&self, user_config: &UserConfig) -> Result<Vec<CardReaderSummary>> {
        let output = run_card_helper(user_config, "readers", None, &[]).await?;
        let mut readers = Vec::new();
        for line in output.lines() {
            let Some((name, present)) = line.split_once('\t') else {
                continue;
            };
            readers.push(CardReaderSummary {
                name: name.to_string(),
                card_present: present.trim() == "present=1",
            });
        }
        Ok(readers)
    }

    async fn card_status(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardStatusSummary> {
        let output = run_card_helper(user_config, "status", reader_name, &[]).await?;
        let lines = output
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        let mut effective_reader = reader_name.unwrap_or_default().to_string();
        let mut card_present = false;
        let mut protocol_text = String::new();
        let mut atr_hex = String::new();
        for line in &lines {
            if let Some(value) = line.strip_prefix("Reader: ") {
                effective_reader = value.to_string();
            } else if let Some(value) = line.strip_prefix("Card present: ") {
                card_present = value == "yes";
            } else if let Some(value) = line.strip_prefix("Protocol: ") {
                protocol_text = value.to_string();
            } else if let Some(value) = line.strip_prefix("ATR: ") {
                atr_hex = value.to_string();
            }
        }
        let atr = hex::decode(&atr_hex)
            .ok()
            .and_then(|raw| Atr::parse(&raw).ok());
        let active_protocol = atr.as_ref().map(ProtocolParameters::from_atr).or_else(|| {
            TransportProtocol::from_status_text(&protocol_text).map(|protocol| ProtocolParameters {
                protocol: Some(protocol),
                ..ProtocolParameters::default()
            })
        });
        let session_state = if card_present {
            IsoSessionState::reset(atr.clone(), active_protocol.clone())
        } else {
            IsoSessionState::default()
        };
        let iso_capabilities = IsoCapabilities {
            protocols: active_protocol
                .as_ref()
                .and_then(|protocol| protocol.protocol)
                .into_iter()
                .collect(),
            extended_length: false,
            logical_channels: false,
            max_logical_channels: 1,
            secure_messaging: false,
            file_model_visibility: false,
            raw_apdu: true,
        };
        Ok(CardStatusSummary {
            reader_name: effective_reader,
            card_present,
            atr,
            active_protocol,
            iso_capabilities,
            session_state,
            lines,
        })
    }

    async fn install_cap(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        cap_path: &Path,
    ) -> Result<Vec<String>> {
        let mut args = reader_arg_list(reader_name);
        args.push("-install".to_string());
        args.push(cap_path.display().to_string());
        Ok(run_gppro(user_config, &args).await?.output_lines)
    }

    async fn delete_item(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        aid: &str,
    ) -> Result<Vec<String>> {
        let mut args = reader_arg_list(reader_name);
        args.push("-delete".to_string());
        args.push(aid.to_string());
        Ok(run_gppro(user_config, &args).await?.output_lines)
    }

    async fn list_packages(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardPackageInventory> {
        let mut args = reader_arg_list(reader_name);
        args.push("-l".to_string());
        let output = run_gppro(user_config, &args).await?;
        Ok(CardPackageInventory {
            reader_name: reader_name.unwrap_or_default().to_string(),
            packages: parse_package_inventory(&output.output_lines),
            output_lines: output.output_lines,
        })
    }

    async fn list_applets(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<CardAppletInventory> {
        let mut args = reader_arg_list(reader_name);
        args.push("-l".to_string());
        let output = run_gppro(user_config, &args).await?;
        Ok(CardAppletInventory {
            reader_name: reader_name.unwrap_or_default().to_string(),
            applets: parse_applet_inventory(&output.output_lines),
            output_lines: output.output_lines,
        })
    }

    async fn transmit_apdu(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        apdu_hex: &str,
    ) -> Result<String> {
        let args = vec!["--hex".to_string(), apdu_hex.to_string()];
        let output = run_card_helper(user_config, "apdu", reader_name, &args).await?;
        output
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .ok_or_else(|| {
                JcimError::Unsupported("card helper returned no APDU response".to_string())
            })
    }

    async fn reset_card(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
    ) -> Result<String> {
        let output = run_card_helper(user_config, "reset", reader_name, &[]).await?;
        output
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .ok_or_else(|| {
                JcimError::Unsupported("card helper returned no ATR after reset".to_string())
            })
    }

    async fn open_gp_secure_channel(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        keyset: &ResolvedGpKeyset,
        security_level: u8,
    ) -> Result<()> {
        let args = vec![
            "--security-level".to_string(),
            format!("0x{security_level:02X}"),
        ];
        let _ = run_card_helper_with_env(
            user_config,
            "gp-auth-open",
            reader_name,
            &args,
            Some(keyset),
        )
        .await?;
        Ok(())
    }

    async fn transmit_gp_secure_command(
        &self,
        user_config: &UserConfig,
        reader_name: Option<&str>,
        keyset: &ResolvedGpKeyset,
        security_level: u8,
        command: &CommandApdu,
    ) -> Result<ResponseApdu> {
        let args = vec![
            "--security-level".to_string(),
            format!("0x{security_level:02X}"),
            "--hex".to_string(),
            hex::encode_upper(command.to_bytes()),
        ];
        let output = run_card_helper_with_env(
            user_config,
            "gp-secure-apdu",
            reader_name,
            &args,
            Some(keyset),
        )
        .await?;
        let response_hex = output
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .ok_or_else(|| {
                JcimError::Unsupported(
                    "card helper returned no authenticated GP APDU response".to_string(),
                )
            })?;
        ResponseApdu::parse(&hex::decode(&response_hex)?)
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::path::Path;

    use jcim_core::globalplatform::{GetStatusOccurrence, RegistryKind};

    use super::super::helper_tool::{fake_java_user_config, host_tool_temp_root, write_fake_java};
    use super::*;

    const FAKE_JAVA_SCRIPT: &str = r#"#!/bin/sh
set -eu
DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
printf '%s\n' "$@" > "$DIR/args.log"
{
  printf 'JCIM_GP_MODE=%s\n' "${JCIM_GP_MODE-}"
  printf 'JCIM_GP_ENC=%s\n' "${JCIM_GP_ENC-}"
  printf 'JCIM_GP_MAC=%s\n' "${JCIM_GP_MAC-}"
  printf 'JCIM_GP_DEK=%s\n' "${JCIM_GP_DEK-}"
} > "$DIR/env.log"

reader=""
hex=""
if [ "${1-}" = "-cp" ]; then
  action="${4-}"
  shift 4
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --reader)
        reader="${2-}"
        shift 2
        ;;
      --hex)
        hex="${2-}"
        shift 2
        ;;
      --security-level)
        shift 2
        ;;
      *)
        shift 1
        ;;
    esac
  done
  case "$action" in
    readers)
      printf 'Reader A\tpresent=1\n'
      printf 'invalid line\n'
      printf 'Reader B\tpresent=0\n'
      ;;
    status)
      if [ "$reader" = "No Card" ]; then
        printf 'Reader: %s\n' "$reader"
        printf 'Card present: no\n'
        printf 'Protocol: T=0\n'
      else
        printf 'Reader: %s\n' "${reader:-Reader A}"
        printf 'Card present: yes\n'
        printf 'Protocol: T=1\n'
        printf 'ATR: 3B800100\n'
      fi
      ;;
    apdu)
      if [ "$hex" = "EMPTY" ]; then
        :
      else
        printf '9000\n'
      fi
      ;;
    reset)
      if [ "$reader" = "Empty Reader" ]; then
        :
      else
        printf '3B800100\n'
      fi
      ;;
    gp-auth-open)
      printf 'gp auth ok\n'
      ;;
    gp-secure-apdu)
      printf '9000\n'
      ;;
    *)
      printf 'unexpected helper action: %s\n' "$action" >&2
      exit 9
      ;;
  esac
elif [ "${1-}" = "-jar" ]; then
  shift 2
  saw_list=0
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "-l" ]; then
      saw_list=1
    fi
    shift 1
  done
  if [ "$saw_list" = "1" ]; then
    printf 'PKG: A000000151000000 demo.package 1.0\n'
    printf 'APP: A000000151000001 DemoApplet\n'
    printf 'noise\n'
  else
    printf 'unexpected gppro args\n' >&2
    exit 8
  fi
else
  printf 'unexpected invocation\n' >&2
  exit 10
fi
"#;

    #[tokio::test]
    async fn java_adapter_parses_reader_status_and_inventory_outputs() {
        let root = host_tool_temp_root("java-adapter-parse");
        let java_bin = write_fake_java(&root, FAKE_JAVA_SCRIPT);
        let user_config = fake_java_user_config(&java_bin);
        let adapter = JavaPhysicalCardAdapter;

        let readers = adapter
            .list_readers(&user_config)
            .await
            .expect("list readers");
        assert_eq!(
            readers,
            vec![
                CardReaderSummary {
                    name: "Reader A".to_string(),
                    card_present: true,
                },
                CardReaderSummary {
                    name: "Reader B".to_string(),
                    card_present: false,
                },
            ]
        );

        let status = adapter
            .card_status(&user_config, Some("Reader A"))
            .await
            .expect("card status");
        assert_eq!(status.reader_name, "Reader A");
        assert!(status.card_present);
        assert_eq!(
            status.atr,
            Some(Atr::parse(&[0x3B, 0x80, 0x01, 0x00]).expect("atr"))
        );
        assert_eq!(
            status
                .active_protocol
                .and_then(|protocol| protocol.protocol),
            Some(TransportProtocol::T1)
        );
        assert!(status.session_state.atr.is_some());

        let no_card = adapter
            .card_status(&user_config, Some("No Card"))
            .await
            .expect("status without card");
        assert_eq!(no_card.reader_name, "No Card");
        assert!(!no_card.card_present);
        assert_eq!(no_card.atr, None);
        assert_eq!(no_card.session_state, IsoSessionState::default());

        let packages = adapter
            .list_packages(&user_config, Some("Reader A"))
            .await
            .expect("packages");
        assert_eq!(packages.reader_name, "Reader A");
        assert_eq!(packages.packages.len(), 1);
        assert_eq!(packages.packages[0].aid, "A000000151000000");

        let applets = adapter
            .list_applets(&user_config, Some("Reader A"))
            .await
            .expect("applets");
        assert_eq!(applets.reader_name, "Reader A");
        assert_eq!(applets.applets.len(), 1);
        assert_eq!(applets.applets[0].aid, "A000000151000001");

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn java_adapter_fails_closed_on_empty_outputs_and_forwards_gp_helper_inputs() {
        let root = host_tool_temp_root("java-adapter-gp");
        let java_bin = write_fake_java(&root, FAKE_JAVA_SCRIPT);
        let user_config = fake_java_user_config(&java_bin);
        let adapter = JavaPhysicalCardAdapter;

        assert_eq!(
            adapter
                .transmit_apdu(&user_config, Some("Reader A"), "00A4040000")
                .await
                .expect("transmit apdu"),
            "9000"
        );
        let empty_apdu = adapter
            .transmit_apdu(&user_config, Some("Reader A"), "EMPTY")
            .await
            .expect_err("empty APDU output should fail");
        assert!(empty_apdu.to_string().contains("returned no APDU response"));

        assert_eq!(
            adapter
                .reset_card(&user_config, Some("Reader A"))
                .await
                .expect("reset card"),
            "3B800100"
        );
        let empty_reset = adapter
            .reset_card(&user_config, Some("Empty Reader"))
            .await
            .expect_err("empty ATR output should fail");
        assert!(
            empty_reset
                .to_string()
                .contains("returned no ATR after reset")
        );

        let keyset = ResolvedGpKeyset::resolve(Some("__test__")).expect("test keyset");
        adapter
            .open_gp_secure_channel(&user_config, Some("Reader A"), &keyset, 0x03)
            .await
            .expect("open gp secure channel");
        assert_eq!(
            read_lines(&root.join("args.log")),
            vec![
                "-cp".to_string(),
                format!(
                    "{}:{}",
                    helper_jar_path().display(),
                    gppro_jar_path().display()
                ),
                "jcim.cardhelper.Main".to_string(),
                "gp-auth-open".to_string(),
                "--reader".to_string(),
                "Reader A".to_string(),
                "--security-level".to_string(),
                "0x03".to_string(),
            ]
        );
        let env = read_lines(&root.join("env.log"));
        assert!(env.contains(&"JCIM_GP_MODE=scp03".to_string()));

        let response = adapter
            .transmit_gp_secure_command(
                &user_config,
                Some("Reader A"),
                &keyset,
                0x03,
                &globalplatform::get_status(
                    RegistryKind::Applications,
                    GetStatusOccurrence::FirstOrAll,
                ),
            )
            .await
            .expect("transmit gp secure command");
        assert_eq!(response.sw, 0x9000);
        let args = read_lines(&root.join("args.log"));
        assert_eq!(args[3], "gp-secure-apdu");
        assert_eq!(args[4], "--reader");
        assert_eq!(args[5], "Reader A");
        assert_eq!(args[6], "--security-level");
        assert_eq!(args[7], "0x03");
        assert_eq!(args[8], "--hex");
        assert_eq!(
            args[9],
            hex::encode_upper(
                globalplatform::get_status(
                    RegistryKind::Applications,
                    GetStatusOccurrence::FirstOrAll,
                )
                .to_bytes()
            )
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn read_lines(path: &Path) -> Vec<String> {
        std::fs::read_to_string(path)
            .expect("read log")
            .lines()
            .map(str::to_string)
            .collect()
    }
}
