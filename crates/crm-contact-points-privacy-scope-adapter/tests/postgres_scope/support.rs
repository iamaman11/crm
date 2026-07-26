use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_contact_points_capability_adapter::{
    CREATE_CAPABILITY as CREATE_CONTACT_POINT_CAPABILITY,
    CREATE_REQUEST_SCHEMA as CREATE_CONTACT_POINT_SCHEMA,
    UPDATE_CAPABILITY as UPDATE_CONTACT_POINT_CAPABILITY,
    UPDATE_REQUEST_SCHEMA as UPDATE_CONTACT_POINT_SCHEMA,
    VERIFY_CAPABILITY as VERIFY_CONTACT_POINT_CAPABILITY,
    VERIFY_REQUEST_SCHEMA as VERIFY_CONTACT_POINT_SCHEMA,
};
use crm_contact_points_privacy_scope_adapter::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, INPUT_MAXIMUM_BYTES,
    INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID,
};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_identity_resolution_capability_adapter::{
    CANONICAL_REDIRECT_PARTY_RECORD_TYPE, CANONICAL_REDIRECT_RELATIONSHIP_TYPE,
    MODULE_ID as IDENTITY_RESOLUTION_MODULE_ID,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, CREATE_REQUEST_SCHEMA as CREATE_PARTY_SCHEMA,
};
use crm_proto_contracts::{
    crm::{
        contact_points::v1 as contact_points, customer::v1 as customer,
        customer_privacy::v1 as privacy, parties::v1 as parties,
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
                500_000_000 + i64::from(seed),
                seed,
                &parties::CreatePartyRequest {
                    party_ref: Some(customer::PartyRef {
                        party_id: party_id.to_owned(),
                    }),
                    kind: parties::PartyKind::Person as i32,
                    display_name: format!("Contact Point Scope Subject {party_id}"),
                },
            ),
        )
        .await
        .expect("create authoritative Party fixture");
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_contact_point(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    contact_point_id: &str,
    party_id: &str,
    kind: contact_points::ContactPointKind,
    value: &str,
    preferred: bool,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_contact_points::MODULE_ID,
                CREATE_CONTACT_POINT_CAPABILITY,
                CREATE_CONTACT_POINT_SCHEMA,
                tenant,
                &format!("contact-point-{contact_point_id}"),
                600_000_000 + i64::from(seed),
                seed,
                &contact_points::CreateContactPointRequest {
                    contact_point_ref: Some(customer::ContactPointRef {
                        contact_point_id: contact_point_id.to_owned(),
                    }),
                    party_ref: Some(customer::PartyRef {
                        party_id: party_id.to_owned(),
                    }),
                    kind: kind as i32,
                    value: value.to_owned(),
                    preferred,
                    valid_from: None,
                    valid_until: None,
                },
            ),
        )
        .await
        .expect("create authoritative Contact Point fixture");
}

pub(crate) async fn verify_contact_point(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    contact_point_id: &str,
    expected_version: i64,
    evidence_ref: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_contact_points::MODULE_ID,
                VERIFY_CONTACT_POINT_CAPABILITY,
                VERIFY_CONTACT_POINT_SCHEMA,
                tenant,
                &format!("verify-contact-point-{contact_point_id}"),
                700_000_000 + i64::from(seed),
                seed,
                &contact_points::VerifyContactPointRequest {
                    contact_point_ref: Some(customer::ContactPointRef {
                        contact_point_id: contact_point_id.to_owned(),
                    }),
                    expected_version,
                    evidence_ref: evidence_ref.to_owned(),
                },
            ),
        )
        .await
        .expect("verify authoritative Contact Point fixture");
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn update_contact_point_status(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    contact_point_id: &str,
    expected_version: i64,
    value: &str,
    status: contact_points::ContactPointStatus,
    preferred: bool,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_contact_points::MODULE_ID,
                UPDATE_CONTACT_POINT_CAPABILITY,
                UPDATE_CONTACT_POINT_SCHEMA,
                tenant,
                &format!("update-contact-point-{contact_point_id}"),
                800_000_000 + i64::from(seed),
                seed,
                &contact_points::UpdateContactPointRequest {
                    contact_point_ref: Some(customer::ContactPointRef {
                        contact_point_id: contact_point_id.to_owned(),
                    }),
                    expected_version,
                    value: value.to_owned(),
                    status: status as i32,
                    preferred,
                    valid_from: None,
                    valid_until: None,
                },
            ),
        )
        .await
        .expect("update authoritative Contact Point fixture");
}

#[allow(clippy::too_many_arguments)]
fn capability_request<M: Message>(
    module_id: &str,
    capability_id: &str,
    input_schema: &str,
    tenant: &str,
    identity: &str,
    started_at: i64,
    hash: u8,
    command: &M,
) -> CapabilityRequest {
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
        input: support::protobuf_payload(module_id, input_schema, DataClass::Personal, command)
            .unwrap(),
        input_hash: [hash; 32],
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
    let wire = privacy::ContactPointsPrivacyScopeContributionRequest {
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
        owner_module_id: ModuleId::try_new(crm_contact_points::MODULE_ID).unwrap(),
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
            owner: ModuleId::try_new(crm_contact_points::MODULE_ID).unwrap(),
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

pub(crate) async fn insert_canonical_redirect(admin: &PgPool, source: &str, target: &str) {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin canonical redirect fixture transaction");
    let transaction_id: String = sqlx::query_scalar(
        r#"
        SELECT last_business_transaction_id
        FROM crm.records
        WHERE tenant_id = $1
          AND record_type = $2
          AND record_id = $3
        "#,
    )
    .bind(TENANT_A)
    .bind(CANONICAL_REDIRECT_PARTY_RECORD_TYPE)
    .bind(source)
    .fetch_one(&mut *transaction)
    .await
    .expect("read redirect source transaction");
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', $3, true),
          set_config('app.capability_id', $4, true),
          set_config('app.capability_version', $5, true),
          set_config('app.business_transaction_id', $6, true)
        "#,
    )
    .bind(TENANT_A)
    .bind("actor-a")
    .bind("request-canonical-redirect")
    .bind("identity-resolution.candidate.merge")
    .bind("1.0.0")
    .bind(&transaction_id)
    .execute(&mut *transaction)
    .await
    .expect("bind complete canonical redirect fixture context");
    sqlx::query(
        r#"
        INSERT INTO crm.relationships (
          tenant_id,
          owner_module_id,
          relationship_type,
          source_record_type,
          source_record_id,
          target_record_type,
          target_record_id,
          version,
          schema_id,
          schema_version,
          descriptor_hash,
          data_class,
          payload_encoding,
          maximum_payload_size,
          retention_policy_id,
          payload_bytes,
          typed_projection,
          last_business_transaction_id
        )
        VALUES (
          $1, $2, $3, $4, $5, $4, $6, 1,
          'crm.identity-resolution.canonical-redirect',
          '1.0.0',
          decode(repeat('a7', 32), 'hex'),
          'personal',
          'json',
          1024,
          'crm.identity-resolution.merge-operation',
          '{}'::text::bytea,
          '{}'::jsonb,
          $7
        )
        "#,
    )
    .bind(TENANT_A)
    .bind(IDENTITY_RESOLUTION_MODULE_ID)
    .bind(CANONICAL_REDIRECT_RELATIONSHIP_TYPE)
    .bind(CANONICAL_REDIRECT_PARTY_RECORD_TYPE)
    .bind(source)
    .bind(target)
    .bind(transaction_id)
    .execute(&mut *transaction)
    .await
    .expect("insert authoritative canonical redirect fixture");
    transaction
        .commit()
        .await
        .expect("commit authoritative canonical redirect fixture");
}

pub(crate) async fn corrupt_contact_point_metadata(admin: &PgPool, contact_point_id: &str) {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin isolated Contact Point metadata corruption transaction");
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', $3, true),
          set_config('app.capability_id', $4, true),
          set_config('app.capability_version', $5, true),
          set_config('app.business_transaction_id', $6, true)
        "#,
    )
    .bind(TENANT_A)
    .bind("actor-a")
    .bind("request-corrupt-contact-point")
    .bind(CREATE_CONTACT_POINT_CAPABILITY)
    .bind("1.0.0")
    .bind(format!("contact-point-{contact_point_id}-tx"))
    .execute(&mut *transaction)
    .await
    .expect("bind isolated Contact Point metadata corruption context");
    sqlx::query(
        r#"
        UPDATE crm.records
        SET schema_id = 'crm.contact-points.contact-point.state.invalid'
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND record_type = $3
          AND record_id = $4
        "#,
    )
    .bind(TENANT_A)
    .bind(crm_contact_points::MODULE_ID)
    .bind(crm_contact_points_capability_adapter::RECORD_TYPE)
    .bind(contact_point_id)
    .execute(&mut *transaction)
    .await
    .expect("corrupt isolated Contact Point metadata fixture");
    transaction
        .commit()
        .await
        .expect("commit isolated Contact Point metadata corruption fixture");
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
