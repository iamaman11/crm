#![forbid(unsafe_code)]

//! Authoritative data-quality governance owner domain.
//!
//! This pure owner crate contains no SQL, transport types, arbitrary evaluator
//! execution or direct cross-owner storage access. Authoritative Party values
//! remain owned by `crm.parties`; application composition supplies only the
//! exact governed source evidence required by the frozen evaluator vocabulary.

mod canonical_json;
mod canonicalization;

pub mod domain {
    // `domain.rs` has one JSON serialization site: immutable semantic identity.
    // Bind it explicitly to the normative platform canonicalization profile.
    use crate::canonicalization::semantic_json as serde_json;
    #[cfg(test)]
    use std::collections::BTreeSet;

    include!("domain.rs");
}

pub mod definition_persistence {
    // Persisted immutable definitions store and verify the exact profile beside
    // the content-derived version identity before strict domain rehydration.
    use crate::canonicalization::persisted_state_json as serde_json;

    include!("definition_persistence.rs");
}

pub mod evaluation_job;

pub mod evaluation_persistence {
    // Mutable job state and immutable staged input both retain the exact
    // canonicalization profile and require canonical byte-for-byte rehydration.
    use crate::canonicalization::persisted_state_json as serde_json;

    include!("evaluation_persistence.rs");
}

pub use definition_persistence::*;
pub use domain::*;
pub use evaluation_job::*;
pub use evaluation_persistence::*;

pub const CANONICALIZATION_PROFILE_ID: &str = canonicalization::PROFILE_ID;
pub const MODULE_ID: &str = "crm.data-quality";
pub const PARTY_RULE_SET_VERSION_RECORD_TYPE: &str = "data_quality.party_rule_set_version";
pub const PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE: &str =
    "data_quality.party_completeness_profile_version";
pub const PARTY_EVALUATION_JOB_RECORD_TYPE: &str = "data_quality.party_evaluation_job";
pub const PARTY_EVALUATION_INPUT_RECORD_TYPE: &str = "data_quality.party_evaluation_input";
pub const RULE_OUTCOME_RECORD_TYPE: &str = "data_quality.rule_outcome";
pub const FINDING_RECORD_TYPE: &str = "data_quality.finding";
pub const FINDING_OBSERVATION_RECORD_TYPE: &str = "data_quality.finding_observation";
pub const PARTY_COMPLETENESS_RESULT_RECORD_TYPE: &str = "data_quality.party_completeness_result";
pub const REMEDIATION_ATTEMPT_RECORD_TYPE: &str = "data_quality.remediation_attempt";
