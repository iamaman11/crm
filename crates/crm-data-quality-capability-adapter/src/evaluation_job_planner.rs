use crate::{
    MODULE_ID, PARTY_EVALUATION_REQUESTED_EVENT_SCHEMA, PARTY_EVALUATION_REQUESTED_EVENT_TYPE,
    REQUEST_PARTY_EVALUATION_CAPABILITY, REQUEST_PARTY_EVALUATION_REQUEST_SCHEMA,
    REQUEST_PARTY_EVALUATION_RESPONSE_SCHEMA,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_data_quality::{
    PARTY_EVALUATION_JOB_RECORD_TYPE, PARTY_EVALUATION_JOB_STATE_MAXIMUM_BYTES,
    PARTY_EVALUATION_JOB_STATE_RETENTION_POLICY_ID, PARTY_EVALUATION_JOB_STATE_SCHEMA_ID,
    PARTY_EVALUATION_JOB_STATE_SCHEMA_VERSION, PartyCompletenessProfileVersion,
    PartyEvaluationJob, PartyEvaluationJobStatus, PartyRuleSetVersion,
    decode_party_evaluation_job_state, encode_party_evaluation_job_state,
    party_evaluation_job_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordId, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::{core::v1 as core, customer::v1 as customer, data_quality::v1 as wire};

#[derive(Debug, Clone)]
pub struct EvaluationReferenceScope {
    pub job_id: RecordId,
    pub party_id: RecordId,
    pub rule_set_version_id: RecordId,
    pub profile_version_id: RecordId,
}

#[derive(Debug, Clone)]
pub struct DataQualityEvaluationJobCapabilityPlanner {
    rule_set: PartyRuleSetVersion,
    profile: PartyCompletenessProfileVersion,
}

impl DataQualityEvaluationJobCapabilityPlanner {
    pub fn new(
        rule_set: PartyRuleSetVersion,
        profile: PartyCompletenessProfileVersion,
    ) -> Result<Self, SdkError> {
        if profile.rule_set_version_id() != rule_set.version_id() {
            return Err(invalid_plan("profile and rule-set bindings differ"));
        }
        Ok(Self { rule_set, profile })
    }

    fn job_from_request(&self, request: &CapabilityRequest) -> Result<PartyEvaluationJob, SdkError> {
        let scope = evaluation_reference_scope_from_request(request)?;
        if scope.rule_set_version_id.as_str() != self.rule_set.version_id().as_str()
            || scope.profile_version_id.as_str() != self.profile.version_id().as_str()
        {
            return Err(invalid_plan("request references differ from validated definitions"));
        }
        PartyEvaluationJob::create(
            scope.job_id,
            scope.party_id,
            &self.rule_set,
            &self.profile,
            request.context.execution.request_started_at_unix_nanos,
        )
    }
}

impl TransactionalAggregatePlanner for DataQualityEvaluationJobCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let job = self.job_from_request(request)?;
        Ok(AggregateTarget {
            reference: job_record_ref(&job)?,
            presence: AggregatePresence::MustBeAbsent,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        if current.is_some() {
            return Err(invalid_plan("evaluation job already exists"));
        }
        let job = self.job_from_request(request)?;
        let aggregate = job_record_ref(&job)?;
        let public_job = party_evaluation_job_to_wire(&job, 1);
        let output = support::protobuf_payload(
            MODULE_ID,
            REQUEST_PARTY_EVALUATION_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::RequestPartyEvaluationResponse {
                evaluation_job: Some(public_job.clone()),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PARTY_EVALUATION_REQUESTED_EVENT_TYPE,
                event_schema_id: PARTY_EVALUATION_REQUESTED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Personal,
            &wire::PartyEvaluationRequestedEvent {
                evaluation_job: Some(public_job),
            },
        )?;
        let audit = support::audit_intent(
            request,
            &aggregate,
            1,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Create {
                    reference: aggregate,
                    payload: party_evaluation_job_persisted_payload(&job)?,
                }],
                relationships: Vec::new(),
                events: vec![event],
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![audit],
            },
            output: Some(output),
        })
    }
}

pub fn evaluation_reference_scope_from_request(
    request: &CapabilityRequest,
) -> Result<EvaluationReferenceScope, SdkError> {
    let command: wire::RequestPartyEvaluationRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        REQUEST_PARTY_EVALUATION_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    Ok(EvaluationReferenceScope {
        job_id: record_id(
            command
                .evaluation_job_ref
                .ok_or_else(|| missing("evaluation_job_ref"))?
                .evaluation_job_id,
            "data_quality.evaluation_job_ref.evaluation_job_id",
        )?,
        party_id: record_id(
            command
                .party_ref
                .ok_or_else(|| missing("party_ref"))?
                .party_id,
            "data_quality.party_ref.party_id",
        )?,
        rule_set_version_id: record_id(
            command
                .rule_set_version_ref
                .ok_or_else(|| missing("rule_set_version_ref"))?
                .rule_set_version_id,
            "data_quality.rule_set_version_ref.rule_set_version_id",
        )?,
        profile_version_id: record_id(
            command
                .completeness_profile_version_ref
                .ok_or_else(|| missing("completeness_profile_version_ref"))?
                .completeness_profile_version_id,
            "data_quality.completeness_profile_version_ref.completeness_profile_version_id",
        )?,
    })
}

pub fn party_evaluation_job_to_wire(job: &PartyEvaluationJob, resource_version: i64) -> wire::PartyEvaluationJob {
    wire::PartyEvaluationJob {
        evaluation_job_ref: Some(wire::PartyEvaluationJobRef {
            evaluation_job_id: job.job_id().as_str().to_owned(),
        }),
        party_ref: Some(customer::PartyRef {
            party_id: job.party_id().as_str().to_owned(),
        }),
        rule_set_version_ref: Some(wire::PartyRuleSetVersionRef {
            rule_set_version_id: job.rule_set_version_id().to_owned(),
        }),
        completeness_profile_version_ref: Some(wire::PartyCompletenessProfileVersionRef {
            completeness_profile_version_id: job.profile_version_id().to_owned(),
        }),
        status: match job.status() {
            PartyEvaluationJobStatus::Created => wire::PartyEvaluationJobStatus::Created as i32,
            PartyEvaluationJobStatus::Staged => wire::PartyEvaluationJobStatus::Staged as i32,
            PartyEvaluationJobStatus::Completed => wire::PartyEvaluationJobStatus::Completed as i32,
        },
        evaluated_party_resource_version: job.party_resource_version().map(|version| {
            customer::CustomerResourceVersion {
                version,
                created_at: None,
                updated_at: None,
            }
        }),
        evaluated_rules: job.evaluated_rules(),
        failed_rules: job.failed_rules(),
        created_at: Some(core::UnixTime {
            unix_nanos: job.created_at(),
        }),
        updated_at: Some(core::UnixTime {
            unix_nanos: job.updated_at(),
        }),
        resource_version: Some(customer::CustomerResourceVersion {
            version: resource_version,
            created_at: Some(core::UnixTime {
                unix_nanos: job.created_at(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: job.updated_at(),
            }),
        }),
    }
}

pub fn party_evaluation_job_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PARTY_EVALUATION_JOB_STATE_SCHEMA_ID,
        schema_version: PARTY_EVALUATION_JOB_STATE_SCHEMA_VERSION,
        descriptor_hash: party_evaluation_job_state_descriptor_hash(),
        maximum_size_bytes: PARTY_EVALUATION_JOB_STATE_MAXIMUM_BYTES,
        retention_policy_id: PARTY_EVALUATION_JOB_STATE_RETENTION_POLICY_ID,
    }
}

pub fn party_evaluation_job_persisted_payload(
    job: &PartyEvaluationJob,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        party_evaluation_job_persisted_contract(),
        DataClass::Personal,
        encode_party_evaluation_job_state(job)?,
    )
}

pub fn party_evaluation_job_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<PartyEvaluationJob, SdkError> {
    if snapshot.reference.record_type.as_str() != PARTY_EVALUATION_JOB_RECORD_TYPE {
        return Err(invalid_plan("record type is not an evaluation job"));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        party_evaluation_job_persisted_contract(),
        DataClass::Personal,
    )?;
    let job = decode_party_evaluation_job_state(bytes)?;
    if job.job_id().as_str() != snapshot.reference.record_id.as_str() {
        return Err(invalid_plan("persisted evaluation job identity differs from its record"));
    }
    Ok(job)
}

fn job_record_ref(job: &PartyEvaluationJob) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        PARTY_EVALUATION_JOB_RECORD_TYPE,
        job.job_id().as_str(),
        "data_quality.evaluation_job_ref.evaluation_job_id",
    )
}

fn record_id(value: String, field: &'static str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value).map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn missing(field: &'static str) -> SdkError {
    SdkError::invalid_argument(field, "The reference is required")
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != REQUEST_PARTY_EVALUATION_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan("capability definition does not match the request"));
    }
    Ok(())
}

fn invalid_plan(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_CAPABILITY_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party evaluation request could not be planned safely.",
    )
    .with_internal_reference(reference)
}
