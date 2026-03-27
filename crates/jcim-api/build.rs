//! Build script for compiling the JCIM 0.3 protobuf contract.

fn main() {
    let proto = "proto/jcim/v0_3/service.proto";
    println!("cargo:rerun-if-changed={proto}");
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[proto], &["proto"])
        .expect("compile jcim gRPC proto");
}
