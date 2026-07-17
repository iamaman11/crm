use super::data_quality_evaluation_fixture::{
    INTERNAL_MATERIALIZE, INTERNAL_STAGE, REQUEST_EVALUATION,
};
use crm_data_quality_capability_adapter::{
    ACKNOWLEDGE_FINDING_CAPABILITY, ASSIGN_FINDING_CAPABILITY,
    REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY, WAIVE_FINDING_CAPABILITY,
};
use sqlx::PgPool;

const PARTY_UPDATE_CAPABILITY: &str = "parties.party.update";

pub async fn register_evaluation_capabilities(admin: &PgPool) {
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
