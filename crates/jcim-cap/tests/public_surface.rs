//! Integration coverage for the `jcim-cap` public surface.

#![forbid(unsafe_code)]

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::DeflateEncoder;

use jcim_cap::cap::CapPackage;
use jcim_cap::export::{ExportRegistry, PackageExport};
use jcim_core::aid::Aid;
use jcim_core::model::{CardProfile, CardProfileId, JavaCardClassicVersion};

#[test]
fn cap_package_parses_from_public_bytes_and_path_inputs() {
    let cap_bytes = sample_cap();
    let parsed = CapPackage::from_bytes(cap_bytes.clone()).expect("parse cap bytes");
    assert_eq!(parsed.package_name, "com.example.echo");
    assert_eq!(parsed.package_aid.to_hex(), "A00000006203010C01");
    assert_eq!(parsed.applets.len(), 1);
    parsed
        .validate_for_profile(&CardProfile::builtin(CardProfileId::Classic222))
        .expect("validate profile");

    let root = temp_root("cap-file");
    let path = root.join("demo.cap");
    std::fs::create_dir_all(&root).expect("create temp root");
    std::fs::write(&path, cap_bytes).expect("write cap");
    let from_path = CapPackage::from_path(&path).expect("parse cap path");
    assert_eq!(from_path.package_name, parsed.package_name);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn export_registry_accepts_builtin_and_hint_file_exports() {
    let parsed = CapPackage::from_bytes(sample_cap()).expect("parse cap");

    let builtin = ExportRegistry::new_for_version(JavaCardClassicVersion::V2_2);
    builtin
        .validate_imports(&parsed.imports)
        .expect("builtin exports satisfy imports");

    let root = temp_root("hint-file");
    std::fs::create_dir_all(&root).expect("create temp root");
    let hint_path = root.join("hint.exp");
    std::fs::write(
        &hint_path,
        "name = example.pkg\naid = A0000001510002\nmajor = 1\nminor = 0\n",
    )
    .expect("write hint");

    let mut registry = ExportRegistry::default();
    registry
        .register_hint_file(&hint_path)
        .expect("register hint");
    registry.register(PackageExport {
        name: "manual.pkg".to_string(),
        aid: Aid::from_hex("A0000001510003").expect("aid"),
        major: 1,
        minor: 1,
        introduced_in: JavaCardClassicVersion::V3_0_1,
    });

    registry
        .validate_imports(&[
            jcim_cap::cap::ImportedPackage {
                aid: Aid::from_hex("A0000001510002").expect("aid"),
                major: 1,
                minor: 0,
            },
            jcim_cap::cap::ImportedPackage {
                aid: Aid::from_hex("A0000001510003").expect("aid"),
                major: 1,
                minor: 1,
            },
        ])
        .expect("validate hint and manual exports");

    let _ = std::fs::remove_dir_all(root);
}

fn sample_cap() -> Vec<u8> {
    build_zip(&[
        (
            "META-INF/MANIFEST.MF",
            b"Manifest-Version: 1.0\nJava-Card-CAP-File-Version: 2.2\nJava-Card-Package-AID: A00000006203010C01\nJava-Card-Package-Name: com.example.echo\nJava-Card-Applet-1-AID: A00000006203010C0101\nJava-Card-Applet-1-Name: EchoApplet\n"
                .to_vec(),
        ),
        (
            "com/example/echo/javacard/Header.cap",
            vec![
                0x01, 0x00, 0x16, 0xDE, 0xCA, 0xFF, 0xED, 0x01, 0x02, 0x00, 0x01, 0x00,
                0x09, 0xA0, 0x00, 0x00, 0x00, 0x62, 0x03, 0x01, 0x0C, 0x01, 0x04, b'e',
                b'c', b'h', b'o',
            ],
        ),
        (
            "com/example/echo/javacard/Applet.cap",
            vec![
                0x03, 0x00, 0x0E, 0x01, 0x0A, 0xA0, 0x00, 0x00, 0x00, 0x62, 0x03, 0x01,
                0x0C, 0x01, 0x01, 0x00, 0x10,
            ],
        ),
        (
            "com/example/echo/javacard/Import.cap",
            vec![
                0x04, 0x00, 0x07, 0x01, 0x01, 0x00, 0x07, 0xA0, 0x00, 0x00, 0x00, 0x62,
                0x00, 0x01,
            ],
        ),
    ])
}

fn build_zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();

    for (name, content) in entries {
        let local_header_offset = out.len() as u32;
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(content).expect("deflate content");
        let payload = encoder.finish().expect("finish deflate");
        let name_bytes = name.as_bytes();

        out.extend_from_slice(&0x0403_4B50_u32.to_le_bytes());
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&8_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&(content.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(&payload);

        central.extend_from_slice(&0x0201_4B50_u32.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&8_u16.to_le_bytes());
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

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-cap-public-{label}-{unique:x}"))
}
