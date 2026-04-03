use jcim_core::globalplatform;

use crate::error::{JcimSdkError, Result};
use crate::types::GpSecureChannelSummary;

use super::iso::{aid_from_proto, iso_session_state_from_proto};

/// Decode one GP secure-channel protobuf payload into the stable SDK summary type.
pub(in crate::client) fn gp_secure_channel_from_proto(
    info: Option<jcim_api::v0_3::GpSecureChannelInfo>,
) -> Result<GpSecureChannelSummary> {
    let info = info.ok_or_else(|| {
        JcimSdkError::InvalidResponse(
            "missing GP secure-channel summary in service response".to_string(),
        )
    })?;
    let protocol = match jcim_api::v0_3::SecureMessagingProtocol::try_from(info.protocol).ok() {
        Some(jcim_api::v0_3::SecureMessagingProtocol::Scp02) => globalplatform::ScpMode::Scp02,
        Some(jcim_api::v0_3::SecureMessagingProtocol::Scp03) => globalplatform::ScpMode::Scp03,
        _ => {
            return Err(JcimSdkError::InvalidResponse(
                "service returned a non-GP secure-messaging protocol for GP auth".to_string(),
            ));
        }
    };
    Ok(GpSecureChannelSummary {
        secure_channel: globalplatform::EstablishedSecureChannel {
            keyset: globalplatform::GpKeysetMetadata {
                name: info.keyset_name,
                mode: protocol,
            },
            security_level: globalplatform::SecurityLevel::Raw(info.security_level as u8),
            session_id: info.session_id,
        },
        selected_aid: aid_from_proto(info.selected_aid)?.ok_or_else(|| {
            JcimSdkError::InvalidResponse(
                "service omitted GP secure-channel selected AID".to_string(),
            )
        })?,
        session_state: iso_session_state_from_proto(info.session_state)?,
    })
}
