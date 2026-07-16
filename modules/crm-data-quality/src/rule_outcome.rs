use crate::{
    PartyEvaluationJob, PartyEvaluationJobStatus, PartyRuleEvaluation, RuleKey,
    derived_identity::derived_id,
};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};

const RULE_OUTCOME_ID_DOMAIN: &[u8] = b"crm.data-quality.rule-outcome/v1";
const MAX_REASON_CODE_BYTES: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRuleOutcome {
    outcome_id: String,
    job_id: RecordId,
    party_id: RecordId,
    party_resource_version: i64,
    rule_set_version_id: String,
    rule_key: RuleKey,
    passed: bool,
    reason_code: String,
    evaluated_at: i64,
}

impl PartyRuleOutcome {
    pub fn evaluate(
        job: &PartyEvaluationJob,
        evaluation: &PartyRuleEvaluation,
        evaluated_at: i64,
    ) -> Result<Self, SdkError> {
        let party_resource_version = staged_party_version(job)?;
        if evaluation.rule_set_version_id().as_str() != job.rule_set_version_id()
            || evaluated_at < job.updated_at()
        {
            return Err(invalid_outcome("evaluation does not match the staged job"));
        }
        let reason_code = canonical_reason_code(evaluation.reason_code())?;
        if evaluation.passed() != (reason_code == "DATA_QUALITY_RULE_PASSED") {
            return Err(invalid_outcome("evaluation pass state and reason code differ"));
        }
        let rule_key = evaluation.rule_key().clone();
        let outcome_id = derived_id(
            "dq-rule-outcome",
            RULE_OUTCOME_ID_DOMAIN,
            &[
                job.job_id().as_str().as_bytes(),
                rule_key.as_str().as_bytes(),
                job.rule_set_version_id().as_bytes(),
            ],
        );
        Ok(Self {
            outcome_id,
            job_id: job.job_id().clone(),
            party_id: job.party_id().clone(),
            party_resource_version,
            rule_set_version_id: job.rule_set_version_id().to_owned(),
            rule_key,
            passed: evaluation.passed(),
            reason_code,
            evaluated_at,
        })
    }

    pub(crate) fn restore(state: PartyRuleOutcomeRestore) -> Result<Self, SdkError> {
        if state.party_resource_version <= 0 || state.evaluated_at < 0 {
            return Err(invalid_outcome("persisted outcome version or timestamp is invalid"));
        }
        let reason_code = canonical_reason_code(&state.reason_code)?;
        if state.passed != (reason_code == "DATA_QUALITY_RULE_PASSED") {
            return Err(invalid_outcome("persisted outcome pass state and reason code differ"));
        }
        let expected_id = derived_id(
            "dq-rule-outcome",
            RULE_OUTCOME_ID_DOMAIN,
            &[
                state.job_id.as_str().as_bytes(),
                state.rule_key.as_str().as_bytes(),
                state.rule_set_version_id.as_bytes(),
            ],
        );
        if state.outcome_id != expected_id {
            return Err(invalid_outcome("persisted outcome identity is invalid"));
        }
        Ok(Self {
            outcome_id: state.outcome_id,
            job_id: state.job_id,
            party_id: state.party_id,
            party_resource_version: state.party_resource_version,
            rule_set_version_id: state.rule_set_version_id,
            rule_key: state.rule_key,
            passed: state.passed,
            reason_code,
            evaluated_at: state.evaluated_at,
        })
    }

    pub fn outcome_id(&self) -> &str {
        &self.outcome_id
    }

    pub fn job_id(&self) -> &RecordId {
        &self.job_id
    }

    pub fn party_id(&self) -> &RecordId {
        &self.party_id
    }

    pub const fn party_resource_version(&self) -> i64 {
        self.party_resource_version
    }

    pub fn rule_set_version_id(&self) -> &str {
        &self.rule_set_version_id
    }

    pub fn rule_key(&self) -> &RuleKey {
        &self.rule_key
    }

    pub const fn passed(&self) -> bool {
        self.passed
    }

    pub fn reason_code(&self) -> &str {
        &self.reason_code
    }

    pub const fn evaluated_at(&self) -> i64 {
        self.evaluated_at
    }
}

pub(crate) struct PartyRuleOutcomeRestore {
    pub outcome_id: String,
    pub job_id: RecordId,
    pub party_id: RecordId,
    pub party_resource_version: i64,
    pub rule_set_version_id: String,
    pub rule_key: RuleKey,
    pub passed: bool,
    pub reason_code: String,
    pub evaluated_at: i64,
}

fn staged_party_version(job: &PartyEvaluationJob) -> Result<i64, SdkError> {
    if job.status() != PartyEvaluationJobStatus::Staged {
        return Err(invalid_outcome("only a staged job can produce outcomes"));
    }
    job.party_resource_version()
        .filter(|version| *version > 0)
        .ok_or_else(|| invalid_outcome("staged job is missing its Party version"))
}

fn canonical_reason_code(value: &str) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > MAX_REASON_CODE_BYTES
        || !value.is_ascii()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(invalid_outcome("rule outcome reason code is invalid"));
    }
    Ok(value.to_owned())
}

fn invalid_outcome(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_RULE_OUTCOME_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Party rule outcome is invalid.",
    )
    .with_internal_reference(reference)
}
