from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    if old not in text:
        raise SystemExit(f"marker not found in {path}: {old[:120]!r}")
    path.write_text(text.replace(old, new, 1))


response_conflict = Path("modules/crm-customer-enrichment/src/response_conflict.rs")
replace_once(
    response_conflict,
    "    ActorId, CausationId, ErrorCategory, FieldName, FieldViolation, SdkError, TenantId,\n",
    "    ActorId, CausationId, ErrorCategory, FieldName, FieldViolation, PortFuture, SdkError,\n    TenantId,\n",
)
policy_contract = r'''
/// Exact immutable conflict binding evaluated immediately before an operator resolution write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictResolutionPolicyRequest {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub conflict_id: ProviderResponseConflictId,
    pub request_id: EnrichmentRequestId,
    pub retry_generation: u32,
    pub first_receipt_id: ProviderResponseReceiptId,
    pub decision: ProviderResponseConflictDecision,
    pub safe_reason_code: String,
    pub approval_evidence_reference: String,
    pub evaluated_at_unix_ms: u64,
}

/// Closed, versioned live authorization outcome for one exact conflict resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderResponseConflictResolutionPolicyDecision {
    Allowed { policy_version: String },
    Denied {
        policy_version: String,
        safe_reason_code: String,
    },
}

/// Infrastructure-owned final authorization boundary for provider-response conflict resolution.
///
/// Implementations must evaluate the exact immutable conflict, operator, decision and approval
/// evidence. The caller must invoke this port after loading current state and immediately before
/// the atomic resolution write.
pub trait ProviderResponseConflictResolutionPolicyPort: Send + Sync {
    fn evaluate<'a>(
        &'a self,
        request: ProviderResponseConflictResolutionPolicyRequest,
    ) -> PortFuture<'a, Result<ProviderResponseConflictResolutionPolicyDecision, SdkError>>;
}

'''
replace_once(response_conflict, "#[cfg(test)]\nmod tests {", policy_contract + "#[cfg(test)]\nmod tests {")

provider_lib = Path("crates/crm-customer-enrichment-provider-process-composition/src/lib.rs")
replace_once(
    provider_lib,
    "mod conflict_persistence;\nmod worker;\n\npub use conflict_persistence::*;\npub use worker::*;",
    "mod conflict_persistence;\nmod conflict_resolution;\nmod worker;\n\npub use conflict_persistence::*;\npub use conflict_resolution::*;\npub use worker::*;",
)

resolution_source = r'''use crate::{
    PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_SCHEMA,
    PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE, provider_response_conflict_persisted_contract,
    provider_response_conflict_persisted_payload, provider_response_conflict_to_wire,
};
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{
    BatchError, BatchMutationPlan, BatchMutationResult, PostgresDataStore, RecordMutation,
};
use crm_customer_enrichment::{
    PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE, ProviderResponseConflict,
    ProviderResponseConflictDecision, ProviderResponseConflictResolutionDraft,
    ProviderResponseConflictResolutionPolicyDecision, ProviderResponseConflictResolutionPolicyPort,
    ProviderResponseConflictResolutionPolicyRequest, ReplayDisposition,
    decode_provider_response_conflict_state, encode_provider_response_conflict_state,
};
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, RECORD_PROVIDER_RESPONSE_CAPABILITY, provider_response_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, ErrorCategory,
    ExecutionContext, IdempotencyKey, ModuleExecutionContext, RecordId, RequestId, SchemaVersion,
    SdkError, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use std::fmt;
use std::sync::Arc;

const CONFLICT_ID_PREFIX: &str = "enrichment-response-conflict-";
const MAX_POLICY_EVIDENCE_BYTES: usize = 80;

/// Exact operator command used only by the internal governed resolution composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictResolutionCommand {
    pub tenant_id: TenantId,
    pub conflict_id: RecordId,
    pub actor_id: ActorId,
    pub decision: ProviderResponseConflictDecision,
    pub safe_reason_code: String,
    pub approval_evidence_reference: String,
    pub causation_id: CausationId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub resolved_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictResolutionResult {
    pub conflict: ProviderResponseConflict,
    pub replayed: bool,
}

#[derive(Clone)]
pub struct PostgresProviderResponseConflictResolutionExecutor {
    store: PostgresDataStore,
    policy: Arc<dyn ProviderResponseConflictResolutionPolicyPort>,
}

impl fmt::Debug for PostgresProviderResponseConflictResolutionExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresProviderResponseConflictResolutionExecutor")
            .field("store", &self.store)
            .field(
                "policy",
                &"dyn ProviderResponseConflictResolutionPolicyPort",
            )
            .finish()
    }
}

impl PostgresProviderResponseConflictResolutionExecutor {
    pub fn new(
        store: PostgresDataStore,
        policy: Arc<dyn ProviderResponseConflictResolutionPolicyPort>,
    ) -> Self {
        Self { store, policy }
    }

    pub async fn execute(
        &self,
        command: ProviderResponseConflictResolutionCommand,
    ) -> Result<ProviderResponseConflictResolutionResult, SdkError> {
        let suffix = conflict_suffix(&command.conflict_id)?;
        let definition = provider_response_capability_definition()?;
        let context = resolution_context(&definition, &command, suffix)?;
        let record = support::record_ref(
            PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
            command.conflict_id.as_str(),
            "customer_enrichment.provider_response_conflict_resolution.provider_response_conflict_id",
        )?;
        let snapshot = self
            .store
            .get_record(&context, &record)
            .await?
            .ok_or_else(conflict_not_found)?;
        if !matches!(snapshot.version, 1 | 2) {
            return Err(resolution_state_invalid(
                "provider-response conflict record version must be 1 or 2",
            ));
        }
        let bytes = support::persisted_json_bytes_with_data_class(
            &snapshot,
            provider_response_conflict_persisted_contract(),
            DataClass::Confidential,
        )?;
        let mut conflict = decode_provider_response_conflict_state(bytes)?;
        if conflict.tenant_id() != &command.tenant_id
            || conflict.conflict_id().as_str() != command.conflict_id.as_str()
        {
            return Err(resolution_state_invalid(
                "loaded provider-response conflict does not match command identity",
            ));
        }

        let policy_decision = self
            .policy
            .evaluate(ProviderResponseConflictResolutionPolicyRequest {
                tenant_id: conflict.tenant_id().clone(),
                actor_id: command.actor_id.clone(),
                conflict_id: conflict.conflict_id().clone(),
                request_id: conflict.request_id().clone(),
                retry_generation: conflict.retry_generation(),
                first_receipt_id: conflict.first_receipt_id().clone(),
                decision: command.decision,
                safe_reason_code: command.safe_reason_code.clone(),
                approval_evidence_reference: command.approval_evidence_reference.clone(),
                evaluated_at_unix_ms: command.resolved_at_unix_ms,
            })
            .await?;
        let policy_version = allowed_policy_version(policy_decision)?;
        let disposition = conflict.resolve(ProviderResponseConflictResolutionDraft {
            decision: command.decision,
            resolved_by: command.actor_id.clone(),
            policy_version,
            safe_reason_code: command.safe_reason_code.clone(),
            approval_evidence_reference: command.approval_evidence_reference.clone(),
            causation_id: command.causation_id.clone(),
            resolved_at_unix_ms: command.resolved_at_unix_ms,
        })?;
        if disposition == ReplayDisposition::Duplicate {
            if snapshot.version != 2 {
                return Err(resolution_state_invalid(
                    "resolved provider-response conflict must be stored at version 2",
                ));
            }
            return Ok(ProviderResponseConflictResolutionResult {
                conflict,
                replayed: true,
            });
        }
        if snapshot.version != 1 {
            return Err(resolution_state_invalid(
                "unresolved provider-response conflict must be stored at version 1",
            ));
        }

        let input = provider_response_conflict_persisted_payload(&conflict)?;
        let request = CapabilityRequest {
            context,
            input_hash: semantic_input_hash(&input),
            input,
            approval: None,
        };
        let state_bytes = encode_provider_response_conflict_state(&conflict)?;
        let event = support::event_evidence_with_data_class(
            &request,
            record.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE,
                event_schema_id: PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_SCHEMA,
                aggregate_version: 2,
                previous_version: Some(1),
            },
            DataClass::Confidential,
            &wire::ProviderResponseConflictRecordedEvent {
                provider_response_conflict: Some(provider_response_conflict_to_wire(&conflict)?),
            },
        )?;
        let audit = support::audit_intent(
            &request,
            &record,
            2,
            RECORD_PROVIDER_RESPONSE_CAPABILITY,
            &state_bytes,
        )?;
        let batch = BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: record.clone(),
                expected_version: 1,
                payload: provider_response_conflict_persisted_payload(&conflict)?,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(&definition, &request)?,
            audits: vec![audit],
        };
        batch.validate().map_err(resolution_batch_error)?;
        let result = self
            .store
            .execute_batch(&batch)
            .await
            .map_err(resolution_batch_error)?;
        validate_resolution_result(&record, &conflict, &result)?;
        Ok(ProviderResponseConflictResolutionResult {
            conflict,
            replayed: result.replayed,
        })
    }
}

fn resolution_context(
    definition: &crm_capability_runtime::CapabilityDefinition,
    command: &ProviderResponseConflictResolutionCommand,
    suffix: &str,
) -> Result<ModuleExecutionContext, SdkError> {
    let request_started_at_unix_nanos = command
        .resolved_at_unix_ms
        .checked_mul(1_000_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| resolution_input_invalid("resolution time exceeds execution range"))?;
    Ok(ModuleExecutionContext {
        module_id: definition.owner_module_id.clone(),
        execution: ExecutionContext {
            tenant_id: command.tenant_id.clone(),
            actor_id: command.actor_id.clone(),
            request_id: configured(RequestId::try_new(format!(
                "enrichment-conflict-resolution-request-{suffix}"
            )))?,
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
            trace_id: command.trace_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            idempotency_key: configured(IdempotencyKey::try_new(format!(
                "enrichment-conflict-resolution-{suffix}"
            )))?,
            business_transaction_id: configured(BusinessTransactionId::try_new(format!(
                "enrichment-conflict-resolution-tx-{suffix}"
            )))?,
            schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
            request_started_at_unix_nanos,
        },
    })
}

fn allowed_policy_version(
    decision: ProviderResponseConflictResolutionPolicyDecision,
) -> Result<String, SdkError> {
    match decision {
        ProviderResponseConflictResolutionPolicyDecision::Allowed { policy_version } => {
            validate_policy_evidence(&policy_version, "policy_version")?;
            Ok(policy_version)
        }
        ProviderResponseConflictResolutionPolicyDecision::Denied {
            policy_version,
            safe_reason_code,
        } => {
            validate_policy_evidence(&policy_version, "policy_version")?;
            validate_policy_evidence(&safe_reason_code, "safe_reason_code")?;
            Err(SdkError::new(
                "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_DENIED",
                ErrorCategory::Authorization,
                false,
                "The provider-response conflict resolution is not authorized.",
            )
            .with_internal_reference(format!(
                "policy_version={policy_version};reason_code={safe_reason_code}"
            )))
        }
    }
}

fn validate_policy_evidence(value: &str, field: &'static str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAX_POLICY_EVIDENCE_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(resolution_policy_invalid(format!(
            "{field} is not bounded canonical policy evidence"
        )));
    }
    Ok(())
}

fn validate_resolution_result(
    record: &crm_module_sdk::RecordRef,
    expected: &ProviderResponseConflict,
    result: &BatchMutationResult,
) -> Result<(), SdkError> {
    let [snapshot] = result.records.as_slice() else {
        return Err(resolution_state_invalid(
            "conflict resolution returned an unexpected record set",
        ));
    };
    if snapshot.reference != *record || snapshot.version != 2 {
        return Err(resolution_state_invalid(
            "conflict resolution returned the wrong record identity or version",
        ));
    }
    if !result.linked_relationships.is_empty() || !result.unlinked_relationships.is_empty() {
        return Err(resolution_state_invalid(
            "conflict resolution unexpectedly changed relationships",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        provider_response_conflict_persisted_contract(),
        DataClass::Confidential,
    )?;
    if decode_provider_response_conflict_state(bytes)? != *expected {
        return Err(resolution_state_invalid(
            "conflict resolution returned different canonical state",
        ));
    }
    Ok(())
}

fn conflict_suffix(conflict_id: &RecordId) -> Result<&str, SdkError> {
    let suffix = conflict_id
        .as_str()
        .strip_prefix(CONFLICT_ID_PREFIX)
        .ok_or_else(|| resolution_input_invalid("conflict id has the wrong prefix"))?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(resolution_input_invalid(
            "conflict id must end in 64 lowercase hexadecimal characters",
        ));
    }
    Ok(suffix)
}

fn conflict_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The provider-response conflict was not found.",
    )
}

fn resolution_input_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_INPUT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The provider-response conflict resolution input is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn resolution_policy_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_POLICY_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider-response conflict resolution policy response is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn resolution_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The stored provider-response conflict resolution state is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn resolution_batch_error(error: BatchError) -> SdkError {
    let (code, category, retryable) = match error {
        BatchError::Conflict(_) | BatchError::IdempotencyKeyReused => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_CONFLICT",
            ErrorCategory::Conflict,
            false,
        ),
        BatchError::IdempotencyInProgress => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_IN_PROGRESS",
            ErrorCategory::Unavailable,
            true,
        ),
        BatchError::Database(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STORE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
        ),
        BatchError::Sdk(_) | BatchError::InvalidPlan(_) | BatchError::InvalidStoredValue(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_PLAN_INVALID",
            ErrorCategory::Internal,
            false,
        ),
    };
    SdkError::new(
        code,
        category,
        retryable,
        "The provider-response conflict resolution could not be persisted.",
    )
    .with_internal_reference(error.to_string())
}

fn configured<T, E: fmt::Display>(result: Result<T, E>) -> Result<T, SdkError> {
    result.map_err(|error| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The provider-response conflict resolution configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}
'''
Path("crates/crm-customer-enrichment-provider-process-composition/src/conflict_resolution.rs").write_text(
    resolution_source
)

cargo = Path("crates/crm-customer-enrichment-provider-process-composition/Cargo.toml")
replace_once(
    cargo,
    '[[test]]\nname = "postgres_conflict_process_hold"\npath = "tests/postgres_conflict_process_hold.rs"\nrequired-features = ["postgres-integration"]\n',
    '[[test]]\nname = "postgres_conflict_process_hold"\npath = "tests/postgres_conflict_process_hold.rs"\nrequired-features = ["postgres-integration"]\n\n[[test]]\nname = "postgres_conflict_resolution"\npath = "tests/postgres_conflict_resolution.rs"\nrequired-features = ["postgres-integration"]\n',
)

resolution_test = r'''#![cfg(feature = "postgres-integration")]

use crm_capability_plan_support as support;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestId, MappingDraft,
    MappingNormalization, MappingVersion, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseConflictDecision, ProviderResponseConflictDraft,
    ProviderResponseConflictResolutionPolicyDecision, ProviderResponseConflictResolutionPolicyPort,
    ProviderResponseConflictResolutionPolicyRequest, ProviderResponseReceiptId, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE, MODULE_ID,
    enrichment_request_persisted_payload, enrichment_request_record_ref,
    enrichment_request_to_wire,
};
use crm_customer_enrichment_provider_process_composition::{
    PostgresProviderResponseConflictResolutionExecutor, PostgresProviderResponseConflictStore,
    ProviderResponseConflictPersistenceLineage, ProviderResponseConflictResolutionCommand,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    ModuleId, PortFuture, RecordId, RequestId, SchemaVersion, SdkError, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const TENANT_ID: &str = "tenant-a";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_conflict_resolution_is_live_authorized_immutable_and_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL conflict resolution because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect conflict resolution store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect conflict resolution evidence reader");
    let request = canonical_request();
    seed_request(&store, &request)
        .await
        .expect("seed canonical enrichment request");
    let persistence = PostgresProviderResponseConflictStore::new(store.clone());
    let recorded = persistence
        .record(draft(request.request_id().clone()), lineage())
        .await
        .expect("record unresolved provider-response conflict");
    let conflict_id = recorded.conflict.conflict_id().as_str().to_owned();

    let denied_policy = Arc::new(StaticPolicy::new(
        ProviderResponseConflictResolutionPolicyDecision::Denied {
            policy_version: "provider-conflict-policy-v1".to_owned(),
            safe_reason_code: "operator-not-authorized".to_owned(),
        },
    ));
    let denied_executor = PostgresProviderResponseConflictResolutionExecutor::new(
        store.clone(),
        denied_policy.clone(),
    );
    let denied = denied_executor
        .execute(command(
            &conflict_id,
            ProviderResponseConflictDecision::RetainFirstReceipt,
        ))
        .await
        .expect_err("denied resolution must fail closed");
    assert_eq!(
        denied.code,
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_DENIED"
    );
    assert_eq!(denied_policy.calls(), 1);
    assert_eq!(record_version(&admin).await, 1);

    let allowed_policy = Arc::new(StaticPolicy::new(
        ProviderResponseConflictResolutionPolicyDecision::Allowed {
            policy_version: "provider-conflict-policy-v1".to_owned(),
        },
    ));
    let executor = PostgresProviderResponseConflictResolutionExecutor::new(
        store.clone(),
        allowed_policy.clone(),
    );
    let first = executor
        .execute(command(
            &conflict_id,
            ProviderResponseConflictDecision::RetainFirstReceipt,
        ))
        .await
        .expect("persist authorized retain-first resolution");
    assert!(!first.replayed);
    assert_eq!(
        first
            .conflict
            .resolution()
            .expect("resolution exists")
            .decision(),
        ProviderResponseConflictDecision::RetainFirstReceipt
    );
    assert_eq!(record_version(&admin).await, 2);

    let replay = executor
        .execute(command(
            &conflict_id,
            ProviderResponseConflictDecision::RetainFirstReceipt,
        ))
        .await
        .expect("replay exact authorized resolution");
    assert!(replay.replayed);
    assert_eq!(replay.conflict, first.conflict);

    let conflicting = executor
        .execute(command(
            &conflict_id,
            ProviderResponseConflictDecision::RejectRequest,
        ))
        .await
        .expect_err("contradictory immutable resolution must fail");
    assert_eq!(
        conflicting.code,
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_ALREADY_RESOLVED"
    );
    assert_eq!(allowed_policy.calls(), 3);

    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_response_conflict'",
        )
        .await,
        1
    );
    assert_eq!(
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
        2
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND capability_id = 'customer_enrichment.response.record'",
        )
        .await,
        3
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-a' AND idempotency_scope = 'capability:customer_enrichment.response.record:1.0.0' AND idempotency_key LIKE 'enrichment-conflict-resolution-%'",
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-a' AND capability_id = 'customer_enrichment.response.record' AND business_transaction_id LIKE 'enrichment-conflict-resolution-tx-%'",
        )
        .await,
        1
    );
}

#[derive(Clone)]
struct StaticPolicy {
    decision: ProviderResponseConflictResolutionPolicyDecision,
    calls: Arc<AtomicUsize>,
}

impl StaticPolicy {
    fn new(decision: ProviderResponseConflictResolutionPolicyDecision) -> Self {
        Self {
            decision,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ProviderResponseConflictResolutionPolicyPort for StaticPolicy {
    fn evaluate<'a>(
        &'a self,
        _request: ProviderResponseConflictResolutionPolicyRequest,
    ) -> PortFuture<'a, Result<ProviderResponseConflictResolutionPolicyDecision, SdkError>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let decision = self.decision.clone();
        Box::pin(async move { Ok(decision) })
    }
}

fn command(
    conflict_id: &str,
    decision: ProviderResponseConflictDecision,
) -> ProviderResponseConflictResolutionCommand {
    ProviderResponseConflictResolutionCommand {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        conflict_id: RecordId::try_new(conflict_id).unwrap(),
        actor_id: ActorId::try_new("operator-a").unwrap(),
        decision,
        safe_reason_code: "retain-first-receipt".to_owned(),
        approval_evidence_reference: "approval/provider-conflict/1".to_owned(),
        causation_id: CausationId::try_new("operator-command-1").unwrap(),
        correlation_id: CorrelationId::try_new("operator-correlation-1").unwrap(),
        trace_id: TraceId::try_new("operator-trace-1").unwrap(),
        resolved_at_unix_ms: 60,
    }
}

fn draft(request_id: EnrichmentRequestId) -> ProviderResponseConflictDraft {
    ProviderResponseConflictDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        request_id,
        retry_generation: 2,
        first_receipt_id: receipt_id(2),
        conflicting_semantic_fingerprint: [3; 32],
        detected_at_unix_ms: 50,
    }
}

fn lineage() -> ProviderResponseConflictPersistenceLineage {
    ProviderResponseConflictPersistenceLineage {
        actor_id: ActorId::try_new("provider-worker-a").unwrap(),
        correlation_id: CorrelationId::try_new("provider-conflict-correlation").unwrap(),
        causation_id: CausationId::try_new("provider-created-event").unwrap(),
        trace_id: TraceId::try_new("provider-conflict-trace").unwrap(),
    }
}

fn canonical_request() -> EnrichmentRequest {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_conflict_resolution".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry conflict resolution licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_conflict_resolution".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_conflict_resolution".to_owned(),
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
        requested_by: ActorId::try_new("actor-a").unwrap(),
        idempotency_key: IdempotencyKey::try_new("provider-conflict-resolution-request").unwrap(),
        target: TargetSnapshot::try_new(
            "party-provider-conflict-resolution-1",
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
            "provider-conflict-resolution-policy-v1",
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
    let payload = support::protobuf_payload(
        MODULE_ID,
        ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::EnrichmentRequestCreatedEvent {
            enrichment_request: Some(enrichment_request_to_wire(request)?),
        },
    )?;
    store
        .create_record(&RecordCreatePlan {
            context: seed_context(),
            record: record.clone(),
            record_payload: enrichment_request_persisted_payload(request)?,
            event_id: "provider-conflict-resolution-seed-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE)?,
                aggregate: record,
                expected_aggregate_version: None,
                deduplication_key: "provider-conflict-resolution-seed".to_owned(),
                payload,
            },
            idempotency: IdempotencyEvidence {
                scope: "customer_enrichment.response.record@1.0.0".to_owned(),
                key: "provider-conflict-resolution-seed".to_owned(),
                request_hash: [51; 32],
                expires_at_unix_nanos: 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "provider-conflict-resolution-seed-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\"operation\":\"seed_provider_conflict_resolution\"}".to_vec(),
                occurred_at_unix_nanos: 10_000_000,
            },
        })
        .await?;
    Ok(())
}

fn seed_context() -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new("actor-a").unwrap(),
            request_id: RequestId::try_new("provider-conflict-resolution-seed-request").unwrap(),
            correlation_id: CorrelationId::try_new("provider-conflict-resolution-seed-correlation")
                .unwrap(),
            causation_id: CausationId::try_new("provider-conflict-resolution-seed-causation")
                .unwrap(),
            trace_id: TraceId::try_new("provider-conflict-resolution-seed-trace").unwrap(),
            capability_id: CapabilityId::try_new("customer_enrichment.response.record").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("provider-conflict-resolution-seed").unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(
                "provider-conflict-resolution-seed-tx",
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

async fn record_version(pool: &PgPool) -> i64 {
    scalar(
        pool,
        "SELECT version::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND record_type = 'customer_enrichment.provider_response_conflict'",
    )
    .await
}

async fn scalar(pool: &PgPool, query: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .expect("read PostgreSQL conflict resolution evidence")
}
'''
Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_resolution.rs"
).write_text(resolution_test)
