use super::data_quality_evaluation_fixture::{INTERNAL_STAGE, REQUEST_EVALUATION};
use sqlx::PgPool;

pub async fn register_evaluation_capabilities(admin: &PgPool) {
    register(
        admin,
        REQUEST_EVALUATION,
        "crm.data_quality.v1.DataQualityService",
        "RequestPartyEvaluation",
        "e1",
        "e2",
    )
    .await;
    register(
        admin,
        INTERNAL_STAGE,
        "crm.data_quality.v1.InternalDataQualityWorker",
        "StagePartyEvaluationInput",
        "e3",
        "e4",
    )
    .await;
}

async fn register(
    admin: &PgPool,
    capability_id: &str,
    service_name: &str,
    method_name: &str,
    input_byte: &str,
    output_byte: &str,
) {
    sqlx::query(
        "INSERT INTO crm.capability_registry (
           capability_id, capability_version, owner_module_id, owner_module_version,
           service_name, method_name, input_descriptor_hash, output_descriptor_hash,
           risk_level, idempotency_required, audit_required, approval_required,
           ai_callable, marketplace_callable, bulk_allowed, export_allowed,
           data_classes_touched
         ) VALUES (
           $1, '1.0.0', 'crm.data-quality', '0.1.0', $2, $3,
           decode(repeat($4, 32), 'hex'), decode(repeat($5, 32), 'hex'),
           'medium', true, true, false, false, false, false, false,
           ARRAY['personal']::text[]
         ) ON CONFLICT (capability_id, capability_version) DO NOTHING",
    )
    .bind(capability_id)
    .bind(service_name)
    .bind(method_name)
    .bind(input_byte)
    .bind(output_byte)
    .execute(admin)
    .await
    .unwrap_or_else(|error| panic!("register {capability_id} audit lineage: {error}"));
}
