//! Canonical generated Connect services and messages; no domain or D1 logic.
// Use the standard generated package tree, including generator-owned lint
// attributes. The root import is inherited by its generated `use super::*`
// statements so the crm.core package also resolves Rust's fmt traits.
#[expect(
    clippy::wrong_self_convention,
    reason = "Buffa 0.9.1 generates from_role/from_party_id getters from Protobuf fields; these are not constructors"
)]
pub mod proto {
    use ::core::fmt;
    include!(concat!(env!("OUT_DIR"), "/_connectrpc.rs"));
}
