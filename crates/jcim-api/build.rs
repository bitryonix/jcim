//! Build script for compiling the JCIM 0.3 protobuf contract.

fn main() {
    let proto = "proto/jcim/v0_3/service.proto";
    let descriptor_path = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"))
        .join("jcim-v0_3-descriptor.bin");
    println!("cargo:rerun-if-changed={proto}");
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(&[proto], &["proto"])
        .expect("compile jcim gRPC proto");
}
