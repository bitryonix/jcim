//! gRPC contract for the JCIM 0.3 local service.
//!
//! Descriptor compatibility for this surface is review-blocking through
//! `crates/jcim-api/tests/descriptor_contract.rs`.

#![forbid(unsafe_code)]

/// Encoded file-descriptor set for the maintained `jcim.v0_3` protobuf contract.
pub const JCIM_V0_3_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/jcim-v0_3-descriptor.bin"));

/// Generated protobuf and gRPC service definitions.
#[allow(missing_docs)]
#[allow(clippy::missing_docs_in_private_items)]
pub mod v0_3 {
    tonic::include_proto!("jcim.v0_3");
}
