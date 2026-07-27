use crate::contract::{
    MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED, MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS,
    MAX_PRIVACY_COMPLETENESS_RESULTS_SCANNED, MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    MAX_PRIVACY_EVALUATION_INPUTS_SCANNED, MAX_PRIVACY_EVALUATION_JOBS_SCANNED,
    MAX_PRIVACY_FINDINGS_SCANNED, MAX_PRIVACY_FINDING_OBSERVATIONS_SCANNED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAX_PRIVACY_REMEDIATION_ATTEMPTS_SCANNED,
    MAX_PRIVACY_RULE_OUTCOMES_SCANNED, PRIVACY_OWNER_SCAN_BATCH_SIZE, validate_definition,
};
use crate::errors::{
    association_state_invalid, canonical_resolution_unavailable, database_unavailable,
    lineage_invalid, map_canonical_party_claim_error, scan_limit_exceeded, stored_state_invalid,
};
use crate::request::{
    CursorState, ResourceFamily, ValidatedRequest, validate_request_contract, validate_wire_request,
};
use crate::response::{VerifiedDataQualityResource, build_response, typed_output};
use crm_capability_plan_support::PersistedPayloadContract;
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::{BoundReadTransaction, PostgresDataStore};
use crm_customer_privacy_owner_scope_support::prove_canonical_party_claim;
use crm_data_quality::{
    FINDING_OBSERVATION_RECORD_TYPE, FINDING_RECORD_TYPE, PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
    PARTY_COMPLETENESS_RESULT_RECORD_TYPE, PARTY_EVALUATION_INPUT_RECORD_TYPE,
    PARTY_EVALUATION_JOB_RECORD_TYPE, PARTY_RULE_SET_VERSION_RECORD_TYPE, REMEDIATION_ATTEMPT_RECORD_TYPE,
    RULE_OUTCOME_RECORD_TYPE, PartyCompletenessProfileVersion, PartyCompletenessResult,
    PartyDisplayNameRemediationAttempt, PartyEvaluationInputSnapshot, PartyEvaluationJob,
    PartyEvaluationJobStatus, PartyFinding, PartyFindingObservation, PartyRuleOutcome,
    PartyRuleSetVersion, decode_finding_observation_state, decode_finding_state,
    decode_party_completeness_result_state, decode_party_evaluation_input_state,
    decode_remediation_attempt_state, decode_rule_outcome_state,
};
use crm_data_quality_capability_adapter::{
    MODULE_ID, completeness_profile_rule_set_version_id_from_snapshot,
    party_completeness_profile_from_immutable_snapshot, party_completeness_profile_persisted_contract,
    party_completeness_result_persisted_contract, party_evaluation_input_persisted_contract,
    party_evaluation_job_from_snapshot, party_evaluation_job_persisted_contract,
    party_finding_observation_persisted_contract, party_finding_persisted_contract,
    party_rule_outcome_persisted_contract, party_rule_set_from_snapshot,
    party_rule_set_persisted_contract, remediation_attempt_persisted_contract,
};
use crm_identity_resolution::PartyReference;
use crm_identity_resolution_topology_composition::prove_canonical_party_in_transaction;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId,
    TypedPayload,
};
use crm_query_runtime::{QueryExecutionResult, QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::Row;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct DataQualityPrivacyScopeQueryAdapter {
    store: PostgresDataStore,
}

impl std::fmt::Debug for DataQualityPrivacyScopeQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DataQualityPrivacyScopeQueryAdapter")
            .field("store", &"PostgresDataStore")
            .finish()
    }
}

impl DataQualityPrivacyScopeQueryAdapter {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    async fn execute_query(
        &self,
        definition: &CapabilityDefinition,
        request: QueryRequest,
    ) -> Result<QueryExecutionResult, SdkError> {
        validate_definition(definition)?;
        validate_request_contract(&request)?;
        let validated = validate_wire_request(&request.context, &request.input.bytes)?;

        let mut transaction = self
            .store
            .begin_bound_read_transaction(&request.context.tenant_id)
            .await?;
        prove_canonical_party_claim(
            &mut transaction,
            &request.context.tenant_id,
            &validated.canonical_party_id,
            validated.identity_resolution_generation,
        )
        .await
        .map_err(map_canonical_party_claim_error)?;

        let page = read_data_quality_page(
            &mut transaction,
            &request.context.tenant_id,
            &validated,
        )
        .await?;
        let response = build_response(
            &validated,
            &page.resources,
            page.scanned_resource_count,
            page.next_state.as_ref(),
        )?;
        let output = typed_output(response.encode_to_vec())?;
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(QueryExecutionResult { output })
    }
}

impl QueryExecutor for DataQualityPrivacyScopeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move { self.execute_query(definition, request).await })
    }
}

struct DataQualityPage {
    resources: Vec<VerifiedDataQualityResource>,
    scanned_resource_count: u64,
    next_state: Option<CursorState>,
}

struct Versioned<T> {
    record_id: RecordId,
    version: u64,
    value: T,
}

struct DefinitionCatalog {
    rule_sets: BTreeMap<String, PartyRuleSetVersion>,
    profiles: BTreeMap<String, PartyCompletenessProfileVersion>,
}

struct DirectRecords {
    jobs: Vec<Versioned<PartyEvaluationJob>>,
    inputs: Vec<Versioned<PartyEvaluationInputSnapshot>>,
    outcomes: Vec<Versioned<PartyRuleOutcome>>,
    findings: Vec<Versioned<PartyFinding>>,
    observations: Vec<Versioned<PartyFindingObservation>>,
    completeness_results: Vec<Versioned<PartyCompletenessResult>>,
    remediation_attempts: Vec<Versioned<PartyDisplayNameRemediationAttempt>>,
}

struct CanonicalResolutionCache {
    values: BTreeMap<String, bool>,
    examined: usize,
}

impl CanonicalResolutionCache {
    fn new() -> Self {
        Self {
            values: BTreeMap::new(),
            examined: 0,
        }
    }

    async fn resolves_to_subject(
        &mut self,
        transaction: &mut BoundReadTransaction<'_>,
        tenant_id: &TenantId,
        party_id: &str,
        request: &ValidatedRequest,
    ) -> Result<bool, SdkError> {
        if let Some(relevant) = self.values.get(party_id) {
            return Ok(*relevant);
        }
        self.examined = self
            .examined
            .checked_add(1)
            .ok_or_else(|| scan_limit_exceeded("canonical Party resolution counter overflowed"))?;
        if self.examined > MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS {
            return Err(scan_limit_exceeded(
                "canonical Party resolution count exceeded the frozen privacy bound",
            ));
        }

        let requested = PartyReference::try_new(party_id).map_err(|error| {
            canonical_resolution_unavailable(format!(
                "persisted Data Quality Party reference is invalid: {error}"
            ))
        })?;
        let canonical =
            PartyReference::try_new(request.canonical_party_id.as_str()).map_err(|error| {
                canonical_resolution_unavailable(format!(
                    "accepted canonical Party reference is invalid: {error}"
                ))
            })?;
        let relevant = match prove_canonical_party_in_transaction(
            transaction,
            tenant_id,
            &requested,
            &canonical,
            request.identity_resolution_generation,
        )
        .await
        {
            Ok(_) => true,
            Err(error) if error.code == "IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH" => false,
            Err(error) if error.code == "IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE" => {
                return Err(lineage_invalid(
                    ErrorCategory::Conflict,
                    true,
                    "Identity Resolution topology generation changed during Data Quality scope discovery",
                ));
            }
            Err(error) => {
                return Err(canonical_resolution_unavailable(format!(
                    "{}: {}",
                    error.code, error.safe_message
                )));
            }
        };
        self.values.insert(party_id.to_owned(), relevant);
        Ok(relevant)
    }
}

struct AssociationCounter {
    examined: usize,
}

impl AssociationCounter {
    fn new() -> Self {
        Self { examined: 0 }
    }

    fn charge(&mut self, reference: &'static str) -> Result<(), SdkError> {
        self.examined = self
            .examined
            .checked_add(1)
            .ok_or_else(|| scan_limit_exceeded("association rehydration counter overflowed"))?;
        if self.examined > MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED {
            return Err(scan_limit_exceeded(format!(
                "association rehydration exceeded the frozen privacy bound while resolving {reference}"
            )));
        }
        Ok(())
    }
}

async fn read_data_quality_page(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &TenantId,
    request: &ValidatedRequest,
) -> Result<DataQualityPage, SdkError> {
    let rule_set_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        PARTY_RULE_SET_VERSION_RECORD_TYPE,
        MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    )
    .await?;
    let profile_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
        MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    )
    .await?;
    let definition_count = rule_set_rows
        .len()
        .checked_add(profile_rows.len())
        .ok_or_else(|| scan_limit_exceeded("definition rehydration counter overflowed"))?;
    if definition_count > MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED {
        return Err(scan_limit_exceeded(
            "Data Quality definition rehydration exceeded the frozen privacy bound",
        ));
    }

    let job_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        PARTY_EVALUATION_JOB_RECORD_TYPE,
        MAX_PRIVACY_EVALUATION_JOBS_SCANNED,
    )
    .await?;
    let input_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        PARTY_EVALUATION_INPUT_RECORD_TYPE,
        MAX_PRIVACY_EVALUATION_INPUTS_SCANNED,
    )
    .await?;
    let outcome_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        RULE_OUTCOME_RECORD_TYPE,
        MAX_PRIVACY_RULE_OUTCOMES_SCANNED,
    )
    .await?;
    let finding_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        FINDING_RECORD_TYPE,
        MAX_PRIVACY_FINDINGS_SCANNED,
    )
    .await?;
    let observation_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        FINDING_OBSERVATION_RECORD_TYPE,
        MAX_PRIVACY_FINDING_OBSERVATIONS_SCANNED,
    )
    .await?;
    let completeness_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        PARTY_COMPLETENESS_RESULT_RECORD_TYPE,
        MAX_PRIVACY_COMPLETENESS_RESULTS_SCANNED,
    )
    .await?;
    let remediation_rows = load_record_rows(
        transaction,
        tenant_id.as_str(),
        REMEDIATION_ATTEMPT_RECORD_TYPE,
        MAX_PRIVACY_REMEDIATION_ATTEMPTS_SCANNED,
    )
    .await?;

    let scanned = [
        job_rows.len(),
        input_rows.len(),
        outcome_rows.len(),
        finding_rows.len(),
        observation_rows.len(),
        completeness_rows.len(),
        remediation_rows.len(),
    ]
    .into_iter()
    .try_fold(0_usize, |total, count| total.checked_add(count))
    .ok_or_else(|| scan_limit_exceeded("owner record scan count overflowed"))?;
    if scanned > MAX_PRIVACY_OWNER_RECORDS_SCANNED {
        return Err(scan_limit_exceeded(
            "Data Quality owner record scan exceeded the frozen privacy bound",
        ));
    }

    let definitions = strict_definitions(rule_set_rows, profile_rows)?;
    let records = DirectRecords {
        jobs: job_rows
            .into_iter()
            .map(strict_job)
            .collect::<Result<Vec<_>, _>>()?,
        inputs: input_rows
            .into_iter()
            .map(strict_input)
            .collect::<Result<Vec<_>, _>>()?,
        outcomes: outcome_rows
            .into_iter()
            .map(strict_outcome)
            .collect::<Result<Vec<_>, _>>()?,
        findings: finding_rows
            .into_iter()
            .map(strict_finding)
            .collect::<Result<Vec<_>, _>>()?,
        observations: observation_rows
            .into_iter()
            .map(strict_observation)
            .collect::<Result<Vec<_>, _>>()?,
        completeness_results: completeness_rows
            .into_iter()
            .map(strict_completeness_result)
            .collect::<Result<Vec<_>, _>>()?,
        remediation_attempts: remediation_rows
            .into_iter()
            .map(strict_remediation_attempt)
            .collect::<Result<Vec<_>, _>>()?,
    };

    validate_associations(tenant_id, &definitions, &records)?;

    let mut resolution_cache = CanonicalResolutionCache::new();
    let mut ordered = Vec::new();
    append_relevant(
        transaction,
        tenant_id,
        request,
        &mut resolution_cache,
        &mut ordered,
        ResourceFamily::EvaluationJob,
        records.jobs.iter().map(|record| {
            (
                record.value.party_id().as_str(),
                &record.record_id,
                record.version,
            )
        }),
    )
    .await?;
    append_relevant(
        transaction,
        tenant_id,
        request,
        &mut resolution_cache,
        &mut ordered,
        ResourceFamily::EvaluationInput,
        records.inputs.iter().map(|record| {
            (
                record.value.party_id().as_str(),
                &record.record_id,
                record.version,
            )
        }),
    )
    .await?;
    append_relevant(
        transaction,
        tenant_id,
        request,
        &mut resolution_cache,
        &mut ordered,
        ResourceFamily::RuleOutcome,
        records.outcomes.iter().map(|record| {
            (
                record.value.party_id().as_str(),
                &record.record_id,
                record.version,
            )
        }),
    )
    .await?;
    append_relevant(
        transaction,
        tenant_id,
        request,
        &mut resolution_cache,
        &mut ordered,
        ResourceFamily::Finding,
        records.findings.iter().map(|record| {
            (
                record.value.party_id().as_str(),
                &record.record_id,
                record.version,
            )
        }),
    )
    .await?;
    append_relevant(
        transaction,
        tenant_id,
        request,
        &mut resolution_cache,
        &mut ordered,
        ResourceFamily::FindingObservation,
        records.observations.iter().map(|record| {
            (
                record.value.party_id().as_str(),
                &record.record_id,
                record.version,
            )
        }),
    )
    .await?;
    append_relevant(
        transaction,
        tenant_id,
        request,
        &mut resolution_cache,
        &mut ordered,
        ResourceFamily::CompletenessResult,
        records.completeness_results.iter().map(|record| {
            (
                record.value.party_id().as_str(),
                &record.record_id,
                record.version,
            )
        }),
    )
    .await?;
    append_relevant(
        transaction,
        tenant_id,
        request,
        &mut resolution_cache,
        &mut ordered,
        ResourceFamily::RemediationAttempt,
        records.remediation_attempts.iter().map(|record| {
            (
                record.value.party_id().as_str(),
                &record.record_id,
                record.version,
            )
        }),
    )
    .await?;

    let mut matching = ordered
        .into_iter()
        .filter(|resource| resource_after_cursor(resource, &request.cursor_state))
        .take(request.page_size as usize + 1)
        .collect::<Vec<_>>();
    let has_more = matching.len() > request.page_size as usize;
    if has_more {
        matching.pop();
    }
    let next_state = if has_more {
        let last = matching.last().ok_or_else(|| {
            association_state_invalid("Data Quality page continuation has no anchor")
        })?;
        Some(CursorState {
            family: last.family,
            after_record_id: Some(last.record_id.clone()),
        })
    } else {
        None
    };

    Ok(DataQualityPage {
        resources: matching,
        scanned_resource_count: u64::try_from(scanned)
            .map_err(|_| scan_limit_exceeded("owner scan count does not fit in u64"))?,
        next_state,
    })
}

async fn append_relevant<'a, I>(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &TenantId,
    request: &ValidatedRequest,
    cache: &mut CanonicalResolutionCache,
    output: &mut Vec<VerifiedDataQualityResource>,
    family: ResourceFamily,
    records: I,
) -> Result<(), SdkError>
where
    I: Iterator<Item = (&'a str, &'a RecordId, u64)>,
{
    for (party_id, record_id, version) in records {
        if cache
            .resolves_to_subject(transaction, tenant_id, party_id, request)
            .await?
        {
            output.push(VerifiedDataQualityResource {
                family,
                record_id: record_id.clone(),
                resource_version: version,
            });
        }
    }
    Ok(())
}

fn validate_associations(
    tenant_id: &TenantId,
    definitions: &DefinitionCatalog,
    records: &DirectRecords,
) -> Result<(), SdkError> {
    let mut counter = AssociationCounter::new();
    let jobs = records
        .jobs
        .iter()
        .map(|record| (record.value.job_id().as_str(), record))
        .collect::<BTreeMap<_, _>>();
    let inputs = records
        .inputs
        .iter()
        .map(|record| (record.value.job_id().as_str(), record))
        .collect::<BTreeMap<_, _>>();
    let outcomes = records
        .outcomes
        .iter()
        .map(|record| (record.value.outcome_id(), record))
        .collect::<BTreeMap<_, _>>();
    let findings = records
        .findings
        .iter()
        .map(|record| (record.value.finding_id(), record))
        .collect::<BTreeMap<_, _>>();
    let observations = records
        .observations
        .iter()
        .map(|record| (record.value.observation_id(), record))
        .collect::<BTreeMap<_, _>>();

    for record in &records.jobs {
        let job = &record.value;
        counter.charge("evaluation job rule set")?;
        let rule_set = definitions
            .rule_sets
            .get(job.rule_set_version_id())
            .ok_or_else(|| association_state_invalid("evaluation job references a missing rule set"))?;
        counter.charge("evaluation job completeness profile")?;
        let profile = definitions
            .profiles
            .get(job.profile_version_id())
            .ok_or_else(|| {
                association_state_invalid("evaluation job references a missing completeness profile")
            })?;
        if profile.rule_set_version_id().as_str() != rule_set.version_id().as_str() {
            return Err(association_state_invalid(
                "evaluation job profile and rule-set bindings differ",
            ));
        }
        let rule_count = u32::try_from(rule_set.rules().len())
            .map_err(|_| association_state_invalid("rule-set size does not fit in u32"))?;
        if job.evaluated_rules() > rule_count || job.failed_rules() > job.evaluated_rules() {
            return Err(association_state_invalid(
                "evaluation job counters exceed its exact rule-set definition",
            ));
        }

        let input = inputs.get(job.job_id().as_str());
        let job_outcomes = records
            .outcomes
            .iter()
            .filter(|outcome| outcome.value.job_id() == job.job_id())
            .collect::<Vec<_>>();
        let failed = u32::try_from(
            job_outcomes
                .iter()
                .filter(|outcome| !outcome.value.passed())
                .count(),
        )
        .map_err(|_| association_state_invalid("failed outcome count does not fit in u32"))?;
        match job.status() {
            PartyEvaluationJobStatus::Created => {
                if input.is_some() || !job_outcomes.is_empty() || job.party_resource_version().is_some() {
                    return Err(association_state_invalid(
                        "created evaluation job has staged or materialized evidence",
                    ));
                }
            }
            PartyEvaluationJobStatus::Staged => {
                if input.is_none() || job.outcomes_materialized() {
                    return Err(association_state_invalid(
                        "durable staged evaluation job has incomplete or premature outcome evidence",
                    ));
                }
            }
            PartyEvaluationJobStatus::Completed => {
                if input.is_none()
                    || job.evaluated_rules() != rule_count
                    || job_outcomes.len() != rule_count as usize
                    || job.failed_rules() != failed
                {
                    return Err(association_state_invalid(
                        "completed evaluation job does not reconcile to its exact input and outcomes",
                    ));
                }
            }
        }
    }

    for record in &records.inputs {
        let input = &record.value;
        counter.charge("evaluation input parent job")?;
        let job = jobs.get(input.job_id().as_str()).ok_or_else(|| {
            association_state_invalid("evaluation input references a missing parent job")
        })?;
        if record.record_id.as_str() != input.job_id().as_str()
            || input.party_id() != job.value.party_id()
            || Some(input.party_resource_version()) != job.value.party_resource_version()
            || input.captured_at() < job.value.created_at()
            || input.captured_at() > job.value.updated_at()
            || job.value.status() == PartyEvaluationJobStatus::Created
        {
            return Err(association_state_invalid(
                "evaluation input disagrees with its exact parent job",
            ));
        }
    }

    for record in &records.outcomes {
        let outcome = &record.value;
        counter.charge("rule outcome parent job")?;
        let job = jobs.get(outcome.job_id().as_str()).ok_or_else(|| {
            association_state_invalid("rule outcome references a missing parent job")
        })?;
        counter.charge("rule outcome rule set")?;
        let rule_set = definitions
            .rule_sets
            .get(outcome.rule_set_version_id())
            .ok_or_else(|| association_state_invalid("rule outcome references a missing rule set"))?;
        counter.charge("rule outcome staged input")?;
        let input = inputs.get(outcome.job_id().as_str()).ok_or_else(|| {
            association_state_invalid("rule outcome references a job without staged input")
        })?;
        if outcome.party_id() != job.value.party_id()
            || outcome.party_resource_version() != job.value.party_resource_version().unwrap_or_default()
            || outcome.rule_set_version_id() != job.value.rule_set_version_id()
            || rule_set.rule(outcome.rule_key()).is_none()
            || outcome.evaluated_at() < input.value.captured_at()
            || outcome.evaluated_at() > job.value.updated_at()
            || job.value.status() != PartyEvaluationJobStatus::Completed
        {
            return Err(association_state_invalid(
                "rule outcome disagrees with its exact job, input or rule-set lineage",
            ));
        }
    }

    for record in &records.observations {
        let observation = &record.value;
        if observation.tenant_id() != tenant_id {
            return Err(association_state_invalid(
                "finding observation tenant differs from the bounded transaction",
            ));
        }
        counter.charge("finding observation parent finding")?;
        let finding = findings.get(observation.finding_id()).ok_or_else(|| {
            association_state_invalid("finding observation references a missing parent finding")
        })?;
        if observation.party_id() != finding.value.party_id()
            || observation.rule_set_version_id() != finding.value.rule_set_version_id()
            || observation.rule_key() != finding.value.rule_key()
            || observation.observed_at() > finding.value.updated_at()
        {
            return Err(association_state_invalid(
                "finding observation disagrees with its exact parent finding",
            ));
        }
    }

    for record in &records.findings {
        let finding = &record.value;
        if finding.tenant_id() != tenant_id {
            return Err(association_state_invalid(
                "finding tenant differs from the bounded transaction",
            ));
        }
        counter.charge("finding rule set")?;
        let rule_set = definitions
            .rule_sets
            .get(finding.rule_set_version_id())
            .ok_or_else(|| association_state_invalid("finding references a missing rule set"))?;
        counter.charge("finding current observation")?;
        let current = observations
            .get(finding.current_observation_id())
            .ok_or_else(|| association_state_invalid("finding current observation is missing"))?;
        if rule_set.rule(finding.rule_key()).is_none()
            || current.value.finding_id() != finding.finding_id()
            || current.value.party_id() != finding.party_id()
            || current.value.rule_set_version_id() != finding.rule_set_version_id()
            || current.value.rule_key() != finding.rule_key()
            || current.value.party_resource_version() > finding.evaluated_party_resource_version()
        {
            return Err(association_state_invalid(
                "finding current evidence or rule lineage is invalid",
            ));
        }
        if let Some(outcome_id) = finding.remediated_by_rule_outcome_id() {
            counter.charge("finding remediating outcome")?;
            let outcome = outcomes.get(outcome_id).ok_or_else(|| {
                association_state_invalid("finding remediating outcome is missing")
            })?;
            if !outcome.value.passed()
                || outcome.value.party_id() != finding.party_id()
                || outcome.value.rule_set_version_id() != finding.rule_set_version_id()
                || outcome.value.rule_key() != finding.rule_key()
                || outcome.value.party_resource_version()
                    != finding.evaluated_party_resource_version()
            {
                return Err(association_state_invalid(
                    "finding remediating outcome disagrees with finding lineage",
                ));
            }
        }
    }

    for record in &records.completeness_results {
        let result = &record.value;
        counter.charge("completeness result parent job")?;
        let job = jobs.get(result.job_id().as_str()).ok_or_else(|| {
            association_state_invalid("completeness result references a missing parent job")
        })?;
        counter.charge("completeness result profile")?;
        let profile = definitions
            .profiles
            .get(result.profile_version_id())
            .ok_or_else(|| {
                association_state_invalid("completeness result references a missing profile")
            })?;
        counter.charge("completeness result rule set")?;
        let rule_set = definitions
            .rule_sets
            .get(profile.rule_set_version_id().as_str())
            .ok_or_else(|| {
                association_state_invalid("completeness result profile rule set is missing")
            })?;
        if result.party_id() != job.value.party_id()
            || result.party_resource_version() != job.value.party_resource_version().unwrap_or_default()
            || result.profile_version_id() != job.value.profile_version_id()
            || profile.rule_set_version_id().as_str() != job.value.rule_set_version_id()
            || job.value.status() != PartyEvaluationJobStatus::Completed
            || result.computed_at() > job.value.updated_at()
        {
            return Err(association_state_invalid(
                "completeness result disagrees with its exact job and definition lineage",
            ));
        }
        for component in result.components() {
            counter.charge("completeness component outcome")?;
            let outcome = outcomes.get(component.rule_outcome_id()).ok_or_else(|| {
                association_state_invalid("completeness component outcome is missing")
            })?;
            let profile_component = profile
                .components()
                .iter()
                .find(|candidate| candidate.component_key() == component.component_key())
                .ok_or_else(|| {
                    association_state_invalid("completeness component is absent from its profile")
                })?;
            let expected_award = if outcome.value.passed() {
                profile_component.weight_basis_points()
            } else {
                0
            };
            if profile_component.rule_key() != component.rule_key()
                || rule_set.rule(component.rule_key()).is_none()
                || outcome.value.job_id() != result.job_id()
                || outcome.value.party_id() != result.party_id()
                || outcome.value.rule_set_version_id() != job.value.rule_set_version_id()
                || outcome.value.rule_key() != component.rule_key()
                || component.awarded_basis_points() != expected_award
                || outcome.value.evaluated_at() > result.computed_at()
            {
                return Err(association_state_invalid(
                    "completeness component disagrees with profile or rule-outcome lineage",
                ));
            }
        }
    }

    for record in &records.remediation_attempts {
        let attempt = &record.value;
        if attempt.tenant_id() != tenant_id {
            return Err(association_state_invalid(
                "remediation attempt tenant differs from the bounded transaction",
            ));
        }
        counter.charge("remediation attempt finding")?;
        let finding = findings.get(attempt.finding_id()).ok_or_else(|| {
            association_state_invalid("remediation attempt references a missing finding")
        })?;
        counter.charge("remediation attempt observation")?;
        let observation = observations.get(attempt.observation_id()).ok_or_else(|| {
            association_state_invalid("remediation attempt references a missing observation")
        })?;
        if attempt.party_id() != finding.value.party_id()
            || observation.value.finding_id() != finding.value.finding_id()
            || observation.value.party_id() != attempt.party_id()
            || attempt.expected_party_version() != observation.value.party_resource_version()
            || attempt.completed_at() < observation.value.observed_at()
        {
            return Err(association_state_invalid(
                "remediation attempt disagrees with finding or observation lineage",
            ));
        }
    }

    for job in &records.jobs {
        if job.value.status() == PartyEvaluationJobStatus::Completed {
            let completeness_count = records
                .completeness_results
                .iter()
                .filter(|result| result.value.job_id() == job.value.job_id())
                .count();
            if completeness_count != 1 {
                return Err(association_state_invalid(
                    "completed evaluation job does not have exactly one completeness result",
                ));
            }
        }
    }

    Ok(())
}

fn strict_definitions(
    rule_set_rows: Vec<StoredRecordRow>,
    profile_rows: Vec<StoredRecordRow>,
) -> Result<DefinitionCatalog, SdkError> {
    let mut rule_sets = BTreeMap::new();
    for row in rule_set_rows {
        let snapshot = strict_snapshot(
            &row,
            PARTY_RULE_SET_VERSION_RECORD_TYPE,
            party_rule_set_persisted_contract(),
            DataClass::Confidential,
        )?;
        let rule_set = party_rule_set_from_snapshot(&snapshot).map_err(map_owner_error)?;
        let id = rule_set.version_id().as_str().to_owned();
        if rule_sets.insert(id, rule_set).is_some() {
            return Err(stored_state_invalid("duplicate strict rule-set identity"));
        }
    }

    let mut profiles = BTreeMap::new();
    for row in profile_rows {
        let snapshot = strict_snapshot(
            &row,
            PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
            party_completeness_profile_persisted_contract(),
            DataClass::Confidential,
        )?;
        let rule_set_id = completeness_profile_rule_set_version_id_from_snapshot(&snapshot)
            .map_err(map_owner_error)?;
        let rule_set = rule_sets.get(&rule_set_id).ok_or_else(|| {
            stored_state_invalid("completeness profile references a missing strict rule set")
        })?;
        let profile = party_completeness_profile_from_immutable_snapshot(&snapshot, rule_set)
            .map_err(map_owner_error)?;
        let id = profile.version_id().as_str().to_owned();
        if profiles.insert(id, profile).is_some() {
            return Err(stored_state_invalid(
                "duplicate strict completeness-profile identity",
            ));
        }
    }
    Ok(DefinitionCatalog {
        rule_sets,
        profiles,
    })
}

fn strict_job(row: StoredRecordRow) -> Result<Versioned<PartyEvaluationJob>, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        PARTY_EVALUATION_JOB_RECORD_TYPE,
        party_evaluation_job_persisted_contract(),
        DataClass::Personal,
    )?;
    let value = party_evaluation_job_from_snapshot(&snapshot).map_err(map_owner_error)?;
    Ok(versioned(row, value)?)
}

fn strict_input(row: StoredRecordRow) -> Result<Versioned<PartyEvaluationInputSnapshot>, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        PARTY_EVALUATION_INPUT_RECORD_TYPE,
        party_evaluation_input_persisted_contract(),
        DataClass::Personal,
    )?;
    let value = decode_party_evaluation_input_state(&snapshot.payload.bytes).map_err(map_owner_error)?;
    if value.job_id().as_str() != row.record_id.as_str() || row.version != 1 {
        return Err(stored_state_invalid(
            "evaluation input identity/version disagrees with its authoritative record",
        ));
    }
    Ok(versioned(row, value)?)
}

fn strict_outcome(row: StoredRecordRow) -> Result<Versioned<PartyRuleOutcome>, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        RULE_OUTCOME_RECORD_TYPE,
        party_rule_outcome_persisted_contract(),
        DataClass::Personal,
    )?;
    let value = decode_rule_outcome_state(&snapshot.payload.bytes).map_err(map_owner_error)?;
    if value.outcome_id() != row.record_id.as_str() || row.version != 1 {
        return Err(stored_state_invalid(
            "rule outcome identity/version disagrees with its authoritative record",
        ));
    }
    Ok(versioned(row, value)?)
}

fn strict_finding(row: StoredRecordRow) -> Result<Versioned<PartyFinding>, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        FINDING_RECORD_TYPE,
        party_finding_persisted_contract(),
        DataClass::Personal,
    )?;
    let value = decode_finding_state(&snapshot.payload.bytes).map_err(map_owner_error)?;
    if value.finding_id() != row.record_id.as_str() {
        return Err(stored_state_invalid(
            "finding identity disagrees with its authoritative record",
        ));
    }
    Ok(versioned(row, value)?)
}

fn strict_observation(row: StoredRecordRow) -> Result<Versioned<PartyFindingObservation>, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        FINDING_OBSERVATION_RECORD_TYPE,
        party_finding_observation_persisted_contract(),
        DataClass::Personal,
    )?;
    let value = decode_finding_observation_state(&snapshot.payload.bytes).map_err(map_owner_error)?;
    if value.observation_id() != row.record_id.as_str() || row.version != 1 {
        return Err(stored_state_invalid(
            "finding observation identity/version disagrees with its authoritative record",
        ));
    }
    Ok(versioned(row, value)?)
}

fn strict_completeness_result(
    row: StoredRecordRow,
) -> Result<Versioned<PartyCompletenessResult>, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        PARTY_COMPLETENESS_RESULT_RECORD_TYPE,
        party_completeness_result_persisted_contract(),
        DataClass::Personal,
    )?;
    let value = decode_party_completeness_result_state(&snapshot.payload.bytes)
        .map_err(map_owner_error)?;
    if value.result_id() != row.record_id.as_str() || row.version != 1 {
        return Err(stored_state_invalid(
            "completeness result identity/version disagrees with its authoritative record",
        ));
    }
    Ok(versioned(row, value)?)
}

fn strict_remediation_attempt(
    row: StoredRecordRow,
) -> Result<Versioned<PartyDisplayNameRemediationAttempt>, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        REMEDIATION_ATTEMPT_RECORD_TYPE,
        remediation_attempt_persisted_contract(),
        DataClass::Personal,
    )?;
    let value = decode_remediation_attempt_state(&snapshot.payload.bytes).map_err(map_owner_error)?;
    if value.attempt_id() != row.record_id.as_str() || row.version != 1 {
        return Err(stored_state_invalid(
            "remediation attempt identity/version disagrees with its authoritative record",
        ));
    }
    Ok(versioned(row, value)?)
}

fn versioned<T>(row: StoredRecordRow, value: T) -> Result<Versioned<T>, SdkError> {
    Ok(Versioned {
        record_id: row.record_id,
        version: positive_version(row.version)?,
        value,
    })
}

fn map_owner_error(error: SdkError) -> SdkError {
    stored_state_invalid(format!("{}: {}", error.code, error.safe_message))
}

fn resource_after_cursor(resource: &VerifiedDataQualityResource, cursor: &CursorState) -> bool {
    match resource.family.cmp(&cursor.family) {
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Equal => cursor
            .after_record_id
            .as_ref()
            .is_none_or(|after| resource.record_id.as_str() > after.as_str()),
    }
}

struct StoredRecordRow {
    record_id: RecordId,
    version: i64,
    owner_module_id: String,
    schema_id: String,
    schema_version: String,
    descriptor_hash: Vec<u8>,
    data_class: String,
    payload_encoding: String,
    maximum_payload_size: i64,
    retention_policy_id: String,
    payload_bytes: Vec<u8>,
}

async fn load_record_rows(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &str,
    record_type: &str,
    maximum: usize,
) -> Result<Vec<StoredRecordRow>, SdkError> {
    let mut after_record_id = String::new();
    let mut output = Vec::new();
    loop {
        let rows = sqlx::query(
            r#"
            SELECT
              record_id,
              version,
              owner_module_id,
              schema_id,
              schema_version,
              descriptor_hash,
              data_class,
              payload_encoding,
              maximum_payload_size,
              retention_policy_id,
              payload_bytes
            FROM crm.records
            WHERE tenant_id = $1
              AND owner_module_id = $2
              AND record_type = $3
              AND record_id > $4
              AND deleted_at IS NULL
            ORDER BY record_id ASC
            LIMIT $5
            "#,
        )
        .bind(tenant_id)
        .bind(MODULE_ID)
        .bind(record_type)
        .bind(&after_record_id)
        .bind(PRIVACY_OWNER_SCAN_BATCH_SIZE)
        .fetch_all(&mut ***transaction)
        .await
        .map_err(database_unavailable)?;
        if rows.is_empty() {
            break;
        }
        let batch_len = rows.len();
        for row in rows {
            let decoded = decode_stored_row(row)?;
            after_record_id = decoded.record_id.as_str().to_owned();
            output.push(decoded);
            if output.len() > maximum {
                return Err(scan_limit_exceeded(format!(
                    "{record_type} scan exceeded the frozen privacy bound"
                )));
            }
        }
        if batch_len < PRIVACY_OWNER_SCAN_BATCH_SIZE as usize {
            break;
        }
    }
    Ok(output)
}

fn decode_stored_row(row: sqlx::postgres::PgRow) -> Result<StoredRecordRow, SdkError> {
    let invalid = |reference: String| stored_state_invalid(reference);
    Ok(StoredRecordRow {
        record_id: RecordId::try_new(
            row.try_get::<String, _>("record_id")
                .map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(|error| invalid(error.to_string()))?,
        version: row
            .try_get("version")
            .map_err(|error| invalid(error.to_string()))?,
        owner_module_id: row
            .try_get("owner_module_id")
            .map_err(|error| invalid(error.to_string()))?,
        schema_id: row
            .try_get("schema_id")
            .map_err(|error| invalid(error.to_string()))?,
        schema_version: row
            .try_get("schema_version")
            .map_err(|error| invalid(error.to_string()))?,
        descriptor_hash: row
            .try_get("descriptor_hash")
            .map_err(|error| invalid(error.to_string()))?,
        data_class: row
            .try_get("data_class")
            .map_err(|error| invalid(error.to_string()))?,
        payload_encoding: row
            .try_get("payload_encoding")
            .map_err(|error| invalid(error.to_string()))?,
        maximum_payload_size: row
            .try_get("maximum_payload_size")
            .map_err(|error| invalid(error.to_string()))?,
        retention_policy_id: row
            .try_get("retention_policy_id")
            .map_err(|error| invalid(error.to_string()))?,
        payload_bytes: row
            .try_get("payload_bytes")
            .map_err(|error| invalid(error.to_string()))?,
    })
}

fn strict_snapshot(
    row: &StoredRecordRow,
    record_type: &str,
    contract: PersistedPayloadContract<'_>,
    data_class: DataClass,
) -> Result<RecordSnapshot, SdkError> {
    let expected_data_class = match data_class {
        DataClass::Confidential => "confidential",
        DataClass::Personal => "personal",
        _ => {
            return Err(stored_state_invalid(
                "unsupported Data Quality privacy persisted data class",
            ));
        }
    };
    if row.version <= 0
        || row.owner_module_id != contract.owner
        || row.schema_id != contract.schema_id
        || row.schema_version != contract.schema_version
        || row.descriptor_hash.as_slice() != contract.descriptor_hash
        || row.data_class != expected_data_class
        || row.payload_encoding != "json"
        || row.maximum_payload_size != contract.maximum_size_bytes as i64
        || row.retention_policy_id != contract.retention_policy_id
    {
        return Err(stored_state_invalid(
            "persisted metadata does not match the canonical Data Quality state contract",
        ));
    }
    let snapshot = RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            record_id: row.record_id.clone(),
        },
        version: row.version,
        payload: TypedPayload {
            owner: ModuleId::try_new(contract.owner)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            schema_id: SchemaId::try_new(contract.schema_id)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            schema_version: SchemaVersion::try_new(contract.schema_version)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            descriptor_hash: contract.descriptor_hash,
            data_class,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: contract.maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new(contract.retention_policy_id)
                .map_err(|error| stored_state_invalid(error.to_string()))?,
            bytes: row.payload_bytes.clone(),
        },
    };
    snapshot
        .payload
        .validate()
        .map_err(|error| stored_state_invalid(error.to_string()))?;
    Ok(snapshot)
}

fn positive_version(version: i64) -> Result<u64, SdkError> {
    u64::try_from(version)
        .map_err(|_| stored_state_invalid("resource version must be positive"))
}
