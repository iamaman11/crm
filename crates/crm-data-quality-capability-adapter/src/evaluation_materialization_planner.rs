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
    PARTY_COMPLETENESS_RESULT_RECORD_TYPE, PARTY_COMPLETENESS_RESULT_STATE_MAXIMUM_BYTES,
    PARTY_COMPLETENESS_RESULT_STATE_RETENTION_POLICY_ID, PARTY_COMPLETENESS_RESULT_STATE_SCHEMA_ID,
    PARTY_COMPLETENESS_RESULT_STATE_SCHEMA_VERSION, PartyCompletenessProfileVersion,
    PartyCompletenessResult, PartyEvaluationInputSnapshot, PartyEvaluationJobStatus,
    PartyQualityInput, PartyRuleOutcome, PartyRuleSetVersion, RULE_OUTCOME_RECORD_TYPE,
    RULE_OUTCOME_STATE_MAXIMUM_BYTES, RULE_OUTCOME_STATE_RETENTION_POLICY_ID,
    RULE_OUTCOME_STATE_SCHEMA_ID, RULE_OUTCOME_STATE_SCHEMA_VERSION,
    encode_party_completeness_result_state, encode_rule_outcome_state,
    party_completeness_result_state_descriptor_hash, rule_outcome_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordId, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::data_quality::v1 as wire;

#[derive(Debug, Clone)]
pub struct DataQualityEvaluationMaterializationPlanner {
    rule_set: PartyRuleSetVersion,
    profile: PartyCompletenessProfileVersion,
    input: PartyEvaluationInputSnapshot,
}

impl DataQualityEvaluationMaterializationPlanner {
    pub fn new(
        rule_set: PartyRuleSetVersion,
        profile: PartyCompletenessProfileVersion,
        input: PartyEvaluationInputSnapshot,
    ) -> Result<Self, SdkError> {
        if profile.rule_set_version_id() != rule_set.version_id() {
            return Err(invalid_plan("profile and rule-set bindings differ"));
        }
        Ok(Self {
            rule_set,
            profile,
            input,
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
                "only an unmaterialized staged evaluation job can produce durable outcomes",
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
        let public_job = party_evaluation_job_to_wire(&materialized_job, aggregate_version);
        let output = support::protobuf_payload(
            MODULE_ID,
            MATERIALIZE_PARTY_EVALUATION_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::MaterializePartyEvaluationResponse {
                evaluation_job: Some(public_job.clone()),
                rule_outcome_refs: outcome_refs.clone(),
                completeness_result_ref: Some(completeness_ref.clone()),
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
            },
        )?;
        let audit = support::audit_intent(
            request,
            &aggregate,
            aggregate_version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;

        let mut records = Vec::with_capacity(outcomes.len() + 2);
        records.push(RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: party_evaluation_job_persisted_payload(&materialized_job)?,
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
