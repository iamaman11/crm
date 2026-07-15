#![forbid(unsafe_code)]

//! Authoritative data-quality governance owner domain.
//!
//! This pure owner crate contains no SQL, transport types, arbitrary evaluator
//! execution or direct cross-owner storage access. Authoritative Party values
//! remain owned by `crm.parties`; application composition supplies only the
//! exact governed source evidence required by the frozen evaluator vocabulary.

pub mod domain {
    #[cfg(test)]
    use std::collections::BTreeSet;

    include!("domain.rs");
}

pub use domain::*;

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
