use crate::{
    EvaluatedPartyKind, PartyCompletenessProfileVersion, PartyQualityInput, PartyRuleSetVersion,
};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyEvaluationJobStatus {
    Created,
    Staged,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyEvaluationJob {
    job_id: RecordId,
    party_id: RecordId,
    rule_set_version_id: String,
    profile_version_id: String,
    status: PartyEvaluationJobStatus,
    party_resource_version: Option<i64>,
    evaluated_rules: u32,
    failed_rules: u32,
    created_at: i64,
    updated_at: i64,
}

impl PartyEvaluationJob {
    pub fn create(
        job_id: RecordId,
        party_id: RecordId,
        rule_set: &PartyRuleSetVersion,
        profile: &PartyCompletenessProfileVersion,
        now: i64,
    ) -> Result<Self, SdkError> {
        if now < 0 || profile.rule_set_version_id() != rule_set.version_id() {
            return Err(invalid(
                "evaluation job references or timestamp are invalid",
            ));
        }
        Ok(Self {
            job_id,
            party_id,
            rule_set_version_id: rule_set.version_id().as_str().to_owned(),
            profile_version_id: profile.version_id().as_str().to_owned(),
            status: PartyEvaluationJobStatus::Created,
            party_resource_version: None,
            evaluated_rules: 0,
            failed_rules: 0,
            created_at: now,
            updated_at: now,
        })
    }

    pub(crate) fn restore(
        job_id: RecordId,
        party_id: RecordId,
        rule_set_version_id: String,
        profile_version_id: String,
        status: PartyEvaluationJobStatus,
        party_resource_version: Option<i64>,
        evaluated_rules: u32,
        failed_rules: u32,
        created_at: i64,
        updated_at: i64,
    ) -> Result<Self, SdkError> {
        let lifecycle_valid = match status {
            PartyEvaluationJobStatus::Created => {
                party_resource_version.is_none() && evaluated_rules == 0 && failed_rules == 0
            }
            PartyEvaluationJobStatus::Staged => {
                party_resource_version.is_some_and(|value| value > 0)
                    && failed_rules <= evaluated_rules
            }
            PartyEvaluationJobStatus::Completed => {
                party_resource_version.is_some_and(|value| value > 0)
                    && evaluated_rules > 0
                    && failed_rules <= evaluated_rules
            }
        };
        if rule_set_version_id.is_empty()
            || profile_version_id.is_empty()
            || created_at < 0
            || updated_at < created_at
            || !lifecycle_valid
        {
            return Err(invalid("persisted evaluation job invariants are invalid"));
        }
        Ok(Self {
            job_id,
            party_id,
            rule_set_version_id,
            profile_version_id,
            status,
            party_resource_version,
            evaluated_rules,
            failed_rules,
            created_at,
            updated_at,
        })
    }

    pub fn stage(
        &self,
        kind: EvaluatedPartyKind,
        display_name: impl Into<String>,
        party_resource_version: i64,
        now: i64,
    ) -> Result<(Self, PartyEvaluationInputSnapshot), SdkError> {
        if self.status != PartyEvaluationJobStatus::Created
            || party_resource_version <= 0
            || now < self.created_at
        {
            return Err(invalid("evaluation job cannot be staged"));
        }
        let input = PartyQualityInput::try_new(kind, display_name)?;
        let mut staged = self.clone();
        staged.status = PartyEvaluationJobStatus::Staged;
        staged.party_resource_version = Some(party_resource_version);
        staged.updated_at = now;
        Ok((
            staged,
            PartyEvaluationInputSnapshot {
                job_id: self.job_id.clone(),
                party_id: self.party_id.clone(),
                kind,
                display_name: input.display_name().to_owned(),
                party_resource_version,
                captured_at: now,
            },
        ))
    }

    pub fn record_materialized_outcomes(
        &self,
        evaluated_rules: u32,
        failed_rules: u32,
        now: i64,
    ) -> Result<Self, SdkError> {
        if self.status != PartyEvaluationJobStatus::Staged
            || self.party_resource_version.is_none()
            || self.evaluated_rules != 0
            || self.failed_rules != 0
            || evaluated_rules == 0
            || failed_rules > evaluated_rules
            || now < self.updated_at
        {
            return Err(invalid(
                "evaluation outcomes cannot be recorded for this job",
            ));
        }
        let mut materialized = self.clone();
        materialized.evaluated_rules = evaluated_rules;
        materialized.failed_rules = failed_rules;
        materialized.updated_at = now;
        Ok(materialized)
    }

    pub fn complete(
        &self,
        expected_evaluated_rules: u32,
        expected_failed_rules: u32,
        now: i64,
    ) -> Result<Self, SdkError> {
        if self.status != PartyEvaluationJobStatus::Staged
            || !self.outcomes_materialized()
            || self.evaluated_rules != expected_evaluated_rules
            || self.failed_rules != expected_failed_rules
            || expected_failed_rules > expected_evaluated_rules
            || now < self.updated_at
        {
            return Err(invalid(
                "evaluation job cannot cross the completion boundary",
            ));
        }
        let mut completed = self.clone();
        completed.status = PartyEvaluationJobStatus::Completed;
        completed.updated_at = now;
        Ok(completed)
    }

    pub fn job_id(&self) -> &RecordId {
        &self.job_id
    }

    pub fn party_id(&self) -> &RecordId {
        &self.party_id
    }

    pub fn rule_set_version_id(&self) -> &str {
        &self.rule_set_version_id
    }

    pub fn profile_version_id(&self) -> &str {
        &self.profile_version_id
    }

    pub const fn status(&self) -> PartyEvaluationJobStatus {
        self.status
    }

    pub const fn party_resource_version(&self) -> Option<i64> {
        self.party_resource_version
    }

    pub const fn evaluated_rules(&self) -> u32 {
        self.evaluated_rules
    }

    pub const fn failed_rules(&self) -> u32 {
        self.failed_rules
    }

    pub const fn outcomes_materialized(&self) -> bool {
        self.evaluated_rules > 0
    }

    pub const fn created_at(&self) -> i64 {
        self.created_at
    }

    pub const fn updated_at(&self) -> i64 {
        self.updated_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyEvaluationInputSnapshot {
    job_id: RecordId,
    party_id: RecordId,
    kind: EvaluatedPartyKind,
    display_name: String,
    party_resource_version: i64,
    captured_at: i64,
}

impl PartyEvaluationInputSnapshot {
    pub(crate) fn restore(
        job_id: RecordId,
        party_id: RecordId,
        kind: EvaluatedPartyKind,
        display_name: String,
        party_resource_version: i64,
        captured_at: i64,
    ) -> Result<Self, SdkError> {
        let input = PartyQualityInput::try_new(kind, display_name)?;
        if party_resource_version <= 0 || captured_at < 0 {
            return Err(invalid("persisted evaluation input invariants are invalid"));
        }
        Ok(Self {
            job_id,
            party_id,
            kind,
            display_name: input.display_name().to_owned(),
            party_resource_version,
            captured_at,
        })
    }

    pub fn job_id(&self) -> &RecordId {
        &self.job_id
    }

    pub fn party_id(&self) -> &RecordId {
        &self.party_id
    }

    pub const fn kind(&self) -> EvaluatedPartyKind {
        self.kind
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub const fn party_resource_version(&self) -> i64 {
        self.party_resource_version
    }

    pub const fn captured_at(&self) -> i64 {
        self.captured_at
    }
}

fn invalid(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_JOB_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Party evaluation job is invalid.",
    )
    .with_internal_reference(reference)
}
