#![forbid(unsafe_code)]

//! Proven shared protocol support for first-party privacy-scope owner adapters.
//!
//! This crate contains only behavior demonstrated to be identical by the
//! independently accepted Parties and Consents implementations: exact query
//! request integrity, common lineage validation, canonical Party topology proof,
//! deterministic length-framed digests and fail-closed owner-action command
//! binding. Owner reads, cursor semantics, record rehydration, evidence
//! classification and destructive/minimizing transformations remain in
//! owner-specific adapters.

mod digest;
mod owner_action;
mod postgres;
mod request;

#[cfg(test)]
mod tests;

pub use digest::{append_frame, framed_digest};
pub use owner_action::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, owner_action_definition,
    owner_action_input_payload, unsupported_owner_action,
};
pub use postgres::{CanonicalPartyClaimError, prove_canonical_party_claim};
pub use request::{
    CommonLineageError, QueryRequestContractError, ValidatedCommonLineage, validate_common_lineage,
    validate_query_request_contract,
};
