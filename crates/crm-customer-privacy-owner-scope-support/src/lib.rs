#![forbid(unsafe_code)]

//! Proven shared protocol support for first-party privacy-scope owner adapters.
//!
//! This crate contains only behavior demonstrated to be identical by the
//! independently accepted Parties and Consents implementations: exact query
//! request integrity, common lineage validation, canonical Party topology proof
//! and deterministic length-framed digests. Owner reads, cursor semantics,
//! record rehydration, evidence classification and response construction remain
//! in owner-specific adapters.

mod digest;
mod postgres;
mod request;

#[cfg(test)]
mod tests;

pub use digest::framed_digest;
pub use postgres::{CanonicalPartyClaimError, prove_canonical_party_claim};
pub use request::{
    CommonLineageError, QueryRequestContractError, ValidatedCommonLineage,
    validate_common_lineage, validate_query_request_contract,
};
