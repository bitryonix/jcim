use jcim_api::v0_3::{
    AppletInfo, CardAppletInfo, CardPackageInfo, DeleteItemResponse, GpSecureChannelInfo,
    InstallCapResponse, ListAppletsResponse, ListPackagesResponse,
};
use jcim_app::{
    CardAppletInventory, CardDeleteSummary, CardInstallSummary, CardPackageInventory,
    GpSecureChannelSummary,
};

use super::iso::{aid_info, iso_session_state_info};

/// Encode a card-install summary into the RPC install response.
pub(crate) fn install_cap_response(summary: CardInstallSummary) -> InstallCapResponse {
    InstallCapResponse {
        reader_name: summary.reader_name,
        cap_path: summary.cap_path.display().to_string(),
        package_name: summary.package_name,
        package_aid: summary.package_aid,
        applets: summary
            .applets
            .into_iter()
            .map(|applet| AppletInfo {
                class_name: applet.class_name,
                aid: applet.aid,
            })
            .collect(),
        output_lines: summary.output_lines,
    }
}

/// Encode a card-delete summary into the RPC delete response.
pub(crate) fn delete_item_response(summary: CardDeleteSummary) -> DeleteItemResponse {
    DeleteItemResponse {
        reader_name: summary.reader_name,
        aid: summary.aid,
        deleted: summary.deleted,
        output_lines: summary.output_lines,
    }
}

/// Encode a card-package inventory snapshot into the RPC package-list response.
pub(crate) fn package_inventory_response(inventory: CardPackageInventory) -> ListPackagesResponse {
    ListPackagesResponse {
        reader_name: inventory.reader_name,
        packages: inventory
            .packages
            .into_iter()
            .map(|package| CardPackageInfo {
                aid: package.aid,
                description: package.description,
            })
            .collect(),
        output_lines: inventory.output_lines,
    }
}

/// Encode a card-applet inventory snapshot into the RPC applet-list response.
pub(crate) fn applet_inventory_response(inventory: CardAppletInventory) -> ListAppletsResponse {
    ListAppletsResponse {
        reader_name: inventory.reader_name,
        applets: inventory
            .applets
            .into_iter()
            .map(|applet| CardAppletInfo {
                aid: applet.aid,
                description: applet.description,
            })
            .collect(),
        output_lines: inventory.output_lines,
    }
}

/// Encode one established GP secure channel into the RPC transport summary.
pub(crate) fn gp_secure_channel_info(summary: &GpSecureChannelSummary) -> GpSecureChannelInfo {
    let protocol = match summary.secure_channel.keyset.mode {
        jcim_core::globalplatform::ScpMode::Scp02 => {
            jcim_api::v0_3::SecureMessagingProtocol::Scp02 as i32
        }
        jcim_core::globalplatform::ScpMode::Scp03 => {
            jcim_api::v0_3::SecureMessagingProtocol::Scp03 as i32
        }
    };
    GpSecureChannelInfo {
        keyset_name: summary.secure_channel.keyset.name.clone(),
        protocol,
        security_level: u32::from(summary.secure_channel.security_level.as_byte()),
        session_id: summary.secure_channel.session_id.clone(),
        selected_aid: Some(aid_info(&summary.selected_aid)),
        session_state: Some(iso_session_state_info(&summary.session_state)),
    }
}
