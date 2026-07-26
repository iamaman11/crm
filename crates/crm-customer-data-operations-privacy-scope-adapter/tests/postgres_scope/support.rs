use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_customer_data_operations::{
    CreateImportRow, ExportJobId, ImportJobId, ImportRow, MarkImportRowSucceeded,
    PartyExportExecutionOutcome, PartyExportExecutionStage, PartyExportSelectionItem,
    PartyImportKind, PreparedPartyRow, SelectedPartyId, TargetPartyId, ValidateImportRowSuccess,
    encode_export_execution_outcome_state, encode_export_execution_stage_state,
    encode_export_selection_item_state, encode_import_row_state,
};
use crm_customer_data_operations_capability_adapter::{
    EXPORT_EXECUTION_OUTCOME_RECORD_TYPE, EXPORT_EXECUTION_STAGE_RECORD_TYPE,
    IMPORT_ROW_RECORD_TYPE, MODULE_ID, export_execution_outcome_persisted_contract,
    export_execution_stage_persisted_contract, export_selection_item_persisted_contract,
    import_row_persisted_contract,
};
use crm_customer_data_operations_privacy_scope_adapter::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, INPUT_MAXIMUM_BYTES,
    INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID,
};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_identity_resolution_capability_adapter::{MERGE_CAPABILITY, MERGE_REQUEST_SCHEMA};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
    CorrelationId, DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    ModuleId, PayloadEncoding, RequestId, RetentionPolicyId, SchemaId, SchemaVersion,
    TenantId, TraceId, TypedPayload,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, CREATE_REQUEST_SCHEMA as CREATE_PARTY_SCHEMA,
};
use crm_proto_contracts::{
    crm::{
        customer::v1 as customer, customer_privacy::v1 as privacy,
        identity_resolution::v1 as identity, parties::v1 as parties,
    },
    message_descriptor_hash,
};
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;

pub(crate) const TENANT_A: &str = "tenant-a";
pub(crate) const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "privacy-worker";
const EXPORT_SELECTION_ITEM_RECORD_TYPE: &str = "customer_data.export_selection_item";

pub(crate) async fn create_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    party_id: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_parties::MODULE_ID,
                CREATE_PARTY_CAPABILITY,
                CREATE_PARTY_SCHEMA,
                tenant,
                &format!("party-{party_id}"),
                100_000_000 + i64::from(seed),
                &parties::CreatePartyRequest {
                    party_ref: Some(customer::PartyRef {
                        party_id: party_id.to_owned(),
                    }),
                    kind: parties::PartyKind::Person as i32,
                    display_name: format!("Customer Data Privacy Subject {party_id}"),
                },
            ),
        )
        .await
        .expect("create authoritative Party fixture");
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn merge_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    operation_id: &str,
    source_party_id: &str,
    survivor_party_id: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_identity_resolution::MODULE_ID,
                MERGE_CAPABILITY,
                MERGE_REQUEST_SCHEMA,
                tenant,
                &format!("merge-{operation_id}"),
                200_000_000 + i64::from(seed),
                &identity::MergePartyRequest {
                    merge_operation_ref: Some(identity::MergeOperationRef {
                        merge_operation_id: operation_id.to_owned(),
                    }),
                    source_party_ref: Some(customer::PartyRef {
                        party_id: source_party_id.to_owned(),
                    }),
                    source_party_version: 1,
                    survivor_party_ref: Some(customer::PartyRef {
                        party_id: survivor_party_id.to_owned(),
                    }),
                    survivor_party_version: 1,
                    decision_ref: format!("approval://{operation_id}"),
                    reason: "duplicate.confirmed".to_owned(),
                    survivorship: vec![identity::SurvivorshipSelection {
                        field_path: "display_name".to_owned(),
                        provenance_party_ref: Some(customer::PartyRef {
                            party_id: source_party_id.to_owned(),
                        }),
                        provenance_party_version: 1,
                        source_value_sha256: [seed; 32].to_vec(),
                        evidence_ref: format!("evidence://{operation_id}"),
                    }],
                },
            ),
        )
        .await
        .expect("create authoritative alias merge fixture");
}

pub(crate) async fn insert_valid_import_row(
    admin: &PgPool,
    tenant: &str,
    job_id: &str,
    position: u32,
    party_id: &str,
    display_name: &str,
    succeeded: bool,
) -> String {
    let mut row = ImportRow::create(CreateImportRow {
        job_id: ImportJobId::try_new(job_id).unwrap(),
        row_position: position,
        external_row_key: None,
        source_external_id: Some(format!("private-source-{job_id}-{position}")),
        occurred_at_unix_nanos: 300_000_000 + i64::from(position),
    })
    .unwrap();
    let target = TargetPartyId::try_new(party_id).unwrap();
    row.mark_valid(ValidateImportRowSuccess {
        expected_version: 1,
        prepared_party: PreparedPartyRow::try_new(
            target.clone(),
            PartyImportKind::Person,
            display_name,
        )
        .unwrap(),
        occurred_at_unix_nanos: 310_000_000 + i64::from(position),
    })
    .unwrap();
    if succeeded {
        row.mark_succeeded(MarkImportRowSucceeded {
            expected_version: 2,
            target_party_id: target,
            occurred_at_unix_nanos: 320_000_000 + i64::from(position),
        })
        .unwrap();
    }
    let record_id = row.row_id().as_str().to_owned();
    insert_record(
        admin,
        tenant,
        IMPORT_ROW_RECORD_TYPE,
        &record_id,
        row.version(),
        import_row_persisted_contract(),
        encode_import_row_state(&row).unwrap(),
    )
    .await;
    record_id
}

pub(crate) async fn insert_pending_import_row(
    admin: &PgPool,
    tenant: &str,
    job_id: &str,
    position: u32,
) -> String {
    let row = ImportRow::create(CreateImportRow {
        job_id: ImportJobId::try_new(job_id).unwrap(),
        row_position: position,
        external_row_key: None,
        source_external_id: Some(format!("private-unrelated-{job_id}-{position}")),
        occurred_at_unix_nanos: 330_000_000 + i64::from(position),
    })
    .unwrap();
    let record_id = row.row_id().as_str().to_owned();
    insert_record(
        admin,
        tenant,
        IMPORT_ROW_RECORD_TYPE,
        &record_id,
        row.version(),
        import_row_persisted_contract(),
        encode_import_row_state(&row).unwrap(),
    )
    .await;
    record_id
}

pub(crate) struct ExportEvidenceIds {
    pub selection: String,
    pub stage: String,
    pub outcome: String,
}

pub(crate) async fn insert_export_evidence(
    admin: &PgPool,
    tenant: &str,
    job_id: &str,
    position: u32,
    party_id: &str,
    private_row: &str,
    private_chunk_sha256: &str,
) -> ExportEvidenceIds {
    let job = ExportJobId::try_new(job_id).unwrap();
    let selection = PartyExportSelectionItem::create(
        job.clone(),
        position,
        SelectedPartyId::try_new(party_id).unwrap(),
        1,
        400_000_000 + i64::from(position),
    )
    .unwrap();
    let stage = PartyExportExecutionStage::emitted(
        job.clone(),
        position,
        private_row.to_owned(),
        2,
        410_000_000 + i64::from(position),
    )
    .unwrap();
    let outcome = PartyExportExecutionOutcome::emitted(
        job,
        position,
        position,
        private_chunk_sha256,
        u64::try_from(private_row.len()).unwrap(),
        2,
        420_000_000 + i64::from(position),
    )
    .unwrap();

    let ids = ExportEvidenceIds {
        selection: selection.item_id().as_str().to_owned(),
        stage: stage.stage_id().as_str().to_owned(),
        outcome: outcome.outcome_id().as_str().to_owned(),
    };
    insert_record(
        admin,
        tenant,
        EXPORT_SELECTION_ITEM_RECORD_TYPE,
        &ids.selection,
        selection.version(),
        export_selection_item_persisted_contract(),
        encode_export_selection_item_state(&selection).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        EXPORT_EXECUTION_STAGE_RECORD_TYPE,
        &ids.stage,
        1,
        export_execution_stage_persisted_contract(),
        encode_export_execution_stage_state(&stage).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        EXPORT_EXECUTION_OUTCOME_RECORD_TYPE,
        &ids.outcome,
        1,
        export_execution_outcome_persisted_contract(),
        encode_export_execution_outcome_state(&outcome).unwrap(),
    )
    .await;
    ids
}

async fn insert_record(
    admin: &PgPool,
    tenant: &str,
    record_type: &str,
    record_id: &str,
    version: i64,
    contract: PersistedPayloadContract<'_>,
    payload_bytes: Vec<u8>,
) {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin isolated Customer Data fixture transaction");
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .expect("disable production triggers for isolated Customer Data fixture");
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, version, owner_module_id,
          schema_id, schema_version, descriptor_hash, data_class, payload_encoding,
          maximum_payload_size, retention_policy_id, payload_bytes,
          last_business_transaction_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'personal', 'json', $9, $10, $11, $12)
        "#,
    )
    .bind(tenant)
    .bind(record_type)
    .bind(record_id)
    .bind(version)
    .bind(contract.owner)
    .bind(contract.schema_id)
    .bind(contract.schema_version)
    .bind(contract.descriptor_hash.as_slice())
    .bind(i64::try_from(contract.maximum_size_bytes).unwrap())
    .bind(contract.retention_policy_id)
    .bind(payload_bytes)
    .bind(format!("fixture-{record_id}"))
    .execute(&mut *transaction)
    .await
    .expect("insert exact authoritative Customer Data fixture");
    sqlx::query("SET LOCAL session_replication_role = 'origin'")
        .execute(&mut *transaction)
        .await
        .expect("restore production trigger mode after Customer Data fixture");
    transaction
        .commit()
        .await
        .expect("commit isolated Customer Data fixture transaction");
}

#[allow(clippy::too_many_arguments)]
fn capability_request<M: Message>(
    module_id: &str,
    capability_id: &str,
    input_schema: &str,
    tenant: &str,
    identity: &str,
    started_at: i64,
    command: &M,
) -> CapabilityRequest {
    let bytes = command.encode_to_vec();
    let input = TypedPayload {
        owner: ModuleId::try_new(module_id).unwrap(),
        schema_id: SchemaId::try_new(input_schema).unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: message_descriptor_hash(input_schema),
        data_class: DataClass::Personal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: support::MAX_PROTOBUF_BYTES,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes,
    };
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(module_id).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
                causation_id: CausationId::try_new(format!("causation-{identity}")).unwrap(),
                trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
                capability_id: CapabilityId::try_new(capability_id).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(format!("{identity}-key")).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!("{identity}-tx"))
                    .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: started_at,
            },
        },
        input_hash: Sha256::digest(&input.bytes).into(),
        input,
        approval: None,
    }
}

pub(crate) fn scope_request(
    tenant: &str,
    party_id: &str,
    generation: u64,
    page_size: u32,
    cursor: &str,
    identity: &str,
) -> QueryRequest {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    let wire = privacy::CustomerDataPrivacyScopeContributionRequest {
        contribution: Some(privacy::PrivacyScopeContributionRequestEnvelope {
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: format!("privacy-case-{identity}"),
                tenant_id: tenant.to_owned(),
                canonical_party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                identity_resolution_generation: generation,
                registry_version: CANONICAL_SCOPE_REGISTRY_VERSION.to_owned(),
                registry_digest_sha256: registry.digest().to_vec(),
                purpose_code: "PRIVACY_ERASURE_SCOPE".to_owned(),
                effective_request_at_unix_ms: 1_000,
            }),
            page_size,
            cursor: cursor.to_owned(),
        }),
    };
    let bytes = wire.encode_to_vec();
    let input_hash: [u8; 32] = Sha256::digest(&bytes).into();
    QueryRequest {
        owner_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        context: QueryExecutionContext {
            tenant_id: TenantId::try_new(tenant).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
            capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
            capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
            schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
            request_started_at_unix_nanos: 2_000_000_000,
        },
        input: TypedPayload {
            owner: ModuleId::try_new(MODULE_ID).unwrap(),
            schema_id: SchemaId::try_new(INPUT_SCHEMA_ID).unwrap(),
            schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
            descriptor_hash: message_descriptor_hash(INPUT_SCHEMA_ID),
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
            retention_policy_id: RetentionPolicyId::try_new(INPUT_RETENTION_POLICY_ID).unwrap(),
            bytes,
        },
        input_hash,
    }
}

pub(crate) async fn current_generation(admin: &PgPool, tenant: &str) -> u64 {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin topology generation evidence transaction");
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant)
        .execute(&mut *transaction)
        .await
        .expect("bind tenant for topology generation evidence");
    let generation: i64 =
        sqlx::query_scalar("SELECT crm.current_identity_resolution_generation($1)")
            .bind(tenant)
            .fetch_one(&mut *transaction)
            .await
            .expect("read current topology generation");
    transaction.commit().await.unwrap();
    u64::try_from(generation).unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WriteSurfaceCounts {
    records: i64,
    relationships: i64,
    business_transactions: i64,
    idempotency_records: i64,
    outbox_events: i64,
    outbox_delivery: i64,
    audit_heads: i64,
    audit_records: i64,
}

pub(crate) async fn write_surface_counts(pool: &PgPool) -> WriteSurfaceCounts {
    WriteSurfaceCounts {
        records: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.records")
            .fetch_one(pool)
            .await
            .expect("count CRM records"),
        relationships: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.relationships")
            .fetch_one(pool)
            .await
            .expect("count CRM relationships"),
        business_transactions: sqlx::query_scalar(
            "SELECT count(*)::bigint FROM crm.business_transactions",
        )
        .fetch_one(pool)
        .await
        .expect("count CRM business transactions"),
        idempotency_records: sqlx::query_scalar(
            "SELECT count(*)::bigint FROM crm.idempotency_records",
        )
        .fetch_one(pool)
        .await
        .expect("count CRM idempotency records"),
        outbox_events: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_events")
            .fetch_one(pool)
            .await
            .expect("count CRM outbox events"),
        outbox_delivery: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_delivery")
            .fetch_one(pool)
            .await
            .expect("count CRM outbox delivery rows"),
        audit_heads: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_heads")
            .fetch_one(pool)
            .await
            .expect("count CRM audit heads"),
        audit_records: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_records")
            .fetch_one(pool)
            .await
            .expect("count CRM audit records"),
    }
}

pub(crate) async fn corrupt_selection_metadata(admin: &PgPool, selection_id: &str) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        r#"
        UPDATE crm.records
        SET schema_id = 'crm.customer_data_operations.export_selection_item.state.invalid'
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND record_type = $3
          AND record_id = $4
        "#,
    )
    .bind(TENANT_A)
    .bind(MODULE_ID)
    .bind(EXPORT_SELECTION_ITEM_RECORD_TYPE)
    .bind(selection_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'origin'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
}
