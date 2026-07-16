use crate::{
    ComponentKey, PartyCompletenessProfileVersion, PartyEvaluationJob, PartyEvaluationJobStatus,
    PartyRuleOutcome, RuleKey, derived_identity::derived_id,
};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use std::collections::BTreeMap;

const COMPLETENESS_RESULT_ID_DOMAIN: &[u8] = b"crm.data-quality.completeness-result/v1";
const TOTAL_BASIS_POINTS: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyCompletenessComponentResult {
    component_key: ComponentKey,
    rule_key: RuleKey,
    rule_outcome_id: String,
    awarded_basis_points: u32,
}

impl PartyCompletenessComponentResult {
    pub fn component_key(&self) -> &ComponentKey {
        &self.component_key
    }

    pub fn rule_key(&self) -> &RuleKey {
        &self.rule_key
    }

    pub fn rule_outcome_id(&self) -> &str {
        &self.rule_outcome_id
    }

    pub const fn awarded_basis_points(&self) -> u32 {
        self.awarded_basis_points
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyCompletenessResult {
    result_id: String,
    job_id: RecordId,
    party_id: RecordId,
    party_resource_version: i64,
    profile_version_id: String,
    score_basis_points: u32,
    components: Vec<PartyCompletenessComponentResult>,
    computed_at: i64,
}

impl PartyCompletenessResult {
    pub fn compute(
        job: &PartyEvaluationJob,
        profile: &PartyCompletenessProfileVersion,
        outcomes: &[PartyRuleOutcome],
        computed_at: i64,
    ) -> Result<Self, SdkError> {
        if job.status() != PartyEvaluationJobStatus::Staged
            || job.profile_version_id() != profile.version_id().as_str()
            || job.rule_set_version_id() != profile.rule_set_version_id().as_str()
            || computed_at < job.updated_at()
        {
            return Err(invalid_result("profile or timestamp does not match the staged job"));
        }
        let party_resource_version = job
            .party_resource_version()
            .filter(|version| *version > 0)
            .ok_or_else(|| invalid_result("staged job is missing its Party version"))?;
        let mut by_rule = BTreeMap::new();
        for outcome in outcomes {
            if outcome.job_id() != job.job_id()
                || outcome.party_id() != job.party_id()
                || outcome.party_resource_version() != party_resource_version
                || outcome.rule_set_version_id() != job.rule_set_version_id()
                || outcome.evaluated_at() > computed_at
            {
                return Err(invalid_result("rule outcome does not match the staged job"));
            }
            if by_rule
                .insert(outcome.rule_key().as_str(), outcome)
                .is_some()
            {
                return Err(invalid_result("rule outcomes contain a duplicate rule key"));
            }
        }

        let mut score_basis_points = 0_u32;
        let mut components = Vec::with_capacity(profile.components().len());
        for component in profile.components() {
            let outcome = by_rule
                .get(component.rule_key().as_str())
                .ok_or_else(|| invalid_result("a completeness component outcome is missing"))?;
            let awarded_basis_points = if outcome.passed() {
                component.weight_basis_points()
            } else {
                0
            };
            score_basis_points = score_basis_points
                .checked_add(awarded_basis_points)
                .ok_or_else(|| invalid_result("completeness score overflowed"))?;
            components.push(PartyCompletenessComponentResult {
                component_key: component.component_key().clone(),
                rule_key: component.rule_key().clone(),
                rule_outcome_id: outcome.outcome_id().to_owned(),
                awarded_basis_points,
            });
        }
        if score_basis_points > TOTAL_BASIS_POINTS {
            return Err(invalid_result("completeness score exceeds 10,000 basis points"));
        }
        let result_id = result_id(job.job_id(), profile.version_id().as_str());
        Ok(Self {
            result_id,
            job_id: job.job_id().clone(),
            party_id: job.party_id().clone(),
            party_resource_version,
            profile_version_id: profile.version_id().as_str().to_owned(),
            score_basis_points,
            components,
            computed_at,
        })
    }

    pub(crate) fn restore(state: PartyCompletenessResultRestore) -> Result<Self, SdkError> {
        if state.party_resource_version <= 0
            || state.computed_at < 0
            || state.score_basis_points > TOTAL_BASIS_POINTS
            || state.components.is_empty()
        {
            return Err(invalid_result("persisted completeness result invariants are invalid"));
        }
        let expected_id = result_id(&state.job_id, &state.profile_version_id);
        if state.result_id != expected_id {
            return Err(invalid_result("persisted completeness result identity is invalid"));
        }
        let awarded_total = state.components.iter().try_fold(0_u32, |sum, component| {
            sum.checked_add(component.awarded_basis_points)
                .ok_or_else(|| invalid_result("persisted component awards overflowed"))
        })?;
        if awarded_total != state.score_basis_points
            || state.components.windows(2).any(|pair| {
                pair[0].component_key.as_str() >= pair[1].component_key.as_str()
            })
            || state.components.iter().any(|component| {
                component.rule_outcome_id.is_empty()
                    || component.awarded_basis_points > TOTAL_BASIS_POINTS
            })
        {
            return Err(invalid_result("persisted completeness lineage is invalid"));
        }
        Ok(Self {
            result_id: state.result_id,
            job_id: state.job_id,
            party_id: state.party_id,
            party_resource_version: state.party_resource_version,
            profile_version_id: state.profile_version_id,
            score_basis_points: state.score_basis_points,
            components: state.components,
            computed_at: state.computed_at,
        })
    }

    pub fn result_id(&self) -> &str {
        &self.result_id
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

    pub fn profile_version_id(&self) -> &str {
        &self.profile_version_id
    }

    pub const fn score_basis_points(&self) -> u32 {
        self.score_basis_points
    }

    pub fn components(&self) -> &[PartyCompletenessComponentResult] {
        &self.components
    }

    pub const fn computed_at(&self) -> i64 {
        self.computed_at
    }
}

pub(crate) struct PartyCompletenessResultRestore {
    pub result_id: String,
    pub job_id: RecordId,
    pub party_id: RecordId,
    pub party_resource_version: i64,
    pub profile_version_id: String,
    pub score_basis_points: u32,
    pub components: Vec<PartyCompletenessComponentResult>,
    pub computed_at: i64,
}

pub(crate) fn restore_component(
    component_key: ComponentKey,
    rule_key: RuleKey,
    rule_outcome_id: String,
    awarded_basis_points: u32,
) -> PartyCompletenessComponentResult {
    PartyCompletenessComponentResult {
        component_key,
        rule_key,
        rule_outcome_id,
        awarded_basis_points,
    }
}

fn result_id(job_id: &RecordId, profile_version_id: &str) -> String {
    derived_id(
        "dq-completeness-result",
        COMPLETENESS_RESULT_ID_DOMAIN,
        &[job_id.as_str().as_bytes(), profile_version_id.as_bytes()],
    )
}

fn invalid_result(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_COMPLETENESS_RESULT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Party completeness result is invalid.",
    )
    .with_internal_reference(reference)
}
