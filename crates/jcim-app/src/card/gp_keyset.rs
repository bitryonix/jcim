use super::*;

/// One env-resolved GlobalPlatform keyset retained only in process memory.
#[derive(Clone)]
pub struct ResolvedGpKeyset {
    pub(crate) name: String,
    pub(crate) mode: globalplatform::ScpMode,
    pub(super) enc_hex: String,
    pub(super) mac_hex: String,
    pub(super) dek_hex: String,
}

impl ResolvedGpKeyset {
    /// Resolve one named GP keyset from environment variables only.
    pub(crate) fn resolve(explicit_name: Option<&str>) -> Result<Self> {
        let name = match explicit_name {
            Some(name) if !name.trim().is_empty() => name.trim().to_string(),
            _ => std::env::var("JCIM_GP_DEFAULT_KEYSET").map_err(|_| {
                JcimError::Unsupported(
                    "missing GP keyset name: pass one explicitly or set JCIM_GP_DEFAULT_KEYSET"
                        .to_string(),
                )
            })?,
        };
        let env_prefix = format!("JCIM_GP_{}", gp_keyset_env_name(&name));
        let mode = match std::env::var(format!("{env_prefix}_MODE"))
            .map_err(|_| {
                JcimError::Unsupported(format!("missing {}_MODE for GP keyset {name}", env_prefix))
            })?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "scp02" => globalplatform::ScpMode::Scp02,
            "scp03" => globalplatform::ScpMode::Scp03,
            other => {
                return Err(JcimError::Unsupported(format!(
                    "unsupported GP mode `{other}` for keyset {name}"
                )));
            }
        };
        let enc_hex = required_gp_key_hex(&env_prefix, "ENC", &name)?;
        let mac_hex = required_gp_key_hex(&env_prefix, "MAC", &name)?;
        let dek_hex = required_gp_key_hex(&env_prefix, "DEK", &name)?;
        Ok(Self {
            name,
            mode,
            enc_hex,
            mac_hex,
            dek_hex,
        })
    }

    pub(crate) fn metadata(&self) -> globalplatform::GpKeysetMetadata {
        globalplatform::GpKeysetMetadata {
            name: self.name.clone(),
            mode: self.mode,
        }
    }

    pub(crate) fn protocol(&self) -> SecureMessagingProtocol {
        match self.mode {
            globalplatform::ScpMode::Scp02 => SecureMessagingProtocol::Scp02,
            globalplatform::ScpMode::Scp03 => SecureMessagingProtocol::Scp03,
        }
    }

    pub(super) fn apply_helper_env(&self, command: &mut Command) {
        command
            .env("JCIM_GP_MODE", self.mode_label())
            .env("JCIM_GP_ENC", &self.enc_hex)
            .env("JCIM_GP_MAC", &self.mac_hex)
            .env("JCIM_GP_DEK", &self.dek_hex);
    }

    fn mode_label(&self) -> &'static str {
        match self.mode {
            globalplatform::ScpMode::Scp02 => "scp02",
            globalplatform::ScpMode::Scp03 => "scp03",
        }
    }
}

fn gp_keyset_env_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn required_gp_key_hex(env_prefix: &str, suffix: &str, keyset_name: &str) -> Result<String> {
    let variable = format!("{env_prefix}_{suffix}");
    let value = std::env::var(&variable).map_err(|_| {
        JcimError::Unsupported(format!("missing {variable} for GP keyset {keyset_name}"))
    })?;
    let normalized = value.trim().to_string();
    let bytes = hex::decode(&normalized).map_err(|error| {
        JcimError::Unsupported(format!(
            "invalid {variable} for GP keyset {keyset_name}: {error}"
        ))
    })?;
    if !matches!(bytes.len(), 16 | 24 | 32) {
        return Err(JcimError::Unsupported(format!(
            "{variable} for GP keyset {keyset_name} must be 16, 24, or 32 bytes"
        )));
    }
    Ok(normalized)
}
