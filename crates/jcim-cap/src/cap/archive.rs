use std::collections::BTreeMap;
use std::io::Read;

use flate2::read::DeflateDecoder;

use jcim_core::error::Result;

use super::error::{cap_format, unexpected_end_of_zip_data, unsupported, zip_entry_out_of_bounds};

/// Metadata extracted from a ZIP central-directory entry while parsing a CAP archive.
#[derive(Clone, Debug)]
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
pub(super) fn parse_zip_entries(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let eocd = find_eocd(bytes)?;
    let entry_count = read_u16(bytes, eocd + 10)? as usize;
    let central_dir_offset = read_u32(bytes, eocd + 16)? as usize;
    let mut offset = central_dir_offset;
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        if read_u32(bytes, offset)? != 0x0201_4B50 {
            return Err(cap_format("invalid ZIP central directory header"));
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
        let name = String::from_utf8_lossy(slice(bytes, name_start, file_name_len)?).to_string();
        entries.push(ZipEntry {
            name,
            compression,
            flags,
            compressed_size,
            uncompressed_size,
            local_header_offset,
        });
        offset = name_start + file_name_len + extra_len + comment_len;
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
        return Err(unsupported("encrypted ZIP entries are not supported"));
    }
    if read_u32(bytes, entry.local_header_offset)? != 0x0403_4B50 {
        return Err(cap_format("invalid ZIP local file header"));
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
        method => Err(unsupported(format!(
            "ZIP compression method {method} is not supported"
        ))),
    }
}

/// Find the ZIP end-of-central-directory record.
fn find_eocd(bytes: &[u8]) -> Result<usize> {
    let min_len = 22;
    if bytes.len() < min_len {
        return Err(cap_format(
            "ZIP archive is too short to contain an end-of-central-directory record",
        ));
    }
    let start = bytes.len().saturating_sub(65_557);
    for offset in (start..=bytes.len() - min_len).rev() {
        if bytes[offset..offset + 4] == [0x50, 0x4B, 0x05, 0x06] {
            return Ok(offset);
        }
    }
    Err(cap_format("ZIP end-of-central-directory record not found"))
}

/// Read one little-endian `u16` from the archive.
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes([
        *bytes.get(offset).ok_or_else(unexpected_end_of_zip_data)?,
        *bytes
            .get(offset + 1)
            .ok_or_else(unexpected_end_of_zip_data)?,
    ]))
}

/// Read one little-endian `u32` from the archive.
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes([
        *bytes.get(offset).ok_or_else(unexpected_end_of_zip_data)?,
        *bytes
            .get(offset + 1)
            .ok_or_else(unexpected_end_of_zip_data)?,
        *bytes
            .get(offset + 2)
            .ok_or_else(unexpected_end_of_zip_data)?,
        *bytes
            .get(offset + 3)
            .ok_or_else(unexpected_end_of_zip_data)?,
    ]))
}

/// Borrow a byte range from the archive while enforcing bounds checks.
fn slice(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    bytes
        .get(offset..offset + len)
        .ok_or_else(zip_entry_out_of_bounds)
}
