use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilityRisk, TransactionalCapabilityExecutor,
};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    PostgresDataStore, PostgresTransactionalAggregateExecutor, RecordMutation,
    TransactionalAggregateGuard, TransactionalAggregatePlanner,
};
use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID, DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION, DiscoveryScopeSnapshot, MODULE_ID,
    PRIVACY_CASE_RECORD_TYPE, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind, PrivacyCaseStatus,
    PrivacyDomainError, RescopeRequirement, ResumeStage, SCOPE_SNAPSHOT_RECORD_TYPE,
    SubjectBinding, SubjectVerificationMethod, action_plan_state_descriptor_hash,
    decode_action_plan_state, decode_discovery_scope_snapshot_state,
    discovery_scope_snapshot_state_descriptor_hash,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload, privacy_case_record_ref,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RecordSnapshot, RecordType, RetentionPolicyId, SchemaId,
    SchemaVersion, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1 as customer_wire, customer_privacy::v1 as wire};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

pub const APPROVE_PRIVACY_CASE_CAPABILITY: &str = "customer_privacy.case.approve";
pub const APPROVE_PRIVACY_CASE_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.ApprovePrivacyCaseRequest";
pub const APPROVE_PRIVACY_CASE_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.ApprovePrivacyCaseResponse";
pub const PRIVACY_CASE_STATUS_CHANGED_EVENT_TYPE: &str = "customer_privacy.case.status_changed";
pub const PRIVACY_CASE_STATUS_CHANGED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.PrivacyCaseStatusChangedEvent";

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyCaseApprovalCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyCaseApprovalCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_exact_coordinate(definition, request)?;
        Ok(AggregateTarget {
            reference: privacy_case_ref_from_request(request)?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_exact_coordinate(definition, request)?;
        let command = approval_command(request)?;
        let requested_ref = case_ref(
            command
                .privacy_case_ref
                .ok_or_else(|| required("customer_privacy.privacy_case_ref"))?,
        )?;
        let current = current.ok_or_else(case_not_found)?;
        if current.reference != requested_ref {
            return Err(case_not_found());
        }

        let mut privacy_case = privacy_case_from_snapshot(current).map_err(case_state_invalid)?;
        if privacy_case.case_id() != &requested_ref.record_id
            || privacy_case.tenant_id() != &request.context.execution.tenant_id
        {
            return Err(case_not_found());
        }

        let previous_version = i64::try_from(privacy_case.version())
            .map_err(|_| invalid_plan("persisted case version exceeds i64"))?;
        privacy_case
            .approve(
                positive_version(command.expected_version)?,
                request.context.execution.actor_id.clone(),
                request.context.execution.request_started_at_unix_nanos,
            )
            .map_err(domain_error)?;

        build_plan(definition, request, current, privacy_case, previous_version)
    }
}

pub fn approval_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(APPROVE_PRIVACY_CASE_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            APPROVE_PRIVACY_CASE_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            APPROVE_PRIVACY_CASE_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: APPROVE_PRIVACY_CASE_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn privacy_case_ref_from_request(request: &CapabilityRequest) -> Result<RecordRef, SdkError> {
    let command = approval_command(request)?;
    case_ref(
        command
            .privacy_case_ref
            .ok_or_else(|| required("customer_privacy.privacy_case_ref"))?,
    )
}

pub fn expected_version_from_request(request: &CapabilityRequest) -> Result<u64, SdkError> {
    positive_version(approval_command(request)?.expected_version)
}

fn approval_command(
    request: &CapabilityRequest,
) -> Result<wire::ApprovePrivacyCaseRequest, SdkError> {
    request.context.validate()?;
    let command = support::decode_request::<wire::ApprovePrivacyCaseRequest>(
        request,
        MODULE_ID,
        APPROVE_PRIVACY_CASE_REQUEST_SCHEMA,
    )?;
    positive_version(command.expected_version)?;
    Ok(command)
}

fn build_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: &RecordSnapshot,
    privacy_case: PrivacyCase,
    previous_version: i64,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let next_version = i64::try_from(privacy_case.version())
        .map_err(|_| invalid_plan("approved case version exceeds i64"))?;
    let approval = privacy_case
        .approval()
        .ok_or_else(|| invalid_plan("approved case is missing approval evidence"))?;
    if privacy_case.status() != PrivacyCaseStatus::Planned
        || current.version != previous_version
        || next_version != previous_version + 1
        || approval.approved_by != request.context.execution.actor_id
        || approval.approved_at_unix_nanos
            != request.context.execution.request_started_at_unix_nanos
    {
        return Err(invalid_plan(
            "case.approve must advance one awaiting-approval aggregate version with exact actor/time evidence",
        ));
    }

    let aggregate = privacy_case_record_ref(&privacy_case)?;
    if aggregate != current.reference {
        return Err(case_not_found());
    }
    let public_case = privacy_case_to_wire(&privacy_case)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        APPROVE_PRIVACY_CASE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::ApprovePrivacyCaseResponse {
            privacy_case: Some(public_case.clone()),
        },
    )?;
    let event = support::event_evidence(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PRIVACY_CASE_STATUS_CHANGED_EVENT_TYPE,
            event_schema_id: PRIVACY_CASE_STATUS_CHANGED_EVENT_SCHEMA,
            aggregate_version: next_version,
            previous_version: Some(previous_version),
        },
        &wire::PrivacyCaseStatusChangedEvent {
            privacy_case: Some(public_case),
        },
    )?;
    let audit = support::audit_intent(
        request,
        &aggregate,
        next_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;

    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: aggregate,
                expected_version: previous_version,
                payload: privacy_case_persisted_payload(&privacy_case)?,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacyApprovalGuard;

impl TransactionalAggregateGuard for PostgresCustomerPrivacyApprovalGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if request.context.execution.capability_id.as_str() != APPROVE_PRIVACY_CASE_CAPABILITY {
                return Err(approval_guard_unsupported());
            }
            let reference = privacy_case_ref_from_request(request)?;
            let expected_version = expected_version_from_request(request)?;
            let privacy_case =
                load_case_in_transaction(transaction, &reference.record_id, RowLock::Update)
                    .await?
                    .ok_or_else(case_not_found)?;

            if privacy_case.case_id() != &reference.record_id
                || privacy_case.tenant_id() != &request.context.execution.tenant_id
            {
                return Err(case_not_found());
            }
            if privacy_case.version() != expected_version {
                return Err(version_conflict(expected_version, privacy_case.version()));
            }
            if privacy_case.status() != PrivacyCaseStatus::AwaitingApproval
                || privacy_case.approval().is_some()
            {
                return Err(approval_conflict(
                    "only an awaiting-approval case without prior approval evidence is eligible",
                ));
            }

            let binding = privacy_case.subject_binding().ok_or_else(|| {
                approval_evidence_invalid("awaiting-approval case has no verified subject binding")
            })?;
            let snapshot_id = privacy_case.scope_snapshot_id().ok_or_else(|| {
                approval_evidence_invalid("awaiting-approval case has no scope snapshot reference")
            })?;
            let plan_id = privacy_case.action_plan_id().ok_or_else(|| {
                approval_evidence_invalid("awaiting-approval case has no action plan reference")
            })?;

            let snapshot = load_snapshot_in_transaction(transaction, snapshot_id, RowLock::Share)
                .await?
                .ok_or_else(|| approval_evidence_invalid("referenced scope snapshot is missing"))?;
            let plan = load_plan_in_transaction(transaction, plan_id, RowLock::Share)
                .await?
                .ok_or_else(|| approval_evidence_invalid("referenced action plan is missing"))?;

            validate_exact_lineage(&privacy_case, &snapshot, &plan, binding)?;
            verify_planning_link_in_transaction(transaction, &privacy_case, &plan).await
        })
    }
}

pub fn postgres_case_approval_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyCaseApprovalCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacyApprovalGuard),
    ))
}

fn validate_exact_lineage(
    privacy_case: &PrivacyCase,
    snapshot: &DiscoveryScopeSnapshot,
    plan: &PrivacyActionPlan,
    binding: &SubjectBinding,
) -> Result<(), SdkError> {
    let snapshot_lineage = snapshot.lineage();
    let plan_lineage = plan.lineage();
    let expected_resulting_version = plan_lineage
        .source_case_version()
        .checked_add(1)
        .ok_or_else(|| approval_evidence_invalid("planning case version overflowed"))?;

    if !plan_lineage.approval_required()
        || expected_resulting_version != privacy_case.version()
        || plan_lineage.privacy_case_id() != privacy_case.case_id()
        || plan_lineage.tenant_id() != privacy_case.tenant_id()
        || plan_lineage.case_kind() != privacy_case.kind()
        || plan_lineage.policy_version() != privacy_case.policy_version()
        || plan_lineage.canonical_party_id() != &binding.canonical_party_id
        || plan_lineage.identity_resolution_generation() != binding.identity_resolution_generation
        || plan_lineage.scope_snapshot_id() != snapshot.snapshot_id()
        || privacy_case.scope_snapshot_id() != Some(snapshot.snapshot_id())
        || privacy_case.action_plan_id() != Some(plan.plan_id())
    {
        return Err(approval_evidence_invalid(
            "case and immutable action-plan lineage do not match exactly",
        ));
    }

    if snapshot_lineage.privacy_case_id() != privacy_case.case_id()
        || snapshot_lineage.tenant_id() != privacy_case.tenant_id()
        || snapshot_lineage.canonical_party_id() != &binding.canonical_party_id
        || snapshot_lineage.identity_resolution_generation()
            != binding.identity_resolution_generation
        || plan_lineage.scope_snapshot_binding_digest() != snapshot.binding_digest()
        || plan_lineage.scope_completeness_digest() != snapshot.aggregation().completeness_digest()
        || plan_lineage.registry_digest() != snapshot_lineage.registry_digest()
        || plan_lineage.purpose_code() != snapshot_lineage.purpose_code()
        || plan_lineage.effective_request_at_unix_ms()
            != snapshot_lineage.effective_request_at_unix_ms()
        || plan_lineage.snapshot_captured_at_unix_nanos() != snapshot.captured_at_unix_nanos()
    {
        return Err(approval_evidence_invalid(
            "scope snapshot and immutable action-plan lineage do not match exactly",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum RowLock {
    Share,
    Update,
}

async fn load_case_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    case_id: &RecordId,
    lock: RowLock,
) -> Result<Option<PrivacyCase>, SdkError> {
    let row = select_record_row(transaction, PRIVACY_CASE_RECORD_TYPE, case_id, lock).await?;
    row.map(|row| decode_case_row(case_id.clone(), row))
        .transpose()
}

async fn load_snapshot_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    snapshot_id: &RecordId,
    lock: RowLock,
) -> Result<Option<DiscoveryScopeSnapshot>, SdkError> {
    let row = select_record_row(transaction, SCOPE_SNAPSHOT_RECORD_TYPE, snapshot_id, lock).await?;
    row.map(|row| decode_snapshot_row(snapshot_id, row))
        .transpose()
}

async fn load_plan_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    plan_id: &RecordId,
    lock: RowLock,
) -> Result<Option<PrivacyActionPlan>, SdkError> {
    let row = select_record_row(transaction, ACTION_PLAN_RECORD_TYPE, plan_id, lock).await?;
    row.map(|row| decode_plan_row(plan_id, row)).transpose()
}

async fn select_record_row(
    transaction: &mut Transaction<'_, Postgres>,
    record_type: &str,
    record_id: &RecordId,
    lock: RowLock,
) -> Result<Option<sqlx::postgres::PgRow>, SdkError> {
    let query = match lock {
        RowLock::Share => sqlx::query(
            r#"
            SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
                   data_class, payload_encoding, maximum_payload_size, retention_policy_id,
                   payload_bytes
            FROM crm.records
            WHERE tenant_id = current_setting('app.tenant_id', true)
              AND owner_module_id = $1 AND record_type = $2 AND record_id = $3
              AND deleted_at IS NULL
            FOR SHARE
            "#,
        ),
        RowLock::Update => sqlx::query(
            r#"
            SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
                   data_class, payload_encoding, maximum_payload_size, retention_policy_id,
                   payload_bytes
            FROM crm.records
            WHERE tenant_id = current_setting('app.tenant_id', true)
              AND owner_module_id = $1 AND record_type = $2 AND record_id = $3
              AND deleted_at IS NULL
            FOR UPDATE
            "#,
        ),
    };
    query
        .bind(MODULE_ID)
        .bind(record_type)
        .bind(record_id.as_str())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error)
}

fn decode_case_row(case_id: RecordId, row: sqlx::postgres::PgRow) -> Result<PrivacyCase, SdkError> {
    let snapshot = decode_record_snapshot(PRIVACY_CASE_RECORD_TYPE, case_id, row)?;
    privacy_case_from_snapshot(&snapshot).map_err(|error| {
        approval_evidence_invalid(format!("privacy case state is invalid: {}", error.code))
    })
}

fn decode_snapshot_row(
    snapshot_id: &RecordId,
    row: sqlx::postgres::PgRow,
) -> Result<DiscoveryScopeSnapshot, SdkError> {
    let snapshot = decode_record_snapshot(SCOPE_SNAPSHOT_RECORD_TYPE, snapshot_id.clone(), row)?;
    validate_immutable_contract(
        &snapshot,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION,
        discovery_scope_snapshot_state_descriptor_hash(),
        DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
        DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID,
    )?;
    let value = decode_discovery_scope_snapshot_state(&snapshot.payload.bytes)
        .map_err(approval_evidence_invalid)?;
    if value.snapshot_id() != snapshot_id {
        return Err(approval_evidence_invalid(
            "scope snapshot identity differs from its record envelope",
        ));
    }
    Ok(value)
}

fn decode_plan_row(
    plan_id: &RecordId,
    row: sqlx::postgres::PgRow,
) -> Result<PrivacyActionPlan, SdkError> {
    let snapshot = decode_record_snapshot(ACTION_PLAN_RECORD_TYPE, plan_id.clone(), row)?;
    validate_immutable_contract(
        &snapshot,
        ACTION_PLAN_STATE_SCHEMA_ID,
        ACTION_PLAN_STATE_SCHEMA_VERSION,
        action_plan_state_descriptor_hash(),
        ACTION_PLAN_STATE_MAXIMUM_BYTES,
        ACTION_PLAN_STATE_RETENTION_POLICY_ID,
    )?;
    let value =
        decode_action_plan_state(&snapshot.payload.bytes).map_err(approval_evidence_invalid)?;
    if value.plan_id() != plan_id {
        return Err(approval_evidence_invalid(
            "action plan identity differs from its record envelope",
        ));
    }
    Ok(value)
}

fn decode_record_snapshot(
    record_type: &str,
    record_id: RecordId,
    row: sqlx::postgres::PgRow,
) -> Result<RecordSnapshot, SdkError> {
    let version: i64 = row.try_get("version").map_err(approval_evidence_invalid)?;
    let owner: String = row
        .try_get("owner_module_id")
        .map_err(approval_evidence_invalid)?;
    let schema_id: String = row
        .try_get("schema_id")
        .map_err(approval_evidence_invalid)?;
    let schema_version: String = row
        .try_get("schema_version")
        .map_err(approval_evidence_invalid)?;
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(approval_evidence_invalid)?;
    let data_class: String = row
        .try_get("data_class")
        .map_err(approval_evidence_invalid)?;
    let encoding: String = row
        .try_get("payload_encoding")
        .map_err(approval_evidence_invalid)?;
    let maximum: i64 = row
        .try_get("maximum_payload_size")
        .map_err(approval_evidence_invalid)?;
    let retention: String = row
        .try_get("retention_policy_id")
        .map_err(approval_evidence_invalid)?;
    let bytes: Vec<u8> = row
        .try_get("payload_bytes")
        .map_err(approval_evidence_invalid)?;
    if data_class != "confidential" || encoding != "json" {
        return Err(approval_evidence_invalid(
            "approval evidence record data class or encoding drifted",
        ));
    }
    let descriptor_hash: [u8; 32] = descriptor_hash.try_into().map_err(|_| {
        approval_evidence_invalid("approval evidence descriptor hash must contain 32 bytes")
    })?;
    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type).map_err(approval_evidence_invalid)?,
            record_id,
        },
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(owner).map_err(approval_evidence_invalid)?,
            schema_id: SchemaId::try_new(schema_id).map_err(approval_evidence_invalid)?,
            schema_version: SchemaVersion::try_new(schema_version)
                .map_err(approval_evidence_invalid)?,
            descriptor_hash,
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: u64::try_from(maximum).map_err(|_| {
                approval_evidence_invalid("approval evidence maximum size is negative")
            })?,
            retention_policy_id: RetentionPolicyId::try_new(retention)
                .map_err(approval_evidence_invalid)?,
            bytes,
        },
    })
}

fn validate_immutable_contract(
    snapshot: &RecordSnapshot,
    schema_id: &str,
    schema_version: &str,
    descriptor_hash: [u8; 32],
    maximum_size_bytes: u64,
    retention_policy_id: &str,
) -> Result<(), SdkError> {
    if snapshot.version != 1
        || snapshot.payload.owner.as_str() != MODULE_ID
        || snapshot.payload.schema_id.as_str() != schema_id
        || snapshot.payload.schema_version.as_str() != schema_version
        || snapshot.payload.descriptor_hash != descriptor_hash
        || snapshot.payload.maximum_size_bytes != maximum_size_bytes
        || snapshot.payload.retention_policy_id.as_str() != retention_policy_id
        || snapshot.payload.validate().is_err()
    {
        return Err(approval_evidence_invalid(
            "immutable approval evidence record envelope drifted",
        ));
    }
    Ok(())
}

async fn verify_planning_link_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    privacy_case: &PrivacyCase,
    plan: &PrivacyActionPlan,
) -> Result<(), SdkError> {
    let row = sqlx::query(
        r#"
        SELECT source_case_version, resulting_case_version, scope_snapshot_id,
               plan_id, plan_digest, approval_required,
               (extract(epoch FROM planned_at) * 1000000000)::bigint AS planned_at_unix_nanos
        FROM crm.customer_privacy_action_plans
        WHERE tenant_id = current_setting('app.tenant_id', true)
          AND privacy_case_id = $1
        "#,
    )
    .bind(privacy_case.case_id().as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?
    .ok_or_else(|| approval_evidence_invalid("immutable planning link is missing"))?;

    let source_case_version: i64 = row
        .try_get("source_case_version")
        .map_err(approval_evidence_invalid)?;
    let resulting_case_version: i64 = row
        .try_get("resulting_case_version")
        .map_err(approval_evidence_invalid)?;
    let plan_digest: Vec<u8> = row
        .try_get("plan_digest")
        .map_err(approval_evidence_invalid)?;
    let plan_digest: [u8; 32] = plan_digest.try_into().map_err(|_| {
        approval_evidence_invalid("planning link digest must contain exactly 32 bytes")
    })?;

    if source_case_version
        != i64::try_from(plan.lineage().source_case_version()).map_err(|_| {
            approval_evidence_invalid("source case version exceeds PostgreSQL range")
        })?
        || resulting_case_version
            != i64::try_from(privacy_case.version()).map_err(|_| {
                approval_evidence_invalid("resulting case version exceeds PostgreSQL range")
            })?
        || row
            .try_get::<String, _>("scope_snapshot_id")
            .map_err(approval_evidence_invalid)?
            != plan.lineage().scope_snapshot_id().as_str()
        || row
            .try_get::<String, _>("plan_id")
            .map_err(approval_evidence_invalid)?
            != plan.plan_id().as_str()
        || plan_digest != *plan.digest()
        || !row
            .try_get::<bool, _>("approval_required")
            .map_err(approval_evidence_invalid)?
        || row
            .try_get::<i64, _>("planned_at_unix_nanos")
            .map_err(approval_evidence_invalid)?
            != plan.planned_at_unix_nanos()
    {
        return Err(approval_evidence_invalid(
            "immutable planning link conflicts with case and plan evidence",
        ));
    }
    Ok(())
}

fn privacy_case_to_wire(privacy_case: &PrivacyCase) -> Result<wire::PrivacyCase, SdkError> {
    let (status, retry_resume_stage) = status_to_wire(privacy_case.status());
    Ok(wire::PrivacyCase {
        privacy_case_ref: Some(wire::PrivacyCaseRef {
            privacy_case_id: privacy_case.case_id().as_str().to_owned(),
        }),
        kind: kind_to_wire(privacy_case.kind()),
        status,
        version: i64::try_from(privacy_case.version())
            .map_err(|_| invalid_plan("approved case version exceeds wire range"))?,
        policy_version: privacy_case.policy_version().as_str().to_owned(),
        created_at_unix_ms: nanos_to_millis(
            privacy_case.created_at_unix_nanos(),
            "customer_privacy.case.created_at",
        )?,
        updated_at_unix_ms: nanos_to_millis(
            privacy_case.last_transition_at_unix_nanos(),
            "customer_privacy.case.updated_at",
        )?,
        previous_privacy_case_ref: privacy_case.previous_case_id().map(|value| {
            wire::PrivacyCaseRef {
                privacy_case_id: value.as_str().to_owned(),
            }
        }),
        subject_binding: privacy_case
            .subject_binding()
            .map(subject_binding_to_wire)
            .transpose()?,
        pending_rescope: privacy_case
            .pending_rescope()
            .map(rescope_to_wire)
            .transpose()?,
        scope_snapshot_id: privacy_case
            .scope_snapshot_id()
            .map(|value| value.as_str().to_owned())
            .unwrap_or_default(),
        privacy_action_plan_ref: privacy_case.action_plan_id().map(|value| {
            wire::PrivacyActionPlanRef {
                privacy_action_plan_id: value.as_str().to_owned(),
            }
        }),
        approval: privacy_case
            .approval()
            .map(|value| {
                Ok(wire::PrivacyApprovalEvidence {
                    approved_by_actor_id: value.approved_by.as_str().to_owned(),
                    approved_at_unix_ms: nanos_to_millis(
                        value.approved_at_unix_nanos,
                        "customer_privacy.case.approval.approved_at",
                    )?,
                })
            })
            .transpose()?,
        retry_resume_stage,
    })
}

fn subject_binding_to_wire(
    value: &SubjectBinding,
) -> Result<wire::SubjectBindingEvidence, SdkError> {
    Ok(wire::SubjectBindingEvidence {
        submitted_party_ref: Some(customer_wire::PartyRef {
            party_id: value.submitted_party_id.as_str().to_owned(),
        }),
        canonical_party_ref: Some(customer_wire::PartyRef {
            party_id: value.canonical_party_id.as_str().to_owned(),
        }),
        identity_resolution_generation: value.identity_resolution_generation,
        verification_method: verification_method_to_wire(value.verification_method),
        verified_by_actor_id: value.verified_by.as_str().to_owned(),
        verified_at_unix_ms: nanos_to_millis(
            value.verified_at_unix_nanos,
            "customer_privacy.case.subject.verified_at",
        )?,
    })
}

fn rescope_to_wire(
    value: &RescopeRequirement,
) -> Result<wire::PrivacyRescopeRequirement, SdkError> {
    Ok(wire::PrivacyRescopeRequirement {
        previous_canonical_party_ref: Some(customer_wire::PartyRef {
            party_id: value.previous_canonical_party_id.as_str().to_owned(),
        }),
        proposed_canonical_party_ref: Some(customer_wire::PartyRef {
            party_id: value.proposed_canonical_party_id.as_str().to_owned(),
        }),
        previous_identity_resolution_generation: value.previous_identity_resolution_generation,
        proposed_identity_resolution_generation: value.proposed_identity_resolution_generation,
        detected_at_unix_ms: nanos_to_millis(
            value.detected_at_unix_nanos,
            "customer_privacy.case.rescope.detected_at",
        )?,
    })
}

fn status_to_wire(value: PrivacyCaseStatus) -> (i32, Option<i32>) {
    let status = match value {
        PrivacyCaseStatus::Draft => wire::PrivacyCaseStatus::Draft,
        PrivacyCaseStatus::Submitted => wire::PrivacyCaseStatus::Submitted,
        PrivacyCaseStatus::SubjectVerified => wire::PrivacyCaseStatus::SubjectVerified,
        PrivacyCaseStatus::Scoping => wire::PrivacyCaseStatus::Scoping,
        PrivacyCaseStatus::Scoped => wire::PrivacyCaseStatus::Scoped,
        PrivacyCaseStatus::Planned => wire::PrivacyCaseStatus::Planned,
        PrivacyCaseStatus::AwaitingApproval => wire::PrivacyCaseStatus::AwaitingApproval,
        PrivacyCaseStatus::Executing => wire::PrivacyCaseStatus::Executing,
        PrivacyCaseStatus::Converging => wire::PrivacyCaseStatus::Converging,
        PrivacyCaseStatus::RescopeRequired => wire::PrivacyCaseStatus::RescopeRequired,
        PrivacyCaseStatus::FailedRetryable(stage) => {
            return (
                wire::PrivacyCaseStatus::FailedRetryable as i32,
                Some(resume_stage_to_wire(stage)),
            );
        }
        PrivacyCaseStatus::Completed => wire::PrivacyCaseStatus::Completed,
        PrivacyCaseStatus::PartiallyCompleted => wire::PrivacyCaseStatus::PartiallyCompleted,
        PrivacyCaseStatus::Denied => wire::PrivacyCaseStatus::Denied,
        PrivacyCaseStatus::Cancelled => wire::PrivacyCaseStatus::Cancelled,
        PrivacyCaseStatus::FailedTerminal => wire::PrivacyCaseStatus::FailedTerminal,
    };
    (status as i32, None)
}

fn resume_stage_to_wire(value: ResumeStage) -> i32 {
    match value {
        ResumeStage::Scoping => wire::RetryResumeStage::Scoping as i32,
        ResumeStage::Planning => wire::RetryResumeStage::Planning as i32,
        ResumeStage::Executing => wire::RetryResumeStage::Executing as i32,
        ResumeStage::Converging => wire::RetryResumeStage::Converging as i32,
    }
}

fn kind_to_wire(value: PrivacyCaseKind) -> i32 {
    match value {
        PrivacyCaseKind::Access => wire::PrivacyCaseKind::Access as i32,
        PrivacyCaseKind::PortabilityExport => wire::PrivacyCaseKind::PortabilityExport as i32,
        PrivacyCaseKind::RestrictProcessing => wire::PrivacyCaseKind::RestrictProcessing as i32,
        PrivacyCaseKind::Erasure => wire::PrivacyCaseKind::Erasure as i32,
    }
}

fn verification_method_to_wire(value: SubjectVerificationMethod) -> i32 {
    match value {
        SubjectVerificationMethod::AuthenticatedPortal => {
            wire::SubjectVerificationMethod::AuthenticatedPortal as i32
        }
        SubjectVerificationMethod::StaffAssisted => {
            wire::SubjectVerificationMethod::StaffAssisted as i32
        }
        SubjectVerificationMethod::VerifiedDocument => {
            wire::SubjectVerificationMethod::VerifiedDocument as i32
        }
        SubjectVerificationMethod::ExistingHighAssuranceIdentity => {
            wire::SubjectVerificationMethod::ExistingHighAssuranceIdentity as i32
        }
    }
}

fn ensure_exact_coordinate(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != APPROVE_PRIVACY_CASE_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str() != APPROVE_PRIVACY_CASE_CAPABILITY
        || request.context.execution.capability_version.as_str() != support::CONTRACT_VERSION
    {
        return Err(invalid_plan(
            "capability definition and request coordinate do not match",
        ));
    }
    Ok(())
}

fn positive_version(value: i64) -> Result<u64, SdkError> {
    if value <= 0 {
        return Err(SdkError::invalid_argument(
            "customer_privacy.case.expected_version",
            "Expected version must be positive.",
        ));
    }
    u64::try_from(value).map_err(|_| {
        SdkError::invalid_argument(
            "customer_privacy.case.expected_version",
            "Expected version is outside the supported range.",
        )
    })
}

fn case_ref(reference: wire::PrivacyCaseRef) -> Result<RecordRef, SdkError> {
    let id = RecordId::try_new(reference.privacy_case_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_privacy.privacy_case_ref.privacy_case_id",
            error.to_string(),
        )
    })?;
    support::record_ref(
        PRIVACY_CASE_RECORD_TYPE,
        id.as_str(),
        "customer_privacy.privacy_case_ref.privacy_case_id",
    )
}

fn nanos_to_millis(value: i64, field: &'static str) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(SdkError::invalid_argument(
            field,
            "Timestamp must not be negative.",
        ));
    }
    Ok(value / 1_000_000)
}

fn domain_error(error: PrivacyDomainError) -> SdkError {
    let category = match error {
        PrivacyDomainError::VersionConflict { .. }
        | PrivacyDomainError::InvalidTransition { .. } => ErrorCategory::Conflict,
        PrivacyDomainError::InvalidArgument { .. } => ErrorCategory::InvalidArgument,
    };
    SdkError::new(
        error.code(),
        category,
        error.retryable(),
        "The customer privacy case could not be approved.",
    )
    .with_internal_reference(error.to_string())
}

fn database_error(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_APPROVAL_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Privacy approval storage is unavailable.",
    )
    .with_internal_reference(reference.to_string())
}

fn case_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The privacy case was not found.",
    )
}

fn version_conflict(expected: u64, actual: u64) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_VERSION_CONFLICT",
        ErrorCategory::Conflict,
        true,
        "The privacy case changed before approval could be committed.",
    )
    .with_internal_reference(format!(
        "expected version {expected}, actual version {actual}"
    ))
}

fn approval_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_APPROVAL_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The privacy case is not eligible for approval.",
    )
    .with_internal_reference(reference)
}

fn approval_evidence_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_APPROVAL_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy approval evidence is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

fn approval_guard_unsupported() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_APPROVAL_GUARD_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The Customer Privacy approval guard does not support this capability.",
    )
}

fn case_state_invalid(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case could not be loaded safely.",
    )
    .with_internal_reference(error.code)
}

fn required(field: &'static str) -> SdkError {
    SdkError::invalid_argument(field, "Privacy case reference is required.")
}

fn invalid_plan(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_APPROVAL_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case approval could not be planned safely.",
    )
    .with_internal_reference(reference)
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_PRIVACY_CASE_APPROVAL_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The privacy case approval capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CausationId, CorrelationId, ExecutionContext,
        IdempotencyKey, RequestId, TenantId, TraceId,
    };
    use prost::Message;

    fn request(expected_version: i64, idempotency_key: &str, started_at: i64) -> CapabilityRequest {
        let command = wire::ApprovePrivacyCaseRequest {
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: "privacy-case-a".to_owned(),
            }),
            expected_version,
        };
        CapabilityRequest {
            context: crm_module_sdk::ModuleExecutionContext {
                module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-a").unwrap(),
                    actor_id: ActorId::try_new("privacy-approver").unwrap(),
                    request_id: RequestId::try_new("request-privacy-approve").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-privacy-approve").unwrap(),
                    causation_id: CausationId::try_new("causation-privacy-approve").unwrap(),
                    trace_id: TraceId::try_new("trace-privacy-approve").unwrap(),
                    capability_id: CapabilityId::try_new(APPROVE_PRIVACY_CASE_CAPABILITY).unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new(
                        "transaction-privacy-approve",
                    )
                    .unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: started_at,
                },
            },
            input: support::protobuf_payload(
                MODULE_ID,
                APPROVE_PRIVACY_CASE_REQUEST_SCHEMA,
                DataClass::Confidential,
                &command,
            )
            .unwrap(),
            input_hash: [41; 32],
            approval: None,
        }
    }

    fn awaiting_approval_snapshot() -> RecordSnapshot {
        let mut case = PrivacyCase::new(
            RecordId::try_new("privacy-case-a").unwrap(),
            TenantId::try_new("tenant-a").unwrap(),
            PrivacyCaseKind::Erasure,
            SchemaVersion::try_new("privacy-policy/1").unwrap(),
            1_000_000_000,
            None,
        )
        .unwrap();
        case.submit(1, 2_000_000_000).unwrap();
        case.verify_subject(
            2,
            RecordId::try_new("submitted-party").unwrap(),
            RecordId::try_new("canonical-party").unwrap(),
            1,
            SubjectVerificationMethod::VerifiedDocument,
            ActorId::try_new("privacy-verifier").unwrap(),
            3_000_000_000,
        )
        .unwrap();
        case.begin_scoping(3, 4_000_000_000).unwrap();
        case.record_scope(
            4,
            RecordId::try_new("privacy-snapshot-a").unwrap(),
            5_000_000_000,
        )
        .unwrap();
        case.record_plan(
            5,
            RecordId::try_new("privacy-plan-a").unwrap(),
            true,
            6_000_000_000,
        )
        .unwrap();
        RecordSnapshot {
            reference: privacy_case_record_ref(&case).unwrap(),
            version: i64::try_from(case.version()).unwrap(),
            payload: privacy_case_persisted_payload(&case).unwrap(),
        }
    }

    #[test]
    fn awaiting_approval_case_advances_with_exact_append_only_evidence() {
        let definition = approval_capability_definition().unwrap();
        let current = awaiting_approval_snapshot();
        let request = request(current.version, "approve-a", 7_000_000_000);
        let plan = CustomerPrivacyCaseApprovalCapabilityPlanner
            .plan(&definition, &request, Some(&current))
            .unwrap();
        assert_eq!(plan.batch.records.len(), 1);
        assert_eq!(plan.batch.events.len(), 1);
        assert_eq!(plan.batch.audits.len(), 1);
        let output = wire::ApprovePrivacyCaseResponse::decode(
            plan.output.as_ref().unwrap().bytes.as_slice(),
        )
        .unwrap()
        .privacy_case
        .unwrap();
        assert_eq!(output.status, wire::PrivacyCaseStatus::Planned as i32);
        assert_eq!(output.version, current.version + 1);
        let approval = output.approval.unwrap();
        assert_eq!(approval.approved_by_actor_id, "privacy-approver");
        assert_eq!(approval.approved_at_unix_ms, 7_000);
    }

    #[test]
    fn stale_and_ineligible_cases_fail_closed() {
        let definition = approval_capability_definition().unwrap();
        let current = awaiting_approval_snapshot();
        let stale = request(current.version - 1, "approve-stale", 7_000_000_000);
        let error = CustomerPrivacyCaseApprovalCapabilityPlanner
            .plan(&definition, &stale, Some(&current))
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_PRIVACY_VERSION_CONFLICT");
        assert!(error.retryable);

        let mut planned = privacy_case_from_snapshot(&current).unwrap();
        planned
            .approve(
                planned.version(),
                ActorId::try_new("privacy-approver").unwrap(),
                7_000_000_000,
            )
            .unwrap();
        let planned_snapshot = RecordSnapshot {
            reference: privacy_case_record_ref(&planned).unwrap(),
            version: i64::try_from(planned.version()).unwrap(),
            payload: privacy_case_persisted_payload(&planned).unwrap(),
        };
        let replay_with_new_key = request(planned_snapshot.version, "approve-again", 8_000_000_000);
        assert_eq!(
            CustomerPrivacyCaseApprovalCapabilityPlanner
                .plan(&definition, &replay_with_new_key, Some(&planned_snapshot))
                .unwrap_err()
                .code,
            "CUSTOMER_PRIVACY_INVALID_TRANSITION"
        );
    }
}
