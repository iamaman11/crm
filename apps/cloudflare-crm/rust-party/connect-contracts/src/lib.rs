//! Canonical generated Connect services and messages; no domain or D1 logic.
// The generated crate is checked by the same strict workspace Clippy gate.
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/_connectrpc.rs"));
}
