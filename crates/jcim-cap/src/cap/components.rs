use std::collections::BTreeMap;

use jcim_core::aid::Aid;
use jcim_core::error::{JcimError, Result};

use super::error::cap_format;
use super::{CapApplet, CapFileVersion, ImportedPackage};

/// Read `META-INF/MANIFEST.MF` into a key-value map when present.
pub(super) fn read_manifest(archive: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    let Some(raw) = archive.get("META-INF/MANIFEST.MF") else {
        return entries;
    };
    let manifest = String::from_utf8_lossy(raw);
    for line in manifest.lines() {
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some((_, value)) = entries.iter_mut().next_back() {
                value.push_str(rest);
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            entries.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    entries
}

/// Infer a package name from the archive layout when the manifest does not provide one.
pub(super) fn infer_package_name(archive: &BTreeMap<String, Vec<u8>>) -> Option<String> {
    archive.keys().find_map(|name| {
        name.strip_suffix("/Header.cap")
            .map(|prefix| prefix.replace("/javacard", "").replace('/', "."))
    })
}

/// Read a required CAP component from the archive.
pub(super) fn read_component<'a>(
    archive: &'a BTreeMap<String, Vec<u8>>,
    component_name: &'static str,
) -> Result<&'a [u8]> {
    read_optional_component(archive, component_name)
        .ok_or(JcimError::MissingCapComponent(component_name))
}

/// Read an optional CAP component from the archive when it exists.
pub(super) fn read_optional_component<'a>(
    archive: &'a BTreeMap<String, Vec<u8>>,
    component_name: &'static str,
) -> Option<&'a [u8]> {
    archive
        .iter()
        .find(|(name, _)| name.ends_with(component_name))
        .map(|(_, bytes)| bytes.as_slice())
}

/// Parse a manifest-style `major.minor` CAP version string.
pub(super) fn parse_version(value: &str) -> Option<CapFileVersion> {
    let (major, minor) = value.split_once('.')?;
    Some(CapFileVersion {
        major: major.parse().ok()?,
        minor: minor.parse().ok()?,
    })
}

/// Decode the package metadata stored in `Header.cap`.
pub(super) fn parse_header_component(
    bytes: &[u8],
) -> Result<(CapFileVersion, u8, u8, Aid, String)> {
    if bytes.len() < 14 {
        return Err(cap_format(
            "Header.cap is too short to contain package metadata",
        ));
    }
    if bytes[0] != 0x01 {
        return Err(cap_format("Header.cap has an unexpected component tag"));
    }
    let version = CapFileVersion {
        major: bytes[8],
        minor: bytes[7],
    };
    let package_minor = bytes[10];
    let package_major = bytes[11];
    let aid_len = bytes[12] as usize;
    let aid_start = 13;
    let aid_end = aid_start + aid_len;
    if bytes.len() < aid_end {
        return Err(cap_format("Header.cap package AID overruns the component"));
    }
    let package_aid = Aid::from_slice(&bytes[aid_start..aid_end])?;
    let package_name = if let Some(name_len) = bytes.get(aid_end) {
        let name_start = aid_end + 1;
        let name_end = name_start + usize::from(*name_len);
        if bytes.len() < name_end {
            return Err(cap_format("Header.cap package name overruns the component"));
        }
        String::from_utf8_lossy(&bytes[name_start..name_end]).to_string()
    } else {
        // CAPs produced by newer toolchains may omit the optional package-name trailer entirely and
        // rely on the manifest or directory structure for a human-readable name. Returning an empty
        // fallback here lets the caller keep parsing and prefer those richer sources later.
        String::new()
    };

    Ok((
        version,
        package_major,
        package_minor,
        package_aid,
        package_name,
    ))
}

/// Decode the applet list stored in `Applet.cap`.
pub(super) fn parse_applet_component(bytes: &[u8]) -> Result<Vec<CapApplet>> {
    if bytes.len() < 4 {
        return Err(cap_format(
            "Applet.cap is too short to contain applet count",
        ));
    }
    let mut offset = 3;
    let count = bytes[offset] as usize;
    offset += 1;
    let mut applets = Vec::with_capacity(count);
    for _ in 0..count {
        if offset >= bytes.len() {
            return Err(cap_format(
                "Applet.cap ended before all applets were decoded",
            ));
        }
        let aid_len = bytes[offset] as usize;
        offset += 1;
        let aid_end = offset + aid_len;
        if aid_end + 2 > bytes.len() {
            return Err(cap_format("Applet.cap AID data overruns the component"));
        }
        let aid = Aid::from_slice(&bytes[offset..aid_end])?;
        offset = aid_end;
        let install_method_offset = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;
        applets.push(CapApplet {
            aid,
            install_method_offset,
            name: None,
        });
    }
    Ok(applets)
}

/// Decode imported package metadata from `Import.cap`.
pub(super) fn parse_import_component(bytes: &[u8]) -> Result<Vec<ImportedPackage>> {
    if bytes.len() < 4 {
        return Err(cap_format(
            "Import.cap is too short to contain import count",
        ));
    }
    let mut offset = 3;
    let count = bytes[offset] as usize;
    offset += 1;
    let mut imports = Vec::with_capacity(count);
    for _ in 0..count {
        if offset + 3 > bytes.len() {
            return Err(cap_format(
                "Import.cap ended before all imports were decoded",
            ));
        }
        let minor = bytes[offset];
        let major = bytes[offset + 1];
        let aid_len = bytes[offset + 2] as usize;
        offset += 3;
        let aid_end = offset + aid_len;
        if aid_end > bytes.len() {
            return Err(cap_format("Import.cap AID data overruns the component"));
        }
        let aid = Aid::from_slice(&bytes[offset..aid_end])?;
        offset = aid_end;
        imports.push(ImportedPackage { aid, major, minor });
    }
    Ok(imports)
}

/// Build applet metadata from manifest entries when explicit applet descriptors are present.
pub(super) fn manifest_applets(
    manifest: &BTreeMap<String, String>,
) -> Option<Result<Vec<CapApplet>>> {
    let mut applets = Vec::new();
    let mut seen = false;
    for index in 1..=32 {
        let aid_key = format!("Java-Card-Applet-{index}-AID");
        let name_key = format!("Java-Card-Applet-{index}-Name");
        let Some(aid_value) = manifest.get(&aid_key) else {
            continue;
        };
        seen = true;
        let aid = match Aid::from_hex(aid_value) {
            Ok(aid) => aid,
            Err(error) => return Some(Err(error)),
        };
        applets.push(CapApplet {
            aid,
            install_method_offset: 0,
            name: manifest.get(&name_key).cloned(),
        });
    }
    seen.then_some(Ok(applets))
}
