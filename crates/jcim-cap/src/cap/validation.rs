use jcim_core::error::{JcimError, Result};
use jcim_core::model::CardProfile;

use super::CapPackage;

pub(super) fn validate_for_profile(package: &CapPackage, profile: &CardProfile) -> Result<()> {
    if package.version.major != 2 || !profile.supports_cap_minor(package.version.minor) {
        return Err(JcimError::Unsupported(format!(
            "CAP version {}.{} is not compatible with Classic profile {}",
            package.version.major,
            package.version.minor,
            profile.version.display_name()
        )));
    }
    Ok(())
}
