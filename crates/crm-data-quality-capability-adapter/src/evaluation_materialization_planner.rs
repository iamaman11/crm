use crate::{
    MATERIALIZE_PARTY_EVALUATION_CAPABILITY, MATERIALIZE_PARTY_EVALUATION_REQUEST_SCHEMA,
    MATERIALIZE_PARTY_EVALUATION_RESPONSE_SCHEMA, MODULE_ID,
    PARTY_EVALUATION_MATERIALIZED_EVENT_SCHEMA, PARTY_EVALUATION_MATERIALIZED_EVENT_TYPE,
    party_evaluation_job_from_snapshot, party_evaluation_job_persisted_payload,
    party_evaluation_job_to_wire,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_data_quality::{
    FINDING_OBSERVATION_RECORD_TYPE, FINDING_OBSERVATION_STATE_MAXIMUM_BYTES,
    FINDING_OBSERVATION_STATE_RETENTION_POLICY_ID, FINDING_OBSERVATION_STATE_SCHEMA_ID,
    FINDING_OBSERVATION_STATE_SCHEMA_VERSION, FINDING_RECORD_TYPE, FINDING_STATE_MAXIMUM_BYTES,
    FINDING_STATE_RETENTION_POLICY_ID, FINDING_STATE_SCHEMA_ID, FINDING_STATE_SCHEMA_VERSION,
    PARTY_COMPLETENESS_RESULT_RECORD_TYPE, PARTY_COMPLETENESS_RESULT_STATE_MAXIMUM_BYTES,
    PARTY_COMPLETENESS_RESULT_STATE_RETENTION_POLICY_ID, PARTY_COMPLETENESS_RESULT_STATE_SCHEMA_ID,
    PARTY_COMPLETENESS_RESULT_STATE_SCHEMA_VERSION, PartyCompletenessProfileVersion,
    PartyCompletenessResult, PartyEvaluationInputSnapshot, PartyEvaluationJobStatus, PartyFinding,
    PartyFindingObservation, PartyQualityInput, PartyRuleOutcome, PartyRuleSetVersion,
    RULE_OUTCOME_RECORD_TYPE, RULE_OUTCOME_STATE_MAXIMUM_BYTES,
    RULE_OUTCOME_STATE_RETENTION_POLICY_ID, RULE_OUTCOME_STATE_SCHEMA_ID,
    RULE_OUTCOME_STATE_SCHEMA_VERSION, encode_finding_observation_state, encode_finding_state,
    encode_party_completeness_result_state, encode_rule_outcome_state,
    finding_observation_state_descriptor_hash, finding_state_descriptor_hash,
    party_completeness_result_state_descriptor_hash, party_finding_id,
    rule_outcome_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordId, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::data_quality::v1 as wire;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ExistingPartyFinding {
    pub version: i64,
    pub finding: PartyFinding,
}

#[derive(Debug, Clone)]
pub struct ExistingPartyFindingObservation {
    pub observation: PartyFindingObservation,
}

#[derive(Debug, Clone)]
pub struct DataQualityEvaluationMaterializationPlanner {
    rule_set: PartyRuleSetVersion,
    profile: PartyCompletenessProfileVersion,
    input: PartyEvaluationInputSnapshot,
    current_findings: BTreeMap<String, ExistingPartyFinding>,
    current_observations: BTreeMap<String, ExistingPartyFindingObservation>,
}

impl DataQualityEvaluationMaterializationPlanner {
    pub fn new(
        rule_set: PartyRuleSetVersion,
        profile: PartyCompletenessProfileVersion,
        input: PartyEvaluationInputSnapshot,
        current_findings: BTreeMap<String, ExistingPartyFinding>,
        current_observations: BTreeMap<String, ExistingPartyFindingObservation>,
    ) -> Result<Self, SdkError> {
        if profile.rule_set_version_id() != rule_set.version_id()
            || current_findings.iter().any(|(finding_id, existing)| {
                existing.version <= 0 || existing.finding.finding_id() != finding_id
            })
            || current_observations
                .iter()
                .any(|(observation_id, existing)| {
                    existing.observation.observation_id() != observation_id
                })
        {
            return Err(invalid_plan(
                "materialization definitions or current finding evidence are invalid",
            ));
        }
        Ok(Self {
            rule_set,
            profile,
            input,
            current_findings,
            current_observations,
        })
    }
}

impl TransactionalAggregatePlanner for DataQualityEvaluationMaterializationPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let command = decode_request(request)?;
        Ok(AggregateTarget {
            reference: job_record_ref(command.evaluation_job_ref)?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        let current = current.ok_or_else(|| invalid_plan("evaluation job is unavailable"))?;
        let command = decode_request(request)?;
        if command.expected_job_version <= 0 || command.expected_job_version != current.version {
            return Err(SdkError::new(
                "DATA_QUALITY_EVALUATION_MATERIALIZATION_VERSION_CONFLICT",
                ErrorCategory::Conflict,
                false,
                "The Party evaluation job changed before outcomes could be materialized.",
            ));
        }
        let job = party_evaluation_job_from_snapshot(current)?;
        if job.status() != PartyEvaluationJobStatus::Staged || job.outcomes_materialized() {
            return Err(invalid_plan(
                "only an unmaterialized staged evaluation job can cross the completion boundary",
            ));
        }
        if self.input.job_id() != job.job_id()
            || self.input.party_id() != job.party_id()
            || self.input.party_resource_version()
                != job.party_resource_version().unwrap_or_default()
            || self.input.captured_at() != job.updated_at()
            || self.rule_set.version_id().as_str() != job.rule_set_version_id()
            || self.profile.version_id().as_str() != job.profile_version_id()
        {
            return Err(invalid_plan(
                "materialization inputs do not match the exact staged job",
            ));
        }

        let quality_input =
            PartyQualityInput::try_new(self.input.kind(), self.input.display_name())?;
        let evaluations = self.rule_set.evaluate(&quality_input);
        let evaluated_rules = u32::try_from(evaluations.len())
            .map_err(|_| invalid_plan("evaluation rule count overflowed"))?;
        let outcomes = evaluations
            .iter()
            .map(|evaluation| {
                PartyRuleOutcome::evaluate(&job, evaluation, self.input.captured_at())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let failed_rules =
            u32::try_from(outcomes.iter().filter(|outcome| !outcome.passed()).count())
                .map_err(|_| invalid_plan("failed evaluation rule count overflowed"))?;
        let completeness = PartyCompletenessResult::compute(
            &job,
            &self.profile,
            &outcomes,
            self.input.captured_at(),
        )?;
        let materialized_job = job.record_materialized_outcomes(
            evaluated_rules,
            failed_rules,
            self.input.captured_at(),
        )?;
        let completed_job = materialized_job.complete(
            evaluated_rules,
            failed_rules,
            request.context.execution.request_started_at_unix_nanos,
        )?;

        let aggregate = current.reference.clone();
        let aggregate_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| invalid_plan("evaluation job version overflowed"))?;
        let outcome_refs = outcomes
            .iter()
            .map(|outcome| wire::PartyRuleOutcomeRef {
                rule_outcome_id: outcome.outcome_id().to_owned(),
            })
            .collect::<Vec<_>>();
        let completeness_ref = wire::PartyCompletenessResultRef {
            completeness_result_id: completeness.result_id().to_owned(),
        };

        let mut finding_refs = Vec::new();
        let mut observation_refs = Vec::new();
        let mut finding_effects = Vec::new();
        for outcome in &outcomes {
            let finding_id = party_finding_id(
                &request.context.execution.tenant_id,
                outcome.party_id(),
                outcome.rule_set_version_id(),
                outcome.rule_key(),
            );
            let existing_finding = self.current_findings.get(&finding_id);
            if outcome.passed() {
                if let Some(existing) = existing_finding {
                    let updated = existing.finding.apply_passing_outcome(outcome)?;
                    finding_refs.push(wire::DataQualityFindingRef {
                        finding_id: finding_id.clone(),
                    });
                    if updated != existing.finding {
                        finding_effects.push(RecordMutation::Update {
                            reference: party_finding_record_ref(&updated)?,
                            expected_version: existing.version,
                            payload: party_finding_persisted_payload(&updated)?,
                        });
                    }
                }
                continue;
            }

            let rule = self
                .rule_set
                .rule(outcome.rule_key())
                .ok_or_else(|| invalid_plan("failed outcome rule is unavailable"))?;
            let observation = PartyFindingObservation::observe_failure(
                request.context.execution.tenant_id.clone(),
                rule,
                outcome,
            )?;
            finding_refs.push(wire::DataQualityFindingRef {
                finding_id: finding_id.clone(),
            });
            observation_refs.push(wire::DataQualityFindingObservationRef {
                finding_observation_id: observation.observation_id().to_owned(),
            });
            match self.current_observations.get(observation.observation_id()) {
                Some(existing) if existing.observation == observation => {}
                Some(_) => {
                    return Err(invalid_plan(
                        "existing finding observation differs from deterministic evidence",
                    ));
                }
                None => finding_effects.push(RecordMutation::Create {
                    reference: party_finding_observation_record_ref(&observation)?,
                    payload: party_finding_observation_persisted_payload(&observation)?,
                }),
            }
            match existing_finding {
                Some(existing) => {
                    let updated = existing.finding.apply_failed_observation(&observation)?;
                    if updated != existing.finding {
                        finding_effects.push(RecordMutation::Update {
                            reference: party_finding_record_ref(&updated)?,
                            expected_version: existing.version,
                            payload: party_finding_persisted_payload(&updated)?,
                        });
                    }
                }
                None => {
                    let opened = PartyFinding::open(rule, &observation)?;
                    finding_effects.push(RecordMutation::Create {
                        reference: party_finding_record_ref(&opened)?,
                        payload: party_finding_persisted_payload(&opened)?,
                    });
                }
            }
        }

        let public_job = party_evaluation_job_to_wire(&completed_job, aggregate_version);
        let output = support::protobuf_payload(
            MODULE_ID,
            MATERIALIZE_PARTY_EVALUATION_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::MaterializePartyEvaluationResponse {
                evaluation_job: Some(public_job.clone()),
                rule_outcome_refs: outcome_refs.clone(),
                completeness_result_ref: Some(completeness_ref.clone()),
                finding_refs: finding_refs.clone(),
                finding_observation_refs: observation_refs.clone(),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PARTY_EVALUATION_MATERIALIZED_EVENT_TYPE,
                event_schema_id: PARTY_EVALUATION_MATERIALIZED_EVENT_SCHEMA,
                aggregate_version,
                previous_version: Some(current.version),
            },
            DataClass::Personal,
            &wire::PartyEvaluationMaterializedEvent {
                evaluation_job: Some(public_job),
                rule_outcome_refs: outcome_refs,
                completeness_result_ref: Some(completeness_ref),
                finding_refs,
                finding_observation_refs: observation_refs,
            },
        )?;
        let audit = support::audit_intent(
            request,
            &aggregate,
            aggregate_version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;

        let mut records = Vec::with_capacity(outcomes.len() + finding_effects.len() + 2);
        records.push(RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: party_evaluation_job_persisted_payload(&completed_job)?,
        });
        for outcome in &outcomes {
            records.push(RecordMutation::Create {
                reference: party_rule_outcome_record_ref(outcome)?,
                payload: party_rule_outcome_persisted_payload(outcome)?,
            });
        }
        records.push(RecordMutation::Create {
            reference: party_completeness_result_record_ref(&completeness)?,
            payload: party_completeness_result_persisted_payload(&completeness)?,
        });
        records.extend(finding_effects);

        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records,
                relationships: Vec::new(),
                events: vec![event],
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![audit],
            },
            output: Some(output),
        })
    }
}

pub fn party_rule_outcome_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: RULE_OUTCOME_STATE_SCHEMA_ID,
        schema_version: RULE_OUTCOME_STATE_SCHEMA_VERSION,
        descriptor_hash: rule_outcome_state_descriptor_hash(),
        maximum_size_bytes: RULE_OUTCOME_STATE_MAXIMUM_BYTES,
        retention_policy_id: RULE_OUTCOME_STATE_RETENTION_POLICY_ID,
    }
}

pub fn party_rule_outcome_persisted_payload(
    outcome: &PartyRuleOutcome,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        party_rule_outcome_persisted_contract(),
        DataClass::Personal,
        encode_rule_outcome_state(outcome)?,
    )
}

pub fn party_rule_outcome_record_ref(
    outcome: &PartyRuleOutcome,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        RULE_OUTCOME_RECORD_TYPE,
        outcome.outcome_id(),
        "data_quality.rule_outcome_ref.rule_outcome_id",
    )
}

pub fn party_completeness_result_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PARTY_COMPLETENESS_RESULT_STATE_SCHEMA_ID,
        schema_version: PARTY_COMPLETENESS_RESULT_STATE_SCHEMA_VERSION,
        descriptor_hash: party_completeness_result_state_descriptor_hash(),
        maximum_size_bytes: PARTY_COMPLETENESS_RESULT_STATE_MAXIMUM_BYTES,
        retention_policy_id: PARTY_COMPLETENESS_RESULT_STATE_RETENTION_POLICY_ID,
    }
}

pub fn party_completeness_result_persisted_payload(
    result: &PartyCompletenessResult,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        party_completeness_result_persisted_contract(),
        DataClass::Personal,
        encode_party_completeness_result_state(result)?,
    )
}

pub fn party_completeness_result_record_ref(
    result: &PartyCompletenessResult,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        PARTY_COMPLETENESS_RESULT_RECORD_TYPE,
        result.result_id(),
        "data_quality.completeness_result_ref.completeness_result_id",
    )
}

pub fn party_finding_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: FINDING_STATE_SCHEMA_ID,
        schema_version: FINDING_STATE_SCHEMA_VERSION,
        descriptor_hash: finding_state_descriptor_hash(),
        maximum_size_bytes: FINDING_STATE_MAXIMUM_BYTES,
        retention_policy_id: FINDING_STATE_RETENTION_POLICY_ID,
    }
}

pub fn party_finding_persisted_payload(
    finding: &PartyFinding,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        party_finding_persisted_contract(),
        DataClass::Personal,
        encode_finding_state(finding)?,
    )
}

pub fn party_finding_record_ref(
    finding: &PartyFinding,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        FINDING_RECORD_TYPE,
        finding.finding_id(),
        "data_quality.finding_ref.finding_id",
    )
}

pub fn party_finding_observation_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: FINDING_OBSERVATION_STATE_SCHEMA_ID,
        schema_version: FINDING_OBSERVATION_STATE_SCHEMA_VERSION,
        descriptor_hash: finding_observation_state_descriptor_hash(),
        maximum_size_bytes: FINDING_OBSERVATION_STATE_MAXIMUM_BYTES,
        retention_policy_id: FINDING_OBSERVATION_STATE_RETENTION_POLICY_ID,
    }
}

pub fn party_finding_observation_persisted_payload(
    observation: &PartyFindingObservation,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        party_finding_observation_persisted_contract(),
        DataClass::Personal,
        encode_finding_observation_state(observation)?,
    )
}

pub fn party_finding_observation_record_ref(
    observation: &PartyFindingObservation,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        FINDING_OBSERVATION_RECORD_TYPE,
        observation.observation_id(),
        "data_quality.finding_observation_ref.finding_observation_id",
    )
}

fn decode_request(
    request: &CapabilityRequest,
) -> Result<wire::MaterializePartyEvaluationRequest, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        MATERIALIZE_PARTY_EVALUATION_REQUEST_SCHEMA,
        DataClass::Personal,
    )
}

fn job_record_ref(
    value: Option<wire::PartyEvaluationJobRef>,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    let job_id = RecordId::try_new(
        value
            .ok_or_else(|| missing("evaluation_job_ref"))?
            .evaluation_job_id,
    )
    .map_err(|error| {
        SdkError::invalid_argument(
            "data_quality.evaluation_job_ref.evaluation_job_id",
            error.to_string(),
        )
    })?;
    support::record_ref(
        crm_data_quality::PARTY_EVALUATION_JOB_RECORD_TYPE,
        job_id.as_str(),
        "data_quality.evaluation_job_ref.evaluation_job_id",
    )
}

fn missing(field: &'static str) -> SdkError {
    SdkError::invalid_argument(field, "The reference is required")
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != MATERIALIZE_PARTY_EVALUATION_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan(
            "capability definition does not match the request",
        ));
    }
    Ok(())
}

fn invalid_plan(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_MATERIALIZATION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party evaluation outcomes could not be materialized safely.",
    )
    .with_internal_reference(reference)
}
