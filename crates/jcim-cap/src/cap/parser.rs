use jcim_core::error::{JcimError, Result};

use super::CapPackage;
use super::archive::parse_zip_entries;
use super::components::{
    infer_package_name, manifest_applets, parse_applet_component, parse_header_component,
    parse_import_component, parse_version, read_component, read_manifest, read_optional_component,
};

/// Parse one CAP archive into the stable `CapPackage` model exposed by this crate.
pub(super) fn parse_cap_package(bytes: Vec<u8>) -> Result<CapPackage> {
    let archive = parse_zip_entries(&bytes)?;
    let manifest = read_manifest(&archive);
    let header = read_component(&archive, "Header.cap")?;
    let applet_component = read_optional_component(&archive, "Applet.cap");
    let import_component = read_optional_component(&archive, "Import.cap");

    let (fallback_version, package_major, package_minor, fallback_aid, fallback_name) =
        parse_header_component(header)?;
    let version = manifest
        .get("Java-Card-CAP-File-Version")
        .and_then(|value| parse_version(value))
        .unwrap_or(fallback_version);
    if !version.is_supported() {
        return Err(JcimError::UnsupportedCapVersion(format!(
            "{}.{}",
            version.major, version.minor
        )));
    }

    let package_aid = manifest
        .get("Java-Card-Package-AID")
        .map(|value| jcim_core::aid::Aid::from_hex(value))
        .transpose()?
        .unwrap_or(fallback_aid);
    let package_name = manifest
        .get("Java-Card-Package-Name")
        .cloned()
        .or_else(|| infer_package_name(&archive))
        .unwrap_or(fallback_name);
    let applets = manifest_applets(&manifest)
        .or_else(|| applet_component.map(parse_applet_component))
        .transpose()?
        .unwrap_or_default();
    let imports = import_component
        .map(parse_import_component)
        .transpose()?
        .unwrap_or_default();

    Ok(CapPackage {
        version,
        package_aid,
        package_name,
        package_major,
        package_minor,
        imports,
        applets,
        manifest,
        raw_bytes: bytes,
    })
}
