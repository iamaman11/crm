from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    path.write_text(text.replace(old, new, 1))


persistence = Path(
    "crates/crm-customer-enrichment-provider-process-composition/src/conflict_persistence.rs"
)
replace_once(
    persistence,
    """use crm_core_data::{
    BatchError, BatchMutationPlan, BatchMutationResult, PostgresDataStore, RecordMutation,
};
""",
    """use crm_core_data::{
    BatchError, BatchMutationPlan, BatchMutationResult, PostgresDataStore, RecordMutation,
    RelatedRecordListQuery, RelationshipMutation,
};
""",
    "conflict persistence core-data imports",
)
replace_once(
    persistence,
    """use crm_customer_enrichment::{
    PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE, PROVIDER_RESPONSE_CONFLICT_STATE_MAXIMUM_BYTES,
    PROVIDER_RESPONSE_CONFLICT_STATE_RETENTION_POLICY_ID,
    PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_ID, PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_VERSION,
    ProviderResponseConflict, ProviderResponseConflictDraft,
    decode_provider_response_conflict_state, encode_provider_response_conflict_state,
    provider_response_conflict_state_descriptor_hash,
};
""",
    """use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
    PROVIDER_RESPONSE_CONFLICT_STATE_MAXIMUM_BYTES,
    PROVIDER_RESPONSE_CONFLICT_STATE_RETENTION_POLICY_ID,
    PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_ID, PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_VERSION,
    ProviderResponseConflict, ProviderResponseConflictDraft,
    decode_provider_response_conflict_state, encode_provider_response_conflict_state,
    provider_response_conflict_state_descriptor_hash,
};
""",
    "conflict persistence domain imports",
)
replace_once(
    persistence,
    """use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, ErrorCategory,
    ExecutionContext, IdempotencyKey, ModuleExecutionContext, RecordRef, RequestId, SchemaVersion,
    SdkError, TraceId, TypedPayload,
};
""",
    """use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, ErrorCategory,
    ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RecordId, RecordRef,
    RecordType, RelationshipRef, RelationshipType, RequestId, SchemaVersion, SdkError, TenantId,
    TraceId, TypedPayload,
};
""",
    "conflict persistence sdk imports",
)
replace_once(
    persistence,
    """pub const PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderResponseConflictRecordedEvent";

const CONFLICT_ID_PREFIX: &str = "enrichment-response-conflict-";
""",
    """pub const PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderResponseConflictRecordedEvent";
pub const PROVIDER_RESPONSE_CONFLICT_RELATIONSHIP_TYPE: &str =
    "customer_enrichment.request.provider_response_conflict";

const CONFLICT_ID_PREFIX: &str = "enrichment-response-conflict-";
const CONFLICT_RELATIONSHIP_SCHEMA_ID: &str =
    "crm.customer-enrichment.request.provider_response_conflict-link";
const CONFLICT_RELATIONSHIP_SCHEMA_VERSION: &str = "1.0.0";
const CONFLICT_RELATIONSHIP_MAXIMUM_BYTES: u64 = 1_024;
const CONFLICT_RELATIONSHIP_DESCRIPTOR_HASH: [u8; 32] = [
    89, 89, 224, 34, 91, 2, 35, 23, 37, 77, 136, 62, 69, 3, 50, 251, 156, 49, 254,
    55, 85, 173, 200, 194, 57, 250, 124, 1, 62, 34, 185, 183,
];
const MAX_CONFLICTS_PER_REQUEST: u32 = 16;
""",
    "conflict relationship contract",
)
replace_once(
    persistence,
    """pub struct ProviderResponseConflictPersistencePlan {
    pub conflict: ProviderResponseConflict,
    pub record: RecordRef,
    pub batch: BatchMutationPlan,
}
""",
    """pub struct ProviderResponseConflictPersistencePlan {
    pub conflict: ProviderResponseConflict,
    pub record: RecordRef,
    pub relationship: RelationshipRef,
    pub batch: BatchMutationPlan,
}
""",
    "conflict persistence plan relationship",
)
replace_once(
    persistence,
    """        Ok(ProviderResponseConflictPersistenceResult {
            conflict: plan.conflict,
            replayed: result.replayed,
        })
    }
}
""",
    """        Ok(ProviderResponseConflictPersistenceResult {
            conflict: plan.conflict,
            replayed: result.replayed,
        })
    }

    pub async fn unresolved_for_request(
        &self,
        tenant_id: TenantId,
        request_id: RecordId,
    ) -> Result<Option<ProviderResponseConflict>, SdkError> {
        let page = self
            .store
            .list_related_records_for_query(&RelatedRecordListQuery {
                tenant_id: tenant_id.clone(),
                relationship_owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
                relationship_type: configured(RelationshipType::try_new(
                    PROVIDER_RESPONSE_CONFLICT_RELATIONSHIP_TYPE,
                ))?,
                source: RecordRef {
                    record_type: configured(RecordType::try_new(ENRICHMENT_REQUEST_RECORD_TYPE))?,
                    record_id: request_id.clone(),
                },
                target_owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
                target_record_type: configured(RecordType::try_new(
                    PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
                ))?,
                page_size: MAX_CONFLICTS_PER_REQUEST,
                after_record_id: None,
            })
            .await?;
        if page.next_record_id.is_some() {
            return Err(conflict_state_invalid(
                "provider-response conflicts exceed the governed per-request bound",
            ));
        }

        let mut unresolved = None;
        for snapshot in page.records {
            let bytes = support::persisted_json_bytes_with_data_class(
                &snapshot,
                provider_response_conflict_persisted_contract(),
                DataClass::Confidential,
            )?;
            let conflict = decode_provider_response_conflict_state(bytes)?;
            if conflict.tenant_id() != &tenant_id
                || conflict.request_id().as_str() != request_id.as_str()
            {
                return Err(conflict_state_invalid(
                    "related provider-response conflict does not match its request identity",
                ));
            }
            if conflict.resolution().is_none() {
                if unresolved.replace(conflict).is_some() {
                    return Err(conflict_state_invalid(
                        "request has more than one unresolved provider-response conflict",
                    ));
                }
            }
        }
        Ok(unresolved)
    }
}
""",
    "unresolved conflict lookup",
)
replace_once(
    persistence,
    """    let record = provider_response_conflict_record_ref(&conflict)?;
    let state_bytes = encode_provider_response_conflict_state(&conflict)?;
""",
    """    let record = provider_response_conflict_record_ref(&conflict)?;
    let relationship = provider_response_conflict_relationship(&conflict, &record)?;
    let state_bytes = encode_provider_response_conflict_state(&conflict)?;
""",
    "conflict relationship planning",
)
replace_once(
    persistence,
    """        relationships: Vec::new(),
        events: vec![event],
""",
    """        relationships: vec![RelationshipMutation::Link {
            relationship: relationship.clone(),
            payload: provider_response_conflict_relationship_payload()?,
        }],
        events: vec![event],
""",
    "conflict relationship mutation",
)
replace_once(
    persistence,
    """    Ok(ProviderResponseConflictPersistencePlan {
        conflict,
        record,
        batch,
    })
}
""",
    """    Ok(ProviderResponseConflictPersistencePlan {
        conflict,
        record,
        relationship,
        batch,
    })
}
""",
    "conflict relationship plan result",
)
replace_once(
    persistence,
    """pub fn provider_response_conflict_to_wire(
    conflict: &ProviderResponseConflict,
) -> Result<wire::ProviderResponseConflict, SdkError> {
""",
    """pub fn provider_response_conflict_relationship(
    conflict: &ProviderResponseConflict,
    conflict_record: &RecordRef,
) -> Result<RelationshipRef, SdkError> {
    Ok(RelationshipRef {
        relationship_type: configured(RelationshipType::try_new(
            PROVIDER_RESPONSE_CONFLICT_RELATIONSHIP_TYPE,
        ))?,
        source: RecordRef {
            record_type: configured(RecordType::try_new(ENRICHMENT_REQUEST_RECORD_TYPE))?,
            record_id: configured(RecordId::try_new(conflict.request_id().as_str().to_owned()))?,
        },
        target: conflict_record.clone(),
    })
}

fn provider_response_conflict_relationship_payload() -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: CONFLICT_RELATIONSHIP_SCHEMA_ID,
            schema_version: CONFLICT_RELATIONSHIP_SCHEMA_VERSION,
            descriptor_hash: CONFLICT_RELATIONSHIP_DESCRIPTOR_HASH,
            maximum_size_bytes: CONFLICT_RELATIONSHIP_MAXIMUM_BYTES,
            retention_policy_id: PROVIDER_RESPONSE_CONFLICT_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Confidential,
        b"{}".to_vec(),
    )
}

pub fn provider_response_conflict_to_wire(
    conflict: &ProviderResponseConflict,
) -> Result<wire::ProviderResponseConflict, SdkError> {
""",
    "conflict relationship helpers",
)
replace_once(
    persistence,
    """    if snapshot.reference != plan.record || snapshot.version != 1 {
        return Err(conflict_plan_invalid(
            "conflict persistence returned the wrong record identity or version",
        ));
    }
""",
    """    if snapshot.reference != plan.record || snapshot.version != 1 {
        return Err(conflict_plan_invalid(
            "conflict persistence returned the wrong record identity or version",
        ));
    }
    if result.linked_relationships.as_slice() != [plan.relationship.clone()] {
        return Err(conflict_plan_invalid(
            "conflict persistence returned the wrong request relationship",
        ));
    }
""",
    "conflict relationship result validation",
)
replace_once(
    persistence,
    """fn conflict_plan_invalid(reference: impl Into<String>) -> SdkError {
""",
    """fn conflict_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored provider-response conflict evidence is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn conflict_plan_invalid(reference: impl Into<String>) -> SdkError {
""",
    "conflict state error",
)
replace_once(
    persistence,
    """        assert_eq!(plan.batch.records.len(), 1);
        assert_eq!(plan.batch.events.len(), 1);
""",
    """        assert_eq!(plan.batch.records.len(), 1);
        assert_eq!(plan.batch.relationships.len(), 1);
        assert_eq!(plan.batch.events.len(), 1);
""",
    "conflict planner relationship test",
)

worker = Path(
    "crates/crm-customer-enrichment-provider-process-composition/src/worker.rs"
)
replace_once(
    worker,
    """use crate::{ProviderDispatchWorkItemInput, build_provider_dispatch_work_item};
""",
    """use crate::{
    PostgresProviderResponseConflictStore, ProviderDispatchWorkItemInput,
    ProviderResponseConflictPersistenceLineage, build_provider_dispatch_work_item,
};
""",
    "provider process crate imports",
)
replace_once(
    worker,
    """use crm_customer_enrichment_worker_composition::{
    CustomerEnrichmentProviderWorker, ProviderDispatchWorkItem, ProviderDispatchWorkerResult,
};
""",
    """use crm_customer_enrichment_worker_composition::{
    CustomerEnrichmentProviderWorker, ProviderDispatchExecution, ProviderDispatchWorkItem,
    ProviderDispatchWorkerResult,
};
""",
    "provider process typed execution import",
)
replace_once(
    worker,
    """use crm_module_sdk::{
    ActorId, DataClass, EventDelivery, EventType, ModuleId, PayloadEncoding, PortFuture, RecordId,
    SdkError, TenantId,
};
""",
    """use crm_module_sdk::{
    ActorId, CausationId, DataClass, EventDelivery, EventType, ModuleId, PayloadEncoding, PortFuture,
    RecordId, SdkError, TenantId,
};
""",
    "provider process lineage imports",
)
replace_once(
    worker,
    """    ) -> PortFuture<'a, Result<ProviderDispatchWorkerResult, SdkError>>;
}
""",
    """    ) -> PortFuture<'a, Result<ProviderDispatchExecution, SdkError>>;
}
""",
    "provider executor typed result",
)
replace_once(
    worker,
    """    ) -> PortFuture<'a, Result<ProviderDispatchWorkerResult, SdkError>> {
        Box::pin(async move { CustomerEnrichmentProviderWorker::execute(self, work_item).await })
    }
}
""",
    """    ) -> PortFuture<'a, Result<ProviderDispatchExecution, SdkError>> {
        Box::pin(async move {
            CustomerEnrichmentProviderWorker::execute_reconciled(self, work_item).await
        })
    }
}
""",
    "provider executor reconciled implementation",
)
replace_once(
    worker,
    """    executor: Arc<dyn ProviderDispatchExecutorPort>,
    actor_id: ActorId,
""",
    """    executor: Arc<dyn ProviderDispatchExecutorPort>,
    conflict_store: PostgresProviderResponseConflictStore,
    actor_id: ActorId,
""",
    "provider process conflict store field",
)
replace_once(
    worker,
    """            .field("executor", &"dyn ProviderDispatchExecutorPort")
            .field("actor_id", &self.actor_id)
""",
    """            .field("executor", &"dyn ProviderDispatchExecutorPort")
            .field("conflict_store", &self.conflict_store)
            .field("actor_id", &self.actor_id)
""",
    "provider process conflict store debug",
)
replace_once(
    worker,
    """        Ok(Self {
            store,
            source,
            executor,
            actor_id,
            page_size,
        })
""",
    """        Ok(Self {
            conflict_store: PostgresProviderResponseConflictStore::new(store.clone()),
            store,
            source,
            executor,
            actor_id,
            page_size,
        })
""",
    "provider process conflict store construction",
)
replace_once(
    worker,
    """        let request_id = RecordId::try_new(request_ref.enrichment_request_id.clone())
            .map_err(worker_identifier_invalid)?;
        let source = self
""",
    """        let request_id = RecordId::try_new(request_ref.enrichment_request_id.clone())
            .map_err(worker_identifier_invalid)?;
        if let Some(conflict) = self
            .conflict_store
            .unresolved_for_request(tenant_id.clone(), request_id.clone())
            .await?
        {
            return Err(unresolved_provider_conflict(
                conflict.conflict_id().as_str(),
            ));
        }
        let source = self
""",
    "provider process unresolved precheck",
)
replace_once(
    worker,
    """        let result = self.executor.execute(work_item).await?;
        Ok(DeliveryDisposition::Executed(Box::new(result)))
""",
    """        match self.executor.execute(work_item).await? {
            ProviderDispatchExecution::Recorded(result) => {
                Ok(DeliveryDisposition::Executed(result))
            }
            ProviderDispatchExecution::Conflicting(draft) => {
                let persisted = self
                    .conflict_store
                    .record(
                        draft,
                        ProviderResponseConflictPersistenceLineage {
                            actor_id: self.actor_id.clone(),
                            correlation_id: delivery.correlation_id.clone(),
                            causation_id: CausationId::try_new(delivery.event_id.as_str().to_owned())
                                .map_err(worker_identifier_invalid)?,
                            trace_id: delivery.trace_id.clone(),
                        },
                    )
                    .await?;
                Err(unresolved_provider_conflict(
                    persisted.conflict.conflict_id().as_str(),
                ))
            }
        }
""",
    "provider process conflict persistence",
)
replace_once(
    worker,
    """fn source_snapshot_invalid() -> SdkError {
""",
    """fn unresolved_provider_conflict(conflict_id: &str) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_UNRESOLVED",
        crm_module_sdk::ErrorCategory::Conflict,
        true,
        "Provider-response conflict resolution is required before processing can continue.",
    )
    .with_internal_reference(format!("conflict_id={conflict_id}"))
}

fn source_snapshot_invalid() -> SdkError {
""",
    "provider process unresolved error",
)
replace_once(
    worker,
    """    fn worker_clock_requires_a_positive_representable_millisecond() {
""",
    """    fn unresolved_provider_conflict_is_a_retryable_checkpoint_hold() {
        let error = unresolved_provider_conflict("enrichment-response-conflict-example");
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_UNRESOLVED"
        );
        assert!(error.retryable);
    }

    #[test]
    fn worker_clock_requires_a_positive_representable_millisecond() {
""",
    "provider process unresolved error test",
)

postgres_test = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_persistence.rs"
)
replace_once(
    postgres_test,
    """use crm_module_sdk::{ActorId, CausationId, CorrelationId, TenantId, TraceId};
""",
    """use crm_module_sdk::{ActorId, CausationId, CorrelationId, RecordId, TenantId, TraceId};
""",
    "postgres conflict lookup import",
)
replace_once(
    postgres_test,
    """    assert_eq!(first.conflict, replay.conflict);
""",
    """    assert_eq!(first.conflict, replay.conflict);
    let unresolved = persistence
        .unresolved_for_request(
            TenantId::try_new(TENANT_ID).unwrap(),
            RecordId::try_new(first.conflict.request_id().as_str().to_owned()).unwrap(),
        )
        .await
        .expect("load unresolved provider-response conflict")
        .expect("unresolved provider-response conflict exists");
    assert_eq!(unresolved, first.conflict);
""",
    "postgres unresolved lookup assertion",
)
replace_once(
    postgres_test,
    """    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND event_type = 'customer_enrichment.provider_response_conflict.recorded'",
        )
        .await,
        1
    );
""",
    """    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.relationships WHERE tenant_id = 'tenant-a' AND relationship_type = 'customer_enrichment.request.provider_response_conflict'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND event_type = 'customer_enrichment.provider_response_conflict.recorded'",
        )
        .await,
        1
    );
""",
    "postgres relationship assertion",
)

cargo = Path(
    "crates/crm-customer-enrichment-provider-process-composition/Cargo.toml"
)
replace_once(
    cargo,
    """[[test]]
name = "postgres_conflict_persistence"
path = "tests/postgres_conflict_persistence.rs"
required-features = ["postgres-integration"]
""",
    """[[test]]
name = "postgres_conflict_persistence"
path = "tests/postgres_conflict_persistence.rs"
required-features = ["postgres-integration"]

[[test]]
name = "postgres_conflict_process_hold"
path = "tests/postgres_conflict_process_hold.rs"
required-features = ["postgres-integration"]
""",
    "provider process PostgreSQL test target",
)

print("applied provider conflict process hold patch")
