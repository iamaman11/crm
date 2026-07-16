use crate::{PartyFinding, PartyFindingStatus, finding::PartyFindingRestore};
use crm_module_sdk::{ActorId, ErrorCategory, SdkError};

const MAX_WAIVER_REASON_BYTES: usize = 512;

impl PartyFinding {
    pub fn assign(
        &self,
        assigned_actor_id: Option<ActorId>,
        now: i64,
    ) -> Result<Self, SdkError> {
        self.ensure_stewardship_time(now)?;
        self.ensure_active_for_stewardship()?;
        self.rebuild(
            self.status(),
            assigned_actor_id,
            self.waiver_reason().map(str::to_owned),
            self.remediated_by_rule_outcome_id().map(str::to_owned),
            now,
        )
    }

    pub fn acknowledge(
        &self,
        expected_current_observation_id: &str,
        now: i64,
    ) -> Result<Self, SdkError> {
        self.ensure_exact_current_observation(expected_current_observation_id)?;
        self.ensure_stewardship_time(now)?;
        self.ensure_active_for_stewardship()?;
        if self.status() == PartyFindingStatus::Waived {
            return Err(stewardship_invalid(
                "a waived finding cannot be acknowledged without newer evidence",
            ));
        }
        self.rebuild(
            PartyFindingStatus::Acknowledged,
            self.assigned_actor_id().cloned(),
            None,
            None,
            now,
        )
    }

    pub fn waive(
        &self,
        expected_current_observation_id: &str,
        reason: impl Into<String>,
        now: i64,
    ) -> Result<Self, SdkError> {
        self.ensure_exact_current_observation(expected_current_observation_id)?;
        self.ensure_stewardship_time(now)?;
        self.ensure_active_for_stewardship()?;
        let reason = canonical_waiver_reason(reason.into())?;
        self.rebuild(
            PartyFindingStatus::Waived,
            self.assigned_actor_id().cloned(),
            Some(reason),
            None,
            now,
        )
    }

    fn ensure_exact_current_observation(&self, expected: &str) -> Result<(), SdkError> {
        if expected.is_empty() || expected != self.current_observation_id() {
            return Err(SdkError::new(
                "DATA_QUALITY_FINDING_OBSERVATION_CONFLICT",
                ErrorCategory::Conflict,
                false,
                "The Data Quality finding changed before the stewardship action could be applied.",
            ));
        }
        Ok(())
    }

    fn ensure_stewardship_time(&self, now: i64) -> Result<(), SdkError> {
        if now < self.updated_at() {
            return Err(stewardship_invalid(
                "stewardship timestamp precedes current finding state",
            ));
        }
        Ok(())
    }

    fn ensure_active_for_stewardship(&self) -> Result<(), SdkError> {
        if self.status() == PartyFindingStatus::Remediated {
            return Err(stewardship_invalid(
                "a remediated finding cannot receive active stewardship changes",
            ));
        }
        Ok(())
    }

    fn rebuild(
        &self,
        status: PartyFindingStatus,
        assigned_actor_id: Option<ActorId>,
        waiver_reason: Option<String>,
        remediated_by_rule_outcome_id: Option<String>,
        updated_at: i64,
    ) -> Result<Self, SdkError> {
        PartyFinding::restore(PartyFindingRestore {
            tenant_id: self.tenant_id().clone(),
            finding_id: self.finding_id().to_owned(),
            party_id: self.party_id().clone(),
            rule_set_version_id: self.rule_set_version_id().to_owned(),
            rule_key: self.rule_key().clone(),
            severity: self.severity(),
            status,
            current_observation_id: self.current_observation_id().to_owned(),
            evaluated_party_resource_version: self.evaluated_party_resource_version(),
            assigned_actor_id,
            waiver_reason,
            remediated_by_rule_outcome_id,
            created_at: self.created_at(),
            updated_at,
        })
    }
}

fn canonical_waiver_reason(value: String) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > MAX_WAIVER_REASON_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(stewardship_invalid("finding waiver reason is invalid"));
    }
    Ok(value)
}

fn stewardship_invalid(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_FINDING_STEWARDSHIP_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Data Quality finding stewardship action is invalid.",
    )
    .with_internal_reference(reference)
}
