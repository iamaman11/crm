#![forbid(unsafe_code)]

//! Authoritative data-quality governance owner domain.
//!
//! This pure owner crate contains no SQL, transport types, arbitrary evaluator
//! execution or direct cross-owner storage access. Authoritative Party values
//! remain owned by `crm.parties`; application composition supplies only the
//! exact governed source evidence required by the frozen evaluator vocabulary.

mod canonical_json;
mod canonicalization;
mod derived_identity;
mod finding_identity;

pub mod domain {
    use crate::canonicalization::semantic_json as serde_json;
    #[cfg(test)]
    use std::collections::BTreeSet;
    include!("domain.rs");
}

pub mod definition_persistence {
    use crate::canonicalization::persisted_state_json as serde_json;
    include!("definition_persistence.rs");
}

#[allow(clippy::too_many_arguments)]
pub mod evaluation_job;

pub mod evaluation_persistence {
    use crate::canonicalization::persisted_state_json as serde_json;
    include!("evaluation_persistence.rs");
}

pub mod completeness_result;
pub mod finding;
mod finding_stewardship;
pub mod remediation;
pub mod rule_outcome;

pub mod completeness_result_persistence {
    use crate::canonicalization::persisted_state_json as serde_json;
    include!("completeness_result_persistence.rs");
}

pub mod finding_persistence {
    use crate::canonicalization::persisted_state_json as serde_json;
    include!("finding_persistence.rs");
}

pub mod remediation_persistence {
    use crate::canonicalization::persisted_state_json as serde_json;
    include!("remediation_persistence.rs");
}

pub mod rule_outcome_persistence {
    use crate::canonicalization::persisted_state_json as serde_json;
    include!("rule_outcome_persistence.rs");
}

pub use completeness_result::*;
pub use completeness_result_persistence::*;
pub use definition_persistence::*;
pub use domain::*;
pub use evaluation_job::*;
pub use evaluation_persistence::*;
pub use finding::*;
pub use finding_identity::*;
pub use finding_persistence::*;
pub use remediation::*;
pub use remediation_persistence::*;
pub use rule_outcome::*;
pub use rule_outcome_persistence::*;

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
