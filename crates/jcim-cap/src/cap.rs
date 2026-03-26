//! Pure-Rust CAP/JAR parsing utilities.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use flate2::read::DeflateDecoder;
use serde::{Deserialize, Serialize};

use jcim_core::aid::Aid;
use jcim_core::error::{JcimError, Result};
use jcim_core::model::CardProfile;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
/// CAP file version extracted from the manifest or `Header.cap`.
pub struct CapFileVersion {
    /// Major CAP version component.
    pub major: u8,
    /// Minor CAP version component.
    pub minor: u8,
}

impl CapFileVersion {
    /// Return whether the parsed CAP version is supported by the current parser.
    pub fn is_supported(&self) -> bool {
        (self.major, self.minor) == (2, 1) || (self.major, self.minor) == (2, 2)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
/// Package import entry from `Import.cap`.
pub struct ImportedPackage {
    /// Imported package AID.
    pub aid: Aid,
    /// Imported major version.
    pub major: u8,
    /// Imported minor version.
    pub minor: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
/// Applet metadata discovered in a CAP package.
pub struct CapApplet {
    /// Applet AID declared by the CAP package.
    pub aid: Aid,
    /// Offset of the install method in the Method component.
    pub install_method_offset: u16,
    /// Optional human-readable applet name sourced from the manifest.
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
/// Parsed CAP package and selected metadata components.
pub struct CapPackage {
    /// CAP file format version.
    pub version: CapFileVersion,
    /// Package AID declared by `Header.cap`.
    pub package_aid: Aid,
    /// Package name sourced from the manifest or inferred from paths.
    pub package_name: String,
    /// Package major version.
    pub package_major: u8,
    /// Package minor version.
    pub package_minor: u8,
    /// Imported package dependencies.
    pub imports: Vec<ImportedPackage>,
    /// Applets declared by the CAP package.
    pub applets: Vec<CapApplet>,
    /// Parsed manifest key-value pairs.
    pub manifest: BTreeMap<String, String>,
    /// Original archive bytes for persistence and hashing.
    pub raw_bytes: Vec<u8>,
}

impl CapPackage {
    /// Read and parse a CAP archive from disk.
    pub fn from_path(path: &Path) -> Result<Self> {
        Self::from_bytes(std::fs::read(path)?)
    }

    /// Parse a CAP archive from in-memory bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
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
            .map(|value| Aid::from_hex(value))
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

        Ok(Self {
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

    /// Validate the CAP against a selected built-in card profile.
    pub fn validate_for_profile(&self, profile: &CardProfile) -> Result<()> {
        if self.version.major != 2 || !profile.supports_cap_minor(self.version.minor) {
            return Err(JcimError::Unsupported(format!(
                "CAP version {}.{} is not compatible with Classic profile {}",
                self.version.major,
                self.version.minor,
                profile.version.display_name()
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
/// Metadata extracted from a ZIP central-directory entry while parsing a CAP archive.
struct ZipEntry {
    /// Entry name as reported by the archive.
    name: String,
    /// ZIP compression method.
    compression: u16,
    /// ZIP general-purpose bit flags.
    flags: u16,
    /// Stored compressed size in bytes.
    compressed_size: usize,
    /// Expected uncompressed size in bytes.
    uncompressed_size: usize,
    /// Offset of the corresponding local-file header.
    local_header_offset: usize,
}

/// Parse every ZIP entry in the CAP archive into a map of filename to decoded bytes.
fn parse_zip_entries(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let eocd = find_eocd(bytes)?;
    let entry_count = read_u16(bytes, eocd + 10)? as usize;
    let central_dir_offset = read_u32(bytes, eocd + 16)? as usize;
    let mut offset = central_dir_offset;
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        if read_u32(bytes, offset)? != 0x0201_4B50 {
            return Err(JcimError::CapFormat(
                "invalid ZIP central directory header".to_string(),
            ));
        }
        let flags = read_u16(bytes, offset + 8)?;
        let compression = read_u16(bytes, offset + 10)?;
        let compressed_size = read_u32(bytes, offset + 20)? as usize;
        let uncompressed_size = read_u32(bytes, offset + 24)? as usize;
        let file_name_len = read_u16(bytes, offset + 28)? as usize;
        let extra_len = read_u16(bytes, offset + 30)? as usize;
        let comment_len = read_u16(bytes, offset + 32)? as usize;
        let local_header_offset = read_u32(bytes, offset + 42)? as usize;
        let name_start = offset + 46;
        let name_end = name_start + file_name_len;
        let name = String::from_utf8_lossy(slice(bytes, name_start, file_name_len)?).to_string();
        entries.push(ZipEntry {
            name,
            compression,
            flags,
            compressed_size,
            uncompressed_size,
            local_header_offset,
        });
        offset = name_end + extra_len + comment_len;
    }

    let mut archive = BTreeMap::new();
    for entry in entries {
        let content = extract_entry(bytes, &entry)?;
        archive.insert(entry.name, content);
    }
    Ok(archive)
}

/// Extract one ZIP entry payload, inflating it when the archive uses deflate compression.
fn extract_entry(bytes: &[u8], entry: &ZipEntry) -> Result<Vec<u8>> {
    if entry.flags & 0x0001 != 0 {
        return Err(JcimError::Unsupported(
            "encrypted ZIP entries are not supported".to_string(),
        ));
    }
    if read_u32(bytes, entry.local_header_offset)? != 0x0403_4B50 {
        return Err(JcimError::CapFormat(
            "invalid ZIP local file header".to_string(),
        ));
    }
    let file_name_len = read_u16(bytes, entry.local_header_offset + 26)? as usize;
    let extra_len = read_u16(bytes, entry.local_header_offset + 28)? as usize;
    let data_offset = entry.local_header_offset + 30 + file_name_len + extra_len;
    // Some Oracle-produced CAP archives set the data-descriptor flag and leave the local-header
    // sizes empty, but the central directory still carries authoritative offsets and lengths. We
    // already parsed those central-directory fields, so slicing the payload with them keeps the
    // parser compatible with those archives without needing to interpret the trailing descriptor.
    let compressed = slice(bytes, data_offset, entry.compressed_size)?;
    match entry.compression {
        0 => Ok(compressed.to_vec()),
        8 => {
            let mut decoder = DeflateDecoder::new(compressed);
            let mut output = Vec::with_capacity(entry.uncompressed_size);
            decoder.read_to_end(&mut output)?;
            Ok(output)
        }
        method => Err(JcimError::Unsupported(format!(
            "ZIP compression method {method} is not supported"
        ))),
    }
}

/// Find the ZIP end-of-central-directory record.
fn find_eocd(bytes: &[u8]) -> Result<usize> {
    let min_len = 22;
    if bytes.len() < min_len {
        return Err(JcimError::CapFormat(
            "ZIP archive is too short to contain an end-of-central-directory record".to_string(),
        ));
    }
    let start = bytes.len().saturating_sub(65_557);
    for offset in (start..=bytes.len() - min_len).rev() {
        if bytes[offset..offset + 4] == [0x50, 0x4B, 0x05, 0x06] {
            return Ok(offset);
        }
    }
    Err(JcimError::CapFormat(
        "ZIP end-of-central-directory record not found".to_string(),
    ))
}

/// Read `META-INF/MANIFEST.MF` into a key-value map when present.
fn read_manifest(archive: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, String> {
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
fn infer_package_name(archive: &BTreeMap<String, Vec<u8>>) -> Option<String> {
    archive.keys().find_map(|name| {
        name.strip_suffix("/Header.cap")
            .map(|prefix| prefix.replace("/javacard", "").replace('/', "."))
    })
}

/// Read a required CAP component from the archive.
fn read_component<'a>(
    archive: &'a BTreeMap<String, Vec<u8>>,
    component_name: &'static str,
) -> Result<&'a [u8]> {
    read_optional_component(archive, component_name)
        .ok_or(JcimError::MissingCapComponent(component_name))
}

/// Read an optional CAP component from the archive when it exists.
fn read_optional_component<'a>(
    archive: &'a BTreeMap<String, Vec<u8>>,
    component_name: &'static str,
) -> Option<&'a [u8]> {
    archive
        .iter()
        .find(|(name, _)| name.ends_with(component_name))
        .map(|(_, bytes)| bytes.as_slice())
}

/// Parse a manifest-style `major.minor` CAP version string.
fn parse_version(value: &str) -> Option<CapFileVersion> {
    let (major, minor) = value.split_once('.')?;
    Some(CapFileVersion {
        major: major.parse().ok()?,
        minor: minor.parse().ok()?,
    })
}

/// Decode the package metadata stored in `Header.cap`.
fn parse_header_component(bytes: &[u8]) -> Result<(CapFileVersion, u8, u8, Aid, String)> {
    if bytes.len() < 14 {
        return Err(JcimError::CapFormat(
            "Header.cap is too short to contain package metadata".to_string(),
        ));
    }
    if bytes[0] != 0x01 {
        return Err(JcimError::CapFormat(
            "Header.cap has an unexpected component tag".to_string(),
        ));
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
        return Err(JcimError::CapFormat(
            "Header.cap package AID overruns the component".to_string(),
        ));
    }
    let package_aid = Aid::from_slice(&bytes[aid_start..aid_end])?;
    let package_name = if let Some(name_len) = bytes.get(aid_end) {
        let name_start = aid_end + 1;
        let name_end = name_start + usize::from(*name_len);
        if bytes.len() < name_end {
            return Err(JcimError::CapFormat(
                "Header.cap package name overruns the component".to_string(),
            ));
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
fn parse_applet_component(bytes: &[u8]) -> Result<Vec<CapApplet>> {
    if bytes.len() < 4 {
        return Err(JcimError::CapFormat(
            "Applet.cap is too short to contain applet count".to_string(),
        ));
    }
    let mut offset = 3;
    let count = bytes[offset] as usize;
    offset += 1;
    let mut applets = Vec::with_capacity(count);
    for _ in 0..count {
        if offset >= bytes.len() {
            return Err(JcimError::CapFormat(
                "Applet.cap ended before all applets were decoded".to_string(),
            ));
        }
        let aid_len = bytes[offset] as usize;
        offset += 1;
        let aid_end = offset + aid_len;
        if aid_end + 2 > bytes.len() {
            return Err(JcimError::CapFormat(
                "Applet.cap AID data overruns the component".to_string(),
            ));
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
fn parse_import_component(bytes: &[u8]) -> Result<Vec<ImportedPackage>> {
    if bytes.len() < 4 {
        return Err(JcimError::CapFormat(
            "Import.cap is too short to contain import count".to_string(),
        ));
    }
    let mut offset = 3;
    let count = bytes[offset] as usize;
    offset += 1;
    let mut imports = Vec::with_capacity(count);
    for _ in 0..count {
        if offset + 3 > bytes.len() {
            return Err(JcimError::CapFormat(
                "Import.cap ended before all imports were decoded".to_string(),
            ));
        }
        let minor = bytes[offset];
        let major = bytes[offset + 1];
        let aid_len = bytes[offset + 2] as usize;
        offset += 3;
        let aid_end = offset + aid_len;
        if aid_end > bytes.len() {
            return Err(JcimError::CapFormat(
                "Import.cap AID data overruns the component".to_string(),
            ));
        }
        let aid = Aid::from_slice(&bytes[offset..aid_end])?;
        offset = aid_end;
        imports.push(ImportedPackage { aid, major, minor });
    }
    Ok(imports)
}

/// Build applet metadata from manifest entries when explicit applet descriptors are present.
fn manifest_applets(manifest: &BTreeMap<String, String>) -> Option<Result<Vec<CapApplet>>> {
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

/// Read one little-endian `u16` from the archive.
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes([
        *bytes
            .get(offset)
            .ok_or_else(|| JcimError::CapFormat("unexpected end of ZIP data".to_string()))?,
        *bytes
            .get(offset + 1)
            .ok_or_else(|| JcimError::CapFormat("unexpected end of ZIP data".to_string()))?,
    ]))
}

/// Read one little-endian `u32` from the archive.
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes([
        *bytes
            .get(offset)
            .ok_or_else(|| JcimError::CapFormat("unexpected end of ZIP data".to_string()))?,
        *bytes
            .get(offset + 1)
            .ok_or_else(|| JcimError::CapFormat("unexpected end of ZIP data".to_string()))?,
        *bytes
            .get(offset + 2)
            .ok_or_else(|| JcimError::CapFormat("unexpected end of ZIP data".to_string()))?,
        *bytes
            .get(offset + 3)
            .ok_or_else(|| JcimError::CapFormat("unexpected end of ZIP data".to_string()))?,
    ]))
}

/// Borrow a byte range from the archive while enforcing bounds checks.
fn slice(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    bytes.get(offset..offset + len).ok_or_else(|| {
        JcimError::CapFormat("ZIP entry extends beyond the archive boundary".to_string())
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::DeflateEncoder;

    use super::CapPackage;
    use jcim_core::model::{CardProfile, CardProfileId};

    #[derive(Clone, Copy)]
    enum CompressionMode {
        Store,
        Deflate,
    }

    fn sample_cap(version: &str, mode: CompressionMode) -> Vec<u8> {
        build_zip(&[
            (
                "META-INF/MANIFEST.MF",
                format!(
                    "Manifest-Version: 1.0\nJava-Card-CAP-File-Version: {version}\nJava-Card-Package-AID: A00000006203010C01\nJava-Card-Package-Name: com.example.echo\nJava-Card-Applet-1-AID: A00000006203010C0101\nJava-Card-Applet-1-Name: EchoApplet\n"
                )
                .into_bytes(),
                mode,
                false,
            ),
            (
                "com/example/echo/javacard/Header.cap",
                vec![
                    0x01, 0x00, 0x16, 0xDE, 0xCA, 0xFF, 0xED, 0x01, 0x02, 0x00, 0x01, 0x00,
                    0x09, 0xA0, 0x00, 0x00, 0x00, 0x62, 0x03, 0x01, 0x0C, 0x01, 0x04, b'e',
                    b'c', b'h', b'o',
                ],
                mode,
                false,
            ),
            (
                "com/example/echo/javacard/Applet.cap",
                vec![
                    0x03, 0x00, 0x0E, 0x01, 0x0A, 0xA0, 0x00, 0x00, 0x00, 0x62, 0x03, 0x01,
                    0x0C, 0x01, 0x01, 0x00, 0x10,
                ],
                mode,
                false,
            ),
            (
                "com/example/echo/javacard/Import.cap",
                vec![
                    0x04, 0x00, 0x07, 0x01, 0x01, 0x00, 0x07, 0xA0, 0x00, 0x00, 0x00, 0x62,
                    0x00, 0x01,
                ],
                mode,
                false,
            ),
        ])
    }

    fn sample_cap_with_prefixed_manifest_aids_and_data_descriptor() -> Vec<u8> {
        build_zip(&[
            (
                "META-INF/MANIFEST.MF",
                b"Manifest-Version: 1.0\nJava-Card-CAP-File-Version: 2.1\nJava-Card-Package-AID: 0xD0:0x00:0x00:0x00:0x01:0x01:0x01:0x01\nJava-Card-Package-Name: com.example.prefixed\nJava-Card-Applet-1-AID: 0xD0:0x00:0x00:0x00:0x01:0x01:0x01:0x01:0x00\nJava-Card-Applet-1-Name: ExampleApplet\n"
                    .to_vec(),
                CompressionMode::Store,
                true,
            ),
            (
                "com/example/prefixed/javacard/Header.cap",
                vec![
                    0x01, 0x00, 0x12, 0xDE, 0xCA, 0xFF, 0xED, 0x01, 0x02, 0x04, 0x01, 0x00,
                    0x08, 0xD0, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01,
                ],
                CompressionMode::Store,
                false,
            ),
            (
                "com/example/prefixed/javacard/Applet.cap",
                vec![
                    0x03, 0x00, 0x0C, 0x01, 0x09, 0xD0, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01,
                    0x01, 0x00, 0x00, 0x10,
                ],
                CompressionMode::Store,
                false,
            ),
        ])
    }

    fn build_zip(entries: &[(&str, Vec<u8>, CompressionMode, bool)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, content, compression_mode, use_data_descriptor) in entries {
            let local_header_offset = out.len() as u32;
            let (compression, payload) = match compression_mode {
                CompressionMode::Store => (0_u16, content.clone()),
                CompressionMode::Deflate => {
                    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
                    encoder.write_all(content).unwrap();
                    (8_u16, encoder.finish().unwrap())
                }
            };
            let name_bytes = name.as_bytes();
            let flags = if *use_data_descriptor {
                0x0008_u16
            } else {
                0_u16
            };
            out.extend_from_slice(&0x0403_4B50_u32.to_le_bytes());
            out.extend_from_slice(&20_u16.to_le_bytes());
            out.extend_from_slice(&flags.to_le_bytes());
            out.extend_from_slice(&compression.to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes());
            out.extend_from_slice(&0_u32.to_le_bytes());
            out.extend_from_slice(
                &if *use_data_descriptor {
                    0_u32
                } else {
                    payload.len() as u32
                }
                .to_le_bytes(),
            );
            out.extend_from_slice(
                &if *use_data_descriptor {
                    0_u32
                } else {
                    content.len() as u32
                }
                .to_le_bytes(),
            );
            out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes());
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(&payload);
            if *use_data_descriptor {
                // The CAP parser should rely on the central directory sizes, so tests emit a real
                // trailing descriptor to cover the Oracle-style archive layout seen in the wild.
                out.extend_from_slice(&0x0807_4B50_u32.to_le_bytes());
                out.extend_from_slice(&0_u32.to_le_bytes());
                out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                out.extend_from_slice(&(content.len() as u32).to_le_bytes());
            }

            central.extend_from_slice(&0x0201_4B50_u32.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&flags.to_le_bytes());
            central.extend_from_slice(&compression.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u32.to_le_bytes());
            central.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            central.extend_from_slice(&(content.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u32.to_le_bytes());
            central.extend_from_slice(&local_header_offset.to_le_bytes());
            central.extend_from_slice(name_bytes);
        }

        let central_offset = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4B50_u32.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&central_offset.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out
    }

    #[test]
    fn parses_minimal_cap_archive() {
        let cap = CapPackage::from_bytes(sample_cap("2.2", CompressionMode::Store)).unwrap();
        assert_eq!(cap.package_name, "com.example.echo");
        assert_eq!(cap.applets.len(), 1);
        assert_eq!(cap.imports.len(), 1);
    }

    #[test]
    fn parses_deflated_cap_archive() {
        let cap = CapPackage::from_bytes(sample_cap("2.2", CompressionMode::Deflate)).unwrap();
        assert_eq!(cap.package_aid.to_string(), "A00000006203010C01");
        assert_eq!(cap.applets.len(), 1);
    }

    #[test]
    fn parses_manifest_data_descriptor_and_prefixed_aids() {
        let cap =
            CapPackage::from_bytes(sample_cap_with_prefixed_manifest_aids_and_data_descriptor())
                .unwrap();
        assert_eq!(cap.version, super::CapFileVersion { major: 2, minor: 1 });
        assert_eq!(cap.package_aid.to_string(), "D000000001010101");
        assert_eq!(cap.package_name, "com.example.prefixed");
        assert_eq!(cap.applets.len(), 1);
        assert_eq!(cap.applets[0].aid.to_string(), "D00000000101010100");
        assert_eq!(cap.applets[0].name.as_deref(), Some("ExampleApplet"));
    }

    #[test]
    fn rejects_extended_cap_versions() {
        let error = CapPackage::from_bytes(sample_cap("2.3", CompressionMode::Store)).unwrap_err();
        assert!(format!("{error}").contains("unsupported CAP file version 2.3"));
    }

    #[test]
    fn validates_against_profile() {
        let cap = CapPackage::from_bytes(sample_cap("2.2", CompressionMode::Store)).unwrap();
        let older = CardProfile::builtin(CardProfileId::Classic211);
        assert!(cap.validate_for_profile(&older).is_err());
        let newer = CardProfile::builtin(CardProfileId::Classic305);
        assert!(cap.validate_for_profile(&newer).is_ok());
    }
}
