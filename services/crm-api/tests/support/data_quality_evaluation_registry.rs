use super::data_quality_evaluation_fixture::{
    INTERNAL_MATERIALIZE, INTERNAL_STAGE, REQUEST_EVALUATION,
};
use crm_data_quality_capability_adapter::{
    ACKNOWLEDGE_FINDING_CAPABILITY, ASSIGN_FINDING_CAPABILITY,
    REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY, WAIVE_FINDING_CAPABILITY,
};
use sqlx::PgPool;

const PARTY_UPDATE_CAPABILITY: &str = "parties.party.update";
const COMPLETION_OTHER_TENANT: &str = "tenant-evaluation-other";
const POLICY_OTHER_TENANT: &str = "tenant-evaluation-policy-other";
const BOOTSTRAP_ACTOR: &str = "actor-a";
const BOOTSTRAP_CAPABILITY: &str = "test.record.mutate";

pub async fn register_evaluation_capabilities(admin: &PgPool) {
    provision_process_tenants(admin).await;
    register_data_quality(
        admin,
        REQUEST_EVALUATION,
        "crm.data_quality.v1.DataQualityService",
        "RequestPartyEvaluation",
        "e1",
        "e2",
        "medium",
    )
    .await;
    register_data_quality(
        admin,
        INTERNAL_STAGE,
        "crm.data_quality.v1.InternalDataQualityWorker",
        "StagePartyEvaluationInput",
        "e3",
        "e4",
        "medium",
    )
    .await;
    register_data_quality(
        admin,
        INTERNAL_MATERIALIZE,
        "crm.data_quality.v1.InternalDataQualityWorker",
        "MaterializePartyEvaluation",
        "e5",
        "e6",
        "medium",
    )
    .await;
    register_data_quality(
        admin,
        ASSIGN_FINDING_CAPABILITY,
        "crm.data_quality.v1.DataQualityService",
        "AssignDataQualityFinding",
        "e7",
        "e8",
        "medium",
    )
    .await;
    register_data_quality(
        admin,
        ACKNOWLEDGE_FINDING_CAPABILITY,
        "crm.data_quality.v1.DataQualityService",
        "AcknowledgeDataQualityFinding",
        "e9",
        "ea",
        "medium",
    )
    .await;
    register_data_quality(
        admin,
        WAIVE_FINDING_CAPABILITY,
        "crm.data_quality.v1.DataQualityService",
        "WaiveDataQualityFinding",
        "eb",
        "ec",
        "medium",
    )
    .await;
    register_data_quality(
        admin,
        REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY,
        "crm.data_quality.v1.DataQualityService",
        "RemediatePartyDisplayName",
        "ed",
        "ee",
        "high",
    )
    .await;
    register_capability(
        admin,
        PARTY_UPDATE_CAPABILITY,
        "crm.parties",
        "0.3.0",
        "crm.parties.v1.PartiesService",
        "UpdateParty",
        "ef",
        "f0",
        "medium",
    )
    .await;
}

async fn provision_process_tenants(admin: &PgPool) {
    for tenant_id in [COMPLETION_OTHER_TENANT, POLICY_OTHER_TENANT] {
        sqlx::query(
            "INSERT INTO crm.tenants (tenant_id, status, data_region)
             VALUES ($1, 'active', 'eu-central')
             ON CONFLICT (tenant_id) DO NOTHING",
        )
        .bind(tenant_id)
        .execute(admin)
        .await
        .unwrap_or_else(|error| panic!("provision process tenant {tenant_id}: {error}"));
        provision_bootstrap_transaction_context(admin, tenant_id).await;
    }
}

async fn provision_bootstrap_transaction_context(admin: &PgPool, tenant_id: &str) {
    let transaction_id = format!("tx-bootstrap-{tenant_id}");
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
           SELECT 1 FROM crm.business_transactions
           WHERE tenant_id = $1 AND business_transaction_id = $2
         )",
    )
    .bind(tenant_id)
    .bind(&transaction_id)
    .fetch_one(admin)
    .await
    .unwrap_or_else(|error| panic!("inspect bootstrap context for {tenant_id}: {error}"));
    if exists {
        return;
    }

    let request_id = format!("request-bootstrap-{tenant_id}");
    let evidence_key = format!("bootstrap-{tenant_id}");
    let event_id = format!("event-bootstrap-{tenant_id}");
    let audit_record_id = format!("audit-bootstrap-{tenant_id}");
    let mut transaction = admin
        .begin()
        .await
        .unwrap_or_else(|error| panic!("begin bootstrap context for {tenant_id}: {error}"));

    sqlx::query(
        "SELECT
           set_config('app.tenant_id', $1, true),
           set_config('app.actor_id', $2, true),
           set_config('app.request_id', $3, true),
           set_config('app.capability_id', $4, true),
           set_config('app.capability_version', '1.0.0', true),
           set_config('app.business_transaction_id', $5, true)",
    )
    .bind(tenant_id)
    .bind(BOOTSTRAP_ACTOR)
    .bind(&request_id)
    .bind(BOOTSTRAP_CAPABILITY)
    .bind(&transaction_id)
    .execute(&mut *transaction)
    .await
    .unwrap_or_else(|error| panic!("bind bootstrap context for {tenant_id}: {error}"));

    sqlx::query(
        "INSERT INTO crm.actors (
           tenant_id, actor_id, actor_type, status, display_name,
           last_business_transaction_id
         ) VALUES ($1, $2, 'service', 'active', $3, $4)",
    )
    .bind(tenant_id)
    .bind(BOOTSTRAP_ACTOR)
    .bind(format!("{tenant_id} bootstrap actor"))
    .bind(&transaction_id)
    .execute(&mut *transaction)
    .await
    .unwrap_or_else(|error| panic!("insert bootstrap actor for {tenant_id}: {error}"));

    sqlx::query(
        "INSERT INTO crm.idempotency_records (
           tenant_id, idempotency_scope, idempotency_key, request_hash, status,
           business_transaction_id, expires_at
         ) VALUES (
           $1, 'test.record.mutate@1.0.0', $2,
           decode(repeat('12', 32), 'hex'), 'completed', $3,
           clock_timestamp() + interval '1 day'
         )",
    )
    .bind(tenant_id)
    .bind(&evidence_key)
    .bind(&transaction_id)
    .execute(&mut *transaction)
    .await
    .unwrap_or_else(|error| panic!("insert bootstrap idempotency for {tenant_id}: {error}"));

    sqlx::query(
        "INSERT INTO crm.outbox_events (
           tenant_id, event_id, business_transaction_id, aggregate_type, aggregate_id,
           aggregate_version, event_sequence, event_type, deduplication_key, schema_id,
           schema_version, descriptor_hash, data_class, payload_encoding,
           maximum_payload_size, retention_policy_id, payload_bytes, occurred_at
         ) VALUES (
           $1, $2, $3, 'crm.actor', $4,
           1, 1, 'actor.created', $5, 'crm.actor.created.v1', '1.0.0',
           decode(repeat('21', 32), 'hex'), 'internal', 'protobuf',
           16, 'standard', decode('02', 'hex'), clock_timestamp()
         )",
    )
    .bind(tenant_id)
    .bind(event_id)
    .bind(&transaction_id)
    .bind(BOOTSTRAP_ACTOR)
    .bind(&evidence_key)
    .execute(&mut *transaction)
    .await
    .unwrap_or_else(|error| panic!("insert bootstrap outbox for {tenant_id}: {error}"));

    sqlx::query(
        "INSERT INTO crm.audit_records (
           tenant_id, audit_sequence, audit_record_id, business_transaction_id, actor_id,
           capability_id, capability_version, canonicalization_profile, previous_hash,
           record_hash, canonical_envelope, occurred_at
         ) VALUES (
           $1, 1, $2, $3, $4,
           $5, '1.0.0', 'crm.cjson/v1',
           decode(repeat('00', 32), 'hex'), decode(repeat('13', 32), 'hex'),
           convert_to($6, 'UTF8'), clock_timestamp()
         )",
    )
    .bind(tenant_id)
    .bind(audit_record_id)
    .bind(&transaction_id)
    .bind(BOOTSTRAP_ACTOR)
    .bind(BOOTSTRAP_CAPABILITY)
    .bind(format!(r#"{{"audit":"{evidence_key}"}}"#))
    .execute(&mut *transaction)
    .await
    .unwrap_or_else(|error| panic!("insert bootstrap audit for {tenant_id}: {error}"));

    sqlx::query(
        "INSERT INTO crm.business_transactions (
           tenant_id, business_transaction_id, actor_id, request_id, capability_id,
           capability_version, expected_outbox_events, expected_audit_records,
           expected_idempotency_records
         ) VALUES ($1, $2, $3, $4, $5, '1.0.0', 1, 1, 1)",
    )
    .bind(tenant_id)
    .bind(&transaction_id)
    .bind(BOOTSTRAP_ACTOR)
    .bind(request_id)
    .bind(BOOTSTRAP_CAPABILITY)
    .execute(&mut *transaction)
    .await
    .unwrap_or_else(|error| panic!("insert bootstrap transaction for {tenant_id}: {error}"));

    transaction
        .commit()
        .await
        .unwrap_or_else(|error| panic!("commit bootstrap context for {tenant_id}: {error}"));
}

#[allow(clippy::too_many_arguments)]
async fn register_data_quality(
    admin: &PgPool,
    capability_id: &str,
    service_name: &str,
    method_name: &str,
    input_byte: &str,
    output_byte: &str,
    risk_level: &str,
) {
    register_capability(
        admin,
        capability_id,
        "crm.data-quality",
        "0.1.0",
        service_name,
        method_name,
        input_byte,
        output_byte,
        risk_level,
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn register_capability(
    admin: &PgPool,
    capability_id: &str,
    owner_module_id: &str,
    owner_module_version: &str,
    service_name: &str,
    method_name: &str,
    input_byte: &str,
    output_byte: &str,
    risk_level: &str,
) {
    sqlx::query(
        "INSERT INTO crm.capability_registry (
           capability_id, capability_version, owner_module_id, owner_module_version,
           service_name, method_name, input_descriptor_hash, output_descriptor_hash,
           risk_level, idempotency_required, audit_required, approval_required,
           ai_callable, marketplace_callable, bulk_allowed, export_allowed,
           data_classes_touched
         ) VALUES (
           $1, '1.0.0', $2, $3, $4, $5,
           decode(repeat($6, 32), 'hex'), decode(repeat($7, 32), 'hex'),
           $8, true, true, false, false, false, false, false,
           ARRAY['personal']::text[]
         ) ON CONFLICT (capability_id, capability_version) DO NOTHING",
    )
    .bind(capability_id)
    .bind(owner_module_id)
    .bind(owner_module_version)
    .bind(service_name)
    .bind(method_name)
    .bind(input_byte)
    .bind(output_byte)
    .bind(risk_level)
    .execute(admin)
    .await
    .unwrap_or_else(|error| panic!("register {capability_id} audit lineage: {error}"));
}
