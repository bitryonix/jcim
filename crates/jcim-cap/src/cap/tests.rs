#![allow(clippy::missing_docs_in_private_items)]

use std::io::Write;

use flate2::Compression;
use flate2::write::DeflateEncoder;

use jcim_core::model::{CardProfile, CardProfileId};

use super::{CapFileVersion, CapPackage};

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
                0x01, 0x00, 0x16, 0xDE, 0xCA, 0xFF, 0xED, 0x01, 0x02, 0x00, 0x01, 0x00, 0x09,
                0xA0, 0x00, 0x00, 0x00, 0x62, 0x03, 0x01, 0x0C, 0x01, 0x04, b'e', b'c', b'h',
                b'o',
            ],
            mode,
            false,
        ),
        (
            "com/example/echo/javacard/Applet.cap",
            vec![
                0x03, 0x00, 0x0E, 0x01, 0x0A, 0xA0, 0x00, 0x00, 0x00, 0x62, 0x03, 0x01, 0x0C,
                0x01, 0x01, 0x00, 0x10,
            ],
            mode,
            false,
        ),
        (
            "com/example/echo/javacard/Import.cap",
            vec![
                0x04, 0x00, 0x07, 0x01, 0x01, 0x00, 0x07, 0xA0, 0x00, 0x00, 0x00, 0x62, 0x00,
                0x01,
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
                0x01, 0x00, 0x12, 0xDE, 0xCA, 0xFF, 0xED, 0x01, 0x02, 0x04, 0x01, 0x00, 0x08,
                0xD0, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01,
            ],
            CompressionMode::Store,
            false,
        ),
        (
            "com/example/prefixed/javacard/Applet.cap",
            vec![
                0x03, 0x00, 0x0C, 0x01, 0x09, 0xD0, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01,
                0x00, 0x00, 0x10,
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
    let cap = CapPackage::from_bytes(sample_cap_with_prefixed_manifest_aids_and_data_descriptor())
        .unwrap();
    assert_eq!(cap.version, CapFileVersion { major: 2, minor: 1 });
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
