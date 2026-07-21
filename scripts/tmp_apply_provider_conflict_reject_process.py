from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    if old not in text:
        raise SystemExit(f"marker not found in {path}: {old[:180]!r}")
    path.write_text(text.replace(old, new, 1))


lib = Path("crates/crm-customer-enrichment-provider-process-composition/src/lib.rs")
replace_once(
    lib,
    "mod conflict_persistence;\nmod conflict_resolution;\nmod worker;\n\npub use conflict_persistence::*;\npub use conflict_resolution::*;\npub use worker::*;",
    "mod conflict_persistence;\nmod conflict_rejection;\nmod conflict_resolution;\nmod worker;\n\npub use conflict_persistence::*;\npub use conflict_rejection::*;\npub use conflict_resolution::*;\npub use worker::*;",
)

rejection = r'''use crate::provider_response_conflict_persisted_payload;
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{
    BatchError, BatchMutationPlan, BatchMutationResult, DataError, PostgresDataStore,
    RecordMutation,
};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, EnrichmentRequestStatus,
    ProviderResponseConflict, ProviderResponseConflictDecision,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA, ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
    MODULE_ID, RECORD_PROVIDER_RESPONSE_CAPABILITY, enrichment_request_from_snapshot,
    enrichment_request_persisted_payload, enrichment_request_to_wire,
    provider_response_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CorrelationId, DataClass, ErrorCategory, ExecutionContext,
    IdempotencyKey, ModuleExecutionContext, RecordId, RequestId, SchemaVersion, SdkError, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use std::fmt;

const CONFLICT_ID_PREFIX: &str = "enrichment-response-conflict-";

/// Stable process lineage used for the terminal request transition after an approved rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictRejectionLineage {
    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictRejectionResult {
    pub request: EnrichmentRequest,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct PostgresProviderResponseConflictRejectExecutor {
    store: PostgresDataStore,
}

impl PostgresProviderResponseConflictRejectExecutor {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    pub async fn execute(
        &self,
        conflict: &ProviderResponseConflict,
        lineage: ProviderResponseConflictRejectionLineage,
    ) -> Result<ProviderResponseConflictRejectionResult, SdkError> {
        let resolution = conflict
            .resolution()
            .ok_or_else(rejection_resolution_missing)?;
        if resolution.decision() != ProviderResponseConflictDecision::RejectRequest {
            return Err(rejection_resolution_invalid(
                "provider-response conflict resolution is not reject-request",
            ));
        }
        let suffix = conflict_suffix(conflict.conflict_id().as_str())?;
        let definition = provider_response_capability_definition()?;
        let input = provider_response_conflict_persisted_payload(conflict)?;
        let request = CapabilityRequest {
            context: rejection_context(conflict, &lineage, suffix)?,
            input_hash: semantic_input_hash(&input),
            input,
            approval: None,
        };
        let record = support::record_ref(
            ENRICHMENT_REQUEST_RECORD_TYPE,
            conflict.request_id().as_str(),
            "customer_enrichment.provider_response_conflict.request_id",
        )?;
        let snapshot = self
            .store
            .get_record(&request.context, &record)
            .await
            .map_err(rejection_read_error)?
            .ok_or_else(rejection_request_not_found)?;
        let mut enrichment_request = enrichment_request_from_snapshot(&snapshot)?;
        if enrichment_request.tenant_id() != conflict.tenant_id()
            || enrichment_request.request_id() != conflict.request_id()
            || enrichment_request.retry_generation() != conflict.retry_generation()
        {
            return Err(rejection_state_invalid(
                "request identity or retry generation differs from immutable conflict evidence",
            ));
        }
        if enrichment_request
            .response_receipt_id()
            .is_some_and(|receipt| receipt != conflict.first_receipt_id())
        {
            return Err(rejection_state_invalid(
                "request is bound to a different provider-response receipt",
            ));
        }

        let (expected_version, aggregate_version) =
            if enrichment_request.status() == EnrichmentRequestStatus::FailedTerminal {
                if enrichment_request.last_safe_failure_code() != Some(resolution.safe_reason_code())
                    || snapshot.version <= 1
                {
                    return Err(rejection_state_invalid(
                        "terminal request does not match the approved immutable rejection",
                    ));
                }
                (snapshot.version - 1, snapshot.version)
            } else {
                if enrichment_request.status().is_terminal() {
                    return Err(rejection_state_invalid(
                        "request already has a different terminal outcome",
                    ));
                }
                enrichment_request.fail_terminal(
                    resolution.safe_reason_code().to_owned(),
                    resolution.resolved_at_unix_ms(),
                )?;
                let aggregate_version = snapshot
                    .version
                    .checked_add(1)
                    .ok_or_else(|| rejection_state_invalid("request version overflow"))?;
                (snapshot.version, aggregate_version)
            };

        let output_request = enrichment_request_to_wire(&enrichment_request)?;
        let persisted = enrichment_request_persisted_payload(&enrichment_request)?;
        let event = support::event_evidence_with_data_class(
            &request,
            record.clone(),
            MODULE_ID,
            EventSpec {
                event_type: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
                event_schema_id: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA,
                aggregate_version,
                previous_version: Some(expected_version),
            },
            DataClass::Personal,
            &wire::EnrichmentRequestStatusChangedEvent {
                enrichment_request: Some(output_request),
            },
        )?;
        let audit = support::audit_intent(
            &request,
            &record,
            aggregate_version,
            RECORD_PROVIDER_RESPONSE_CAPABILITY,
            &persisted.bytes,
        )?;
        let batch = BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: record.clone(),
                expected_version,
                payload: persisted,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(&definition, &request)?,
            audits: vec![audit],
        };
        batch.validate().map_err(rejection_batch_error)?;
        let result = self
            .store
            .execute_batch(&batch)
            .await
            .map_err(rejection_batch_error)?;
        validate_rejection_result(&record, aggregate_version, &enrichment_request, &result)?;
        Ok(ProviderResponseConflictRejectionResult {
            request: enrichment_request,
            replayed: result.replayed,
        })
    }
}

fn rejection_context(
    conflict: &ProviderResponseConflict,
    lineage: &ProviderResponseConflictRejectionLineage,
    suffix: &str,
) -> Result<ModuleExecutionContext, SdkError> {
    let resolution = conflict
        .resolution()
        .ok_or_else(rejection_resolution_missing)?;
    let definition = provider_response_capability_definition()?;
    let request_started_at_unix_nanos = resolution
        .resolved_at_unix_ms()
        .checked_mul(1_000_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| rejection_state_invalid("resolution time exceeds execution range"))?;
    Ok(ModuleExecutionContext {
        module_id: definition.owner_module_id.clone(),
        execution: ExecutionContext {
            tenant_id: conflict.tenant_id().clone(),
            actor_id: lineage.actor_id.clone(),
            request_id: configured(RequestId::try_new(format!(
                "enrichment-conflict-rejection-request-{suffix}"
            )))?,
            correlation_id: lineage.correlation_id.clone(),
            causation_id: resolution.causation_id().clone(),
            trace_id: lineage.trace_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            idempotency_key: configured(IdempotencyKey::try_new(format!(
                "enrichment-conflict-rejection-{suffix}"
            )))?,
            business_transaction_id: configured(BusinessTransactionId::try_new(format!(
                "enrichment-conflict-rejection-tx-{suffix}"
            )))?,
            schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
            request_started_at_unix_nanos,
        },
    })
}

fn validate_rejection_result(
    record: &crm_module_sdk::RecordRef,
    aggregate_version: i64,
    expected: &EnrichmentRequest,
    result: &BatchMutationResult,
) -> Result<(), SdkError> {
    let [snapshot] = result.records.as_slice() else {
        return Err(rejection_state_invalid(
            "terminal rejection returned an unexpected record set",
        ));
    };
    if snapshot.reference != *record || snapshot.version != aggregate_version {
        return Err(rejection_state_invalid(
            "terminal rejection returned the wrong request identity or version",
        ));
    }
    if !result.linked_relationships.is_empty() || !result.unlinked_relationships.is_empty() {
        return Err(rejection_state_invalid(
            "terminal rejection unexpectedly changed relationships",
        ));
    }
    if enrichment_request_from_snapshot(snapshot)? != *expected {
        return Err(rejection_state_invalid(
            "terminal rejection returned different canonical request state",
        ));
    }
    Ok(())
}

fn conflict_suffix(conflict_id: &str) -> Result<&str, SdkError> {
    let suffix = conflict_id
        .strip_prefix(CONFLICT_ID_PREFIX)
        .ok_or_else(|| rejection_state_invalid("conflict id has the wrong prefix"))?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(rejection_state_invalid(
            "conflict id must end in 64 lowercase hexadecimal characters",
        ));
    }
    Ok(suffix)
}

fn rejection_resolution_missing() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_UNRESOLVED",
        ErrorCategory::Conflict,
        true,
        "Provider-response conflict resolution is required before request rejection.",
    )
}

fn rejection_resolution_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_DECISION_INVALID",
        ErrorCategory::Conflict,
        false,
        "The provider-response conflict resolution cannot reject the request.",
    )
    .with_internal_reference(reference.into())
}

fn rejection_request_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The enrichment request selected for conflict rejection was not found.",
    )
}

fn rejection_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STATE_INVALID",
        ErrorCategory::Conflict,
        false,
        "The provider-response conflict rejection state is inconsistent.",
    )
    .with_internal_reference(reference.into())
}

fn rejection_read_error(error: DataError) -> SdkError {
    let (code, category, retryable) = match &error {
        DataError::Database(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STORE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
        ),
        DataError::Sdk(_) | DataError::InvalidPlan(_) | DataError::InvalidStoredValue(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STATE_INVALID",
            ErrorCategory::Internal,
            false,
        ),
    };
    SdkError::new(
        code,
        category,
        retryable,
        "The enrichment request could not be loaded for conflict rejection.",
    )
    .with_internal_reference(error.to_string())
}

fn rejection_batch_error(error: BatchError) -> SdkError {
    let (code, category, retryable) = match error {
        BatchError::Conflict(_) | BatchError::IdempotencyKeyReused => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_CONFLICT",
            ErrorCategory::Conflict,
            false,
        ),
        BatchError::IdempotencyInProgress => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_IN_PROGRESS",
            ErrorCategory::Unavailable,
            true,
        ),
        BatchError::Database(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STORE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
        ),
        BatchError::Sdk(_) | BatchError::InvalidPlan(_) | BatchError::InvalidStoredValue(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_PLAN_INVALID",
            ErrorCategory::Internal,
            false,
        ),
    };
    SdkError::new(
        code,
        category,
        retryable,
        "The terminal provider-response conflict rejection could not be persisted.",
    )
    .with_internal_reference(error.to_string())
}

fn configured<T, E: fmt::Display>(result: Result<T, E>) -> Result<T, SdkError> {
    result.map_err(|error| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The provider-response conflict rejection configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}
'''
Path(
    "crates/crm-customer-enrichment-provider-process-composition/src/conflict_rejection.rs"
).write_text(rejection)

worker = Path("crates/crm-customer-enrichment-provider-process-composition/src/worker.rs")
replace_once(
    worker,
    '''use crate::{
    PostgresProviderResponseConflictStore, ProviderDispatchWorkItemInput,
    ProviderResponseConflictPersistenceLineage, build_provider_dispatch_work_item,
};
''',
    '''use crate::{
    PostgresProviderResponseConflictRejectExecutor, PostgresProviderResponseConflictStore,
    ProviderDispatchWorkItemInput, ProviderResponseConflictPersistenceLineage,
    ProviderResponseConflictRejectionLineage, build_provider_dispatch_work_item,
};
''',
)
replace_once(
    worker,
    '''    pub retained_first_receipts: u32,
}
''',
    '''    pub retained_first_receipts: u32,
    pub rejected_requests: u32,
    pub rejection_replays: u32,
}
''',
)
replace_once(
    worker,
    '''    conflict_store: PostgresProviderResponseConflictStore,
    actor_id: ActorId,
''',
    '''    conflict_store: PostgresProviderResponseConflictStore,
    reject_executor: PostgresProviderResponseConflictRejectExecutor,
    actor_id: ActorId,
''',
)
replace_once(
    worker,
    '''            .field("conflict_store", &self.conflict_store)
            .field("actor_id", &self.actor_id)
''',
    '''            .field("conflict_store", &self.conflict_store)
            .field("reject_executor", &self.reject_executor)
            .field("actor_id", &self.actor_id)
''',
)
replace_once(
    worker,
    '''        Ok(Self {
            conflict_store: PostgresProviderResponseConflictStore::new(store.clone()),
            store,
''',
    '''        Ok(Self {
            conflict_store: PostgresProviderResponseConflictStore::new(store.clone()),
            reject_executor: PostgresProviderResponseConflictRejectExecutor::new(store.clone()),
            store,
''',
)
replace_once(
    worker,
    '''                        Ok(DeliveryDisposition::RetainedFirstReceipt) => {
                            cycle.retained_first_receipts =
                                cycle.retained_first_receipts.saturating_add(1);
                        }
''',
    '''                        Ok(DeliveryDisposition::RetainedFirstReceipt) => {
                            cycle.retained_first_receipts =
                                cycle.retained_first_receipts.saturating_add(1);
                        }
                        Ok(DeliveryDisposition::RejectedRequest { replayed }) => {
                            cycle.rejected_requests = cycle.rejected_requests.saturating_add(1);
                            if replayed {
                                cycle.rejection_replays =
                                    cycle.rejection_replays.saturating_add(1);
                            }
                        }
''',
)
replace_once(
    worker,
    '''                ProviderResponseConflictDecision::RejectRequest => Err(
                    reject_request_resolution_pending(conflict.conflict_id().as_str()),
                ),
''',
    '''                ProviderResponseConflictDecision::RejectRequest => {
                    let result = self
                        .reject_executor
                        .execute(
                            &conflict,
                            ProviderResponseConflictRejectionLineage {
                                actor_id: self.actor_id.clone(),
                                correlation_id: delivery.correlation_id.clone(),
                                trace_id: delivery.trace_id.clone(),
                            },
                        )
                        .await?;
                    Ok(DeliveryDisposition::RejectedRequest {
                        replayed: result.replayed,
                    })
                }
''',
)
replace_once(
    worker,
    '''    RetainedFirstReceipt,
}
''',
    '''    RetainedFirstReceipt,
    RejectedRequest { replayed: bool },
}
''',
)
replace_once(
    worker,
    '''fn reject_request_resolution_pending(conflict_id: &str) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECT_TRANSITION_PENDING",
        crm_module_sdk::ErrorCategory::Conflict,
        true,
        "The approved provider-response conflict rejection has not reached terminal request state.",
    )
    .with_internal_reference(format!("provider_response_conflict_id={conflict_id}"))
}

''',
    "",
)

cargo = Path("crates/crm-customer-enrichment-provider-process-composition/Cargo.toml")
replace_once(
    cargo,
    '''[[test]]
name = "postgres_conflict_resolution"
path = "tests/postgres_conflict_resolution.rs"
required-features = ["postgres-integration"]
''',
    '''[[test]]
name = "postgres_conflict_resolution"
path = "tests/postgres_conflict_resolution.rs"
required-features = ["postgres-integration"]

[[test]]
name = "postgres_conflict_reject_process"
path = "tests/postgres_conflict_reject_process.rs"
required-features = ["postgres-integration"]
''',
)

reject_test = r'''#![cfg(feature = "postgres-integration")]

use crm_capability_plan_support as support;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_core_events::ProjectionStore;
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestStatus, MappingDraft,
    MappingNormalization, MappingVersion, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseConflictDecision, ProviderResponseConflictDraft,
    ProviderResponseConflictResolutionPolicyDecision, ProviderResponseConflictResolutionPolicyPort,
    ProviderResponseConflictResolutionPolicyRequest, ProviderResponseReceiptId, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE,
    ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE, MODULE_ID, enrichment_request_from_snapshot,
    enrichment_request_persisted_payload, enrichment_request_record_ref,
    enrichment_request_to_wire,
};
use crm_customer_enrichment_provider_process_composition::{
    CustomerEnrichmentProviderProcessWorker, PROVIDER_PROCESS_PROJECTION_ID,
    PostgresProviderResponseConflictRejectExecutor,
    PostgresProviderResponseConflictResolutionExecutor, PostgresProviderResponseConflictStore,
    ProviderDispatchExecutorPort, ProviderDispatchSourceDisposition, ProviderDispatchSourcePort,
    ProviderResponseConflictPersistenceLineage, ProviderResponseConflictRejectionLineage,
    ProviderResponseConflictResolutionCommand,
};
use crm_customer_enrichment_worker_composition::{
    ProviderDispatchExecution, ProviderDispatchWorkItem,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, ErrorCategory, EventType, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PortFuture, RecordId, RequestId, SchemaVersion, SdkError,
    TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const TENANT_ID: &str = "tenant-a";
const ACTOR_ID: &str = "actor-a";
const SAFE_REASON: &str = "provider-response-conflict-rejected";
const CORRELATION_ID: &str = "provider-conflict-reject-correlation";
const TRACE_ID: &str = "provider-conflict-reject-trace";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_reject_terminalizes_once_then_resumes_checkpoint_without_provider_io() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL conflict reject process because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect provider conflict reject store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect provider conflict reject evidence reader");
    let request = canonical_request();
    seed_request(&store, &request)
        .await
        .expect("seed reject request-created evidence");

    let conflict_store = PostgresProviderResponseConflictStore::new(store.clone());
    let recorded = conflict_store
        .record(
            ProviderResponseConflictDraft {
                tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
                request_id: request.request_id().clone(),
                retry_generation: request.retry_generation(),
                first_receipt_id: receipt_id(7),
                conflicting_semantic_fingerprint: [9; 32],
                detected_at_unix_ms: 50,
            },
            ProviderResponseConflictPersistenceLineage {
                actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
                correlation_id: CorrelationId::try_new(CORRELATION_ID).unwrap(),
                causation_id: CausationId::try_new("provider-conflict-reject-created-event")
                    .unwrap(),
                trace_id: TraceId::try_new(TRACE_ID).unwrap(),
            },
        )
        .await
        .expect("persist reject conflict");
    let resolver = PostgresProviderResponseConflictResolutionExecutor::new(
        store.clone(),
        Arc::new(AllowRejectPolicy),
    );
    let resolved = resolver
        .execute(ProviderResponseConflictResolutionCommand {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            conflict_id: RecordId::try_new(recorded.conflict.conflict_id().as_str().to_owned())
                .unwrap(),
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            decision: ProviderResponseConflictDecision::RejectRequest,
            safe_reason_code: SAFE_REASON.to_owned(),
            approval_evidence_reference: "approval/provider-conflict/reject-request".to_owned(),
            causation_id: CausationId::try_new("provider-conflict-reject-command").unwrap(),
            correlation_id: CorrelationId::try_new(CORRELATION_ID).unwrap(),
            trace_id: TraceId::try_new(TRACE_ID).unwrap(),
            resolved_at_unix_ms: 70,
        })
        .await
        .expect("persist governed reject resolution");
    assert!(!resolved.replayed);
    assert_eq!(
        resolved
            .conflict
            .resolution()
            .expect("resolution exists")
            .decision(),
        ProviderResponseConflictDecision::RejectRequest
    );

    let before_terminal = evidence_counts(&admin).await;
    let reject_executor = PostgresProviderResponseConflictRejectExecutor::new(store.clone());
    let terminal = reject_executor
        .execute(
            &resolved.conflict,
            ProviderResponseConflictRejectionLineage {
                actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
                correlation_id: CorrelationId::try_new(CORRELATION_ID).unwrap(),
                trace_id: TraceId::try_new(TRACE_ID).unwrap(),
            },
        )
        .await
        .expect("atomically terminalize rejected request");
    assert!(!terminal.replayed);
    assert_eq!(terminal.request.status(), EnrichmentRequestStatus::FailedTerminal);
    assert_eq!(terminal.request.last_safe_failure_code(), Some(SAFE_REASON));
    let after_terminal = evidence_counts(&admin).await;
    assert_eq!(after_terminal.records, before_terminal.records);
    assert_eq!(after_terminal.relationships, before_terminal.relationships);
    assert_eq!(after_terminal.events, before_terminal.events + 1);
    assert_eq!(after_terminal.audits, before_terminal.audits + 1);
    assert_eq!(after_terminal.idempotency, before_terminal.idempotency + 1);
    assert_eq!(after_terminal.transactions, before_terminal.transactions + 1);
    assert_eq!(request_record_version(&admin).await, 2);
    assert_eq!(status_changed_events(&admin).await, 1);
    assert_eq!(suggestion_records(&admin).await, 0);

    let source_calls = Arc::new(AtomicUsize::new(0));
    let executor_calls = Arc::new(AtomicUsize::new(0));
    let worker = CustomerEnrichmentProviderProcessWorker::new(
        store.clone(),
        Arc::new(ForbiddenSource {
            calls: source_calls.clone(),
        }),
        Arc::new(ForbiddenExecutor {
            calls: executor_calls.clone(),
        }),
        ActorId::try_new(ACTOR_ID).unwrap(),
    )
    .expect("compose reject recovery process");
    let resumed = worker
        .run_cycle(TenantId::try_new(TENANT_ID).unwrap(), 80_000_000)
        .await
        .expect("replayed terminal rejection must advance held checkpoint");
    assert_eq!(resumed.created_events, 1);
    assert_eq!(resumed.rejected_requests, 1);
    assert_eq!(resumed.rejection_replays, 1);
    assert_eq!(resumed.dispatched, 0);
    assert_eq!(source_calls.load(Ordering::SeqCst), 0);
    assert_eq!(executor_calls.load(Ordering::SeqCst), 0);
    assert!(
        ProjectionStore::projection_checkpoint(
            &store,
            TenantId::try_new(TENANT_ID).unwrap(),
            PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .expect("read reject-resumed checkpoint")
        .is_some()
    );
    assert_eq!(evidence_counts(&admin).await, after_terminal);

    let snapshot = store
        .get_record(&execution_context(), &enrichment_request_record_ref(&request).unwrap())
        .await
        .expect("reload terminal request")
        .expect("terminal request exists");
    let persisted = enrichment_request_from_snapshot(&snapshot).expect("decode terminal request");
    assert_eq!(persisted.status(), EnrichmentRequestStatus::FailedTerminal);
    assert_eq!(persisted.last_safe_failure_code(), Some(SAFE_REASON));
    assert_eq!(snapshot.version, 2);

    let no_op = worker
        .run_cycle(TenantId::try_new(TENANT_ID).unwrap(), 90_000_000)
        .await
        .expect("post-checkpoint reject replay must be a no-op");
    assert_eq!(no_op.created_events, 0);
    assert_eq!(no_op.rejected_requests, 0);
    assert_eq!(source_calls.load(Ordering::SeqCst), 0);
    assert_eq!(executor_calls.load(Ordering::SeqCst), 0);
    assert_eq!(evidence_counts(&admin).await, after_terminal);
}

#[derive(Clone)]
struct AllowRejectPolicy;

impl ProviderResponseConflictResolutionPolicyPort for AllowRejectPolicy {
    fn evaluate<'a>(
        &'a self,
        request: ProviderResponseConflictResolutionPolicyRequest,
    ) -> PortFuture<'a, Result<ProviderResponseConflictResolutionPolicyDecision, SdkError>> {
        Box::pin(async move {
            assert_eq!(request.decision, ProviderResponseConflictDecision::RejectRequest);
            assert_eq!(request.safe_reason_code, SAFE_REASON);
            Ok(ProviderResponseConflictResolutionPolicyDecision::Allowed {
                policy_version: "provider-conflict-policy-v1".to_owned(),
            })
        })
    }
}

#[derive(Clone)]
struct ForbiddenSource {
    calls: Arc<AtomicUsize>,
}

impl ProviderDispatchSourcePort for ForbiddenSource {
    fn load<'a>(
        &'a self,
        _tenant_id: TenantId,
        _request_id: RecordId,
        _worker_actor_id: ActorId,
        _now_unix_ms: u64,
    ) -> PortFuture<'a, Result<ProviderDispatchSourceDisposition, SdkError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(SdkError::new(
                "TEST_PROVIDER_SOURCE_MUST_NOT_RUN",
                ErrorCategory::Internal,
                false,
                "Provider source must not run after approved rejection.",
            ))
        })
    }
}

#[derive(Clone)]
struct ForbiddenExecutor {
    calls: Arc<AtomicUsize>,
}

impl ProviderDispatchExecutorPort for ForbiddenExecutor {
    fn execute<'a>(
        &'a self,
        _work_item: ProviderDispatchWorkItem,
    ) -> PortFuture<'a, Result<ProviderDispatchExecution, SdkError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(SdkError::new(
                "TEST_PROVIDER_EXECUTOR_MUST_NOT_RUN",
                ErrorCategory::Internal,
                false,
                "Provider executor must not run after approved rejection.",
            ))
        })
    }
}

fn canonical_request() -> EnrichmentRequest {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_conflict_reject".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry conflict reject licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_conflict_reject".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_conflict_reject".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .unwrap();
    EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        requested_by: ActorId::try_new(ACTOR_ID).unwrap(),
        idempotency_key: IdempotencyKey::try_new("provider-conflict-reject-domain-request").unwrap(),
        target: TargetSnapshot::try_new(
            "party-provider-conflict-reject-1",
            7,
            TargetField::PartyDisplayName,
        )
        .unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            None,
            "provider-conflict-reject-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 10,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
    })
    .unwrap()
}

async fn seed_request(
    store: &PostgresDataStore,
    request: &EnrichmentRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let record = enrichment_request_record_ref(request)?;
    let event_payload = support::protobuf_payload(
        MODULE_ID,
        ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::EnrichmentRequestCreatedEvent {
            enrichment_request: Some(enrichment_request_to_wire(request)?),
        },
    )?;
    store
        .create_record(&RecordCreatePlan {
            context: execution_context(),
            record: record.clone(),
            record_payload: enrichment_request_persisted_payload(request)?,
            event_id: "provider-conflict-reject-created-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE)?,
                aggregate: record,
                expected_aggregate_version: None,
                deduplication_key: "provider-conflict-reject-created".to_owned(),
                payload: event_payload,
            },
            idempotency: IdempotencyEvidence {
                scope: "customer_enrichment.response.record@1.0.0".to_owned(),
                key: "provider-conflict-reject-seed".to_owned(),
                request_hash: [61; 32],
                expires_at_unix_nanos: 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "provider-conflict-reject-seed-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\"operation\":\"seed_provider_conflict_reject\"}".to_vec(),
                occurred_at_unix_nanos: 10_000_000,
            },
        })
        .await?;
    Ok(())
}

fn execution_context() -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            request_id: RequestId::try_new("provider-conflict-reject-seed-request").unwrap(),
            correlation_id: CorrelationId::try_new(CORRELATION_ID).unwrap(),
            causation_id: CausationId::try_new("provider-conflict-reject-seed-causation").unwrap(),
            trace_id: TraceId::try_new(TRACE_ID).unwrap(),
            capability_id: CapabilityId::try_new("customer_enrichment.response.record").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("provider-conflict-reject-seed").unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(
                "provider-conflict-reject-seed-tx",
            )
            .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 10_000_000,
        },
    }
}

fn receipt_id(byte: u8) -> ProviderResponseReceiptId {
    serde_json::from_str(&format!(
        "\"enrichment-response-{}\"",
        format!("{byte:02x}").repeat(32)
    ))
    .unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    relationships: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

async fn evidence_counts(pool: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: scalar(pool, "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a'").await,
        relationships: scalar(pool, "SELECT count(*)::bigint FROM crm.relationships WHERE tenant_id = 'tenant-a'").await,
        events: scalar(pool, "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a'").await,
        audits: scalar(pool, "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a'").await,
        idempotency: scalar(pool, "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-a'").await,
        transactions: scalar(pool, "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-a'").await,
    }
}

async fn request_record_version(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT version::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.request'",
    )
    .await
}

async fn status_changed_events(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND event_type = 'customer_enrichment.request.status_changed'",
    )
    .await
}

async fn suggestion_records(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.suggestion'",
    )
    .await
}

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("read provider conflict reject evidence")
}
'''
Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_reject_process.rs"
).write_text(reject_test)

workflow = Path(".github/workflows/customer-enrichment-worker-process-runtime.yml")
text = workflow.read_text()
path_line = '      - "crates/crm-customer-enrichment-worker-composition/**"\n'
if text.count(path_line) != 2:
    raise SystemExit("worker composition path marker count changed")
text = text.replace(
    path_line,
    path_line + '      - "crates/crm-customer-enrichment-provider-process-composition/**"\n',
)
install = '''      - name: Install PostgreSQL client
        run: |
          sudo apt-get update
          sudo apt-get install --yes postgresql-client
'''
provider_steps = '''      - name: Verify durable provider-conflict process scenarios
        shell: bash
        run: |
          set -euo pipefail
          : > database/customer-enrichment-worker-process-runtime.log
          for test_name in \
            postgres_conflict_persistence \
            postgres_conflict_resolution \
            postgres_conflict_process_hold \
            postgres_conflict_reject_process
          do
            bash scripts/prepare_customer_enrichment_worker_process_database.sh
            cargo test \
              -p crm-customer-enrichment-provider-process-composition \
              --features postgres-integration \
              --test "${test_name}" \
              -- --nocapture 2>&1 | tee -a database/customer-enrichment-worker-process-runtime.log
          done
'''
if install not in text:
    raise SystemExit("PostgreSQL install marker not found")
text = text.replace(install, install + provider_steps, 1)
text = text.replace(
    '''          set -euo pipefail
          : > database/customer-enrichment-worker-process-runtime.log
          cargo test \\
            -p crm-customer-enrichment-worker-composition \\
''',
    '''          set -euo pipefail
          cargo test \\
            -p crm-customer-enrichment-worker-composition \\
''',
    1,
)
text = text.replace(
    '''            crates/crm-customer-enrichment-worker-composition/tests/postgres_worker_process.rs
            crates/crm-customer-enrichment-materialization-adapter/
''',
    '''            crates/crm-customer-enrichment-worker-composition/tests/postgres_worker_process.rs
            crates/crm-customer-enrichment-provider-process-composition/
            crates/crm-customer-enrichment-materialization-adapter/
''',
    1,
)
workflow.write_text(text)
