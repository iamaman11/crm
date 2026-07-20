use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{
    BatchError, BatchMutationPlan, BatchMutationResult, PostgresDataStore, RecordMutation,
};
use crm_customer_enrichment::{
    PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE, PROVIDER_RESPONSE_CONFLICT_STATE_MAXIMUM_BYTES,
    PROVIDER_RESPONSE_CONFLICT_STATE_RETENTION_POLICY_ID,
    PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_ID, PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_VERSION,
    ProviderResponseConflict, ProviderResponseConflictDraft,
    decode_provider_response_conflict_state, encode_provider_response_conflict_state,
    provider_response_conflict_state_descriptor_hash,
};
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, RECORD_PROVIDER_RESPONSE_CAPABILITY, provider_response_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, ErrorCategory,
    ExecutionContext, IdempotencyKey, ModuleExecutionContext, RecordRef, RequestId, SchemaVersion,
    SdkError, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;

pub const PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE: &str =
    "customer_enrichment.provider_response_conflict.recorded";
pub const PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderResponseConflictRecordedEvent";

const CONFLICT_ID_PREFIX: &str = "enrichment-response-conflict-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictPersistenceLineage {
    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub causation_id: CausationId,
    pub trace_id: TraceId,
}

pub struct ProviderResponseConflictPersistencePlan {
    pub conflict: ProviderResponseConflict,
    pub record: RecordRef,
    pub batch: BatchMutationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictPersistenceResult {
    pub conflict: ProviderResponseConflict,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct PostgresProviderResponseConflictStore {
    store: PostgresDataStore,
}

impl PostgresProviderResponseConflictStore {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    pub async fn record(
        &self,
        draft: ProviderResponseConflictDraft,
        lineage: ProviderResponseConflictPersistenceLineage,
    ) -> Result<ProviderResponseConflictPersistenceResult, SdkError> {
        let plan = provider_response_conflict_persistence_plan(draft, lineage)?;
        let result = self
            .store
            .execute_batch(&plan.batch)
            .await
            .map_err(conflict_batch_error)?;
        validate_batch_result(&plan, &result)?;
        Ok(ProviderResponseConflictPersistenceResult {
            conflict: plan.conflict,
            replayed: result.replayed,
        })
    }
}

pub fn provider_response_conflict_persistence_plan(
    draft: ProviderResponseConflictDraft,
    lineage: ProviderResponseConflictPersistenceLineage,
) -> Result<ProviderResponseConflictPersistencePlan, SdkError> {
    let conflict = ProviderResponseConflict::record(draft)?;
    let record = provider_response_conflict_record_ref(&conflict)?;
    let state_bytes = encode_provider_response_conflict_state(&conflict)?;
    let input = provider_response_conflict_persisted_payload(&conflict)?;
    let definition = provider_response_capability_definition()?;
    let suffix = conflict_suffix(&conflict)?;
    let started_at_unix_nanos = conflict
        .detected_at_unix_ms()
        .checked_mul(1_000_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| conflict_plan_invalid("conflict detection time exceeds execution range"))?;
    let request = CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: definition.owner_module_id.clone(),
            execution: ExecutionContext {
                tenant_id: conflict.tenant_id().clone(),
                actor_id: lineage.actor_id,
                request_id: configured(RequestId::try_new(format!(
                    "enrichment-conflict-record-{suffix}"
                )))?,
                correlation_id: lineage.correlation_id,
                causation_id: lineage.causation_id,
                trace_id: lineage.trace_id,
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                idempotency_key: configured(IdempotencyKey::try_new(format!(
                    "enrichment-conflict-{suffix}"
                )))?,
                business_transaction_id: configured(BusinessTransactionId::try_new(format!(
                    "enrichment-conflict-tx-{suffix}"
                )))?,
                schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
                request_started_at_unix_nanos: started_at_unix_nanos,
            },
        },
        input_hash: semantic_input_hash(&input),
        input,
        approval: None,
    };
    let event = support::event_evidence_with_data_class(
        &request,
        record.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE,
            event_schema_id: PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_SCHEMA,
            aggregate_version: 1,
            previous_version: None,
        },
        DataClass::Confidential,
        &wire::ProviderResponseConflictRecordedEvent {
            provider_response_conflict: Some(provider_response_conflict_to_wire(&conflict)?),
        },
    )?;
    let audit = support::audit_intent(
        &request,
        &record,
        1,
        RECORD_PROVIDER_RESPONSE_CAPABILITY,
        &state_bytes,
    )?;
    let batch = BatchMutationPlan {
        context: request.context.clone(),
        records: vec![RecordMutation::Create {
            reference: record.clone(),
            payload: provider_response_conflict_persisted_payload(&conflict)?,
        }],
        relationships: Vec::new(),
        events: vec![event],
        idempotency: support::capability_idempotency(&definition, &request)?,
        audits: vec![audit],
    };
    batch.validate().map_err(conflict_batch_error)?;
    Ok(ProviderResponseConflictPersistencePlan {
        conflict,
        record,
        batch,
    })
}

pub fn provider_response_conflict_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_ID,
        schema_version: PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_VERSION,
        descriptor_hash: provider_response_conflict_state_descriptor_hash(),
        maximum_size_bytes: PROVIDER_RESPONSE_CONFLICT_STATE_MAXIMUM_BYTES,
        retention_policy_id: PROVIDER_RESPONSE_CONFLICT_STATE_RETENTION_POLICY_ID,
    }
}

pub fn provider_response_conflict_persisted_payload(
    conflict: &ProviderResponseConflict,
) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        provider_response_conflict_persisted_contract(),
        DataClass::Confidential,
        encode_provider_response_conflict_state(conflict)?,
    )
}

pub fn provider_response_conflict_record_ref(
    conflict: &ProviderResponseConflict,
) -> Result<RecordRef, SdkError> {
    support::record_ref(
        PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
        conflict.conflict_id().as_str(),
        "customer_enrichment.provider_response_conflict_ref.provider_response_conflict_id",
    )
}

pub fn provider_response_conflict_to_wire(
    conflict: &ProviderResponseConflict,
) -> Result<wire::ProviderResponseConflict, SdkError> {
    Ok(wire::ProviderResponseConflict {
        provider_response_conflict_ref: Some(wire::ProviderResponseConflictRef {
            provider_response_conflict_id: conflict.conflict_id().as_str().to_owned(),
        }),
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: conflict.request_id().as_str().to_owned(),
        }),
        retry_generation: conflict.retry_generation(),
        first_provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
            provider_response_receipt_id: conflict.first_receipt_id().as_str().to_owned(),
        }),
        conflicting_semantic_fingerprint: conflict.conflicting_semantic_fingerprint().to_vec(),
        detected_at_unix_ms: i64::try_from(conflict.detected_at_unix_ms())
            .map_err(|_| conflict_plan_invalid("conflict detection time exceeds wire range"))?,
    })
}

fn validate_batch_result(
    plan: &ProviderResponseConflictPersistencePlan,
    result: &BatchMutationResult,
) -> Result<(), SdkError> {
    let [snapshot] = result.records.as_slice() else {
        return Err(conflict_plan_invalid(
            "conflict persistence returned an unexpected record set",
        ));
    };
    if snapshot.reference != plan.record || snapshot.version != 1 {
        return Err(conflict_plan_invalid(
            "conflict persistence returned the wrong record identity or version",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        provider_response_conflict_persisted_contract(),
        DataClass::Confidential,
    )?;
    if decode_provider_response_conflict_state(bytes)? != plan.conflict {
        return Err(conflict_plan_invalid(
            "persisted conflict does not match the planned immutable evidence",
        ));
    }
    Ok(())
}

fn conflict_suffix(conflict: &ProviderResponseConflict) -> Result<&str, SdkError> {
    conflict
        .conflict_id()
        .as_str()
        .strip_prefix(CONFLICT_ID_PREFIX)
        .filter(|suffix| suffix.len() == 64)
        .ok_or_else(|| conflict_plan_invalid("conflict identity is not canonical"))
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| conflict_plan_invalid(error.to_string()))
}

fn conflict_batch_error(error: BatchError) -> SdkError {
    match error {
        BatchError::Sdk(error) => error,
        BatchError::IdempotencyKeyReused => SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_IDEMPOTENCY_REUSED",
            ErrorCategory::Conflict,
            false,
            "The provider-response conflict identity was reused for different evidence.",
        ),
        BatchError::IdempotencyInProgress => SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_IN_PROGRESS",
            ErrorCategory::Conflict,
            true,
            "The provider-response conflict is already being recorded.",
        ),
        BatchError::Conflict(reference) => SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_PERSISTENCE_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The provider-response conflict could not be recorded atomically.",
        )
        .with_internal_reference(reference),
        BatchError::Database(error) => SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_STORAGE_UNAVAILABLE",
            ErrorCategory::Internal,
            true,
            "The provider-response conflict could not be recorded.",
        )
        .with_internal_reference(error.to_string()),
        BatchError::InvalidPlan(reference) | BatchError::InvalidStoredValue(reference) => {
            conflict_plan_invalid(reference)
        }
    }
}

fn conflict_plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider-response conflict could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_enrichment::{EnrichmentRequestId, ProviderResponseReceiptId};
    use crm_module_sdk::{ModuleId, PayloadEncoding};
    use prost::Message;

    fn request_id(byte: u8) -> EnrichmentRequestId {
        serde_json::from_str(&format!(
            "\"enrichment-request-{}\"",
            format!("{byte:02x}").repeat(32)
        ))
        .unwrap()
    }

    fn receipt_id(byte: u8) -> ProviderResponseReceiptId {
        serde_json::from_str(&format!(
            "\"enrichment-response-{}\"",
            format!("{byte:02x}").repeat(32)
        ))
        .unwrap()
    }

    fn draft() -> ProviderResponseConflictDraft {
        ProviderResponseConflictDraft {
            tenant_id: crm_module_sdk::TenantId::try_new("tenant-a").unwrap(),
            request_id: request_id(1),
            retry_generation: 2,
            first_receipt_id: receipt_id(2),
            conflicting_semantic_fingerprint: [3; 32],
            detected_at_unix_ms: 50,
        }
    }

    fn lineage() -> ProviderResponseConflictPersistenceLineage {
        ProviderResponseConflictPersistenceLineage {
            actor_id: ActorId::try_new("provider-worker").unwrap(),
            correlation_id: CorrelationId::try_new("provider-correlation").unwrap(),
            causation_id: CausationId::try_new("provider-created-event").unwrap(),
            trace_id: TraceId::try_new("provider-trace").unwrap(),
        }
    }

    #[test]
    fn plan_contains_one_exact_record_event_audit_and_idempotency_transaction() {
        let plan = provider_response_conflict_persistence_plan(draft(), lineage()).unwrap();
        plan.batch.validate().unwrap();
        assert_eq!(plan.batch.records.len(), 1);
        assert_eq!(plan.batch.events.len(), 1);
        assert_eq!(plan.batch.audits.len(), 1);
        assert_eq!(plan.batch.relationships.len(), 0);
        assert_eq!(
            plan.batch.context.module_id,
            ModuleId::try_new(MODULE_ID).unwrap()
        );
        assert_eq!(
            plan.batch.context.execution.capability_id.as_str(),
            RECORD_PROVIDER_RESPONSE_CAPABILITY
        );
        assert_eq!(
            plan.batch.idempotency.scope,
            "capability:customer_enrichment.response.record:1.0.0"
        );
        assert_eq!(
            plan.batch.idempotency.key,
            plan.batch.context.execution.idempotency_key.as_str()
        );
        assert_eq!(
            plan.batch.events[0].event.event_type.as_str(),
            PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE
        );
        assert_eq!(
            plan.batch.events[0].event.payload.encoding,
            PayloadEncoding::Protobuf
        );
        assert_eq!(
            plan.batch.events[0].event.payload.data_class,
            DataClass::Confidential
        );
        let event = wire::ProviderResponseConflictRecordedEvent::decode(
            plan.batch.events[0].event.payload.bytes.as_slice(),
        )
        .unwrap();
        let conflict = event.provider_response_conflict.unwrap();
        assert_eq!(
            conflict
                .provider_response_conflict_ref
                .unwrap()
                .provider_response_conflict_id,
            plan.conflict.conflict_id().as_str()
        );
        assert_eq!(
            conflict
                .first_provider_response_receipt_ref
                .unwrap()
                .provider_response_receipt_id,
            plan.conflict.first_receipt_id().as_str()
        );
        assert_eq!(conflict.conflicting_semantic_fingerprint, vec![3; 32]);
    }

    #[test]
    fn exact_replay_has_identical_persistence_identity_and_changed_fingerprint_does_not() {
        let first = provider_response_conflict_persistence_plan(draft(), lineage()).unwrap();
        let replay = provider_response_conflict_persistence_plan(draft(), lineage()).unwrap();
        assert_eq!(first.conflict, replay.conflict);
        assert_eq!(first.record, replay.record);
        assert_eq!(first.batch.idempotency, replay.batch.idempotency);
        assert_eq!(
            first.batch.context.execution.business_transaction_id,
            replay.batch.context.execution.business_transaction_id
        );
        assert_eq!(first.batch.events[0], replay.batch.events[0]);
        assert_eq!(first.batch.audits[0], replay.batch.audits[0]);

        let mut changed = draft();
        changed.conflicting_semantic_fingerprint = [4; 32];
        let changed = provider_response_conflict_persistence_plan(changed, lineage()).unwrap();
        assert_ne!(first.conflict.conflict_id(), changed.conflict.conflict_id());
        assert_ne!(first.record, changed.record);
        assert_ne!(first.batch.idempotency.key, changed.batch.idempotency.key);
    }

    #[test]
    fn persisted_payload_is_strict_confidential_conflict_state() {
        let plan = provider_response_conflict_persistence_plan(draft(), lineage()).unwrap();
        let RecordMutation::Create { payload, .. } = &plan.batch.records[0] else {
            panic!("conflict persistence must create an immutable record");
        };
        assert_eq!(payload.data_class, DataClass::Confidential);
        assert_eq!(payload.encoding, PayloadEncoding::Json);
        assert_eq!(
            payload.schema_id.as_str(),
            PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_ID
        );
        assert_eq!(
            decode_provider_response_conflict_state(&payload.bytes).unwrap(),
            plan.conflict
        );
    }
}
