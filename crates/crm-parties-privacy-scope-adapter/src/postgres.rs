use crate::contract::validate_definition;
use crate::errors::{
    database_unavailable, map_lineage_error, row_decode_error, stored_state_invalid,
    subject_not_found,
};
use crate::request::{validate_request_contract, validate_wire_request};
use crate::response::{build_response, typed_output};
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::PostgresDataStore;
use crm_identity_resolution_topology_composition::prove_canonical_party_in_transaction;
use crm_module_sdk::{
    DataClass, ModuleId, PayloadEncoding, PortFuture, RecordRef, RecordSnapshot, RecordType,
    RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
};
use crm_parties::{
    MODULE_ID, PARTY_STATE_MAXIMUM_BYTES, PARTY_STATE_RETENTION_POLICY_ID, PARTY_STATE_SCHEMA_ID,
    PARTY_STATE_SCHEMA_VERSION, party_state_descriptor_hash,
};
use crm_parties_capability_adapter::{RECORD_TYPE as PARTY_RECORD_TYPE, party_from_snapshot};
use crm_query_runtime::{QueryExecutionResult, QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::Row;

#[derive(Clone)]
pub struct PartiesPrivacyScopeQueryAdapter {
    store: PostgresDataStore,
}

impl std::fmt::Debug for PartiesPrivacyScopeQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartiesPrivacyScopeQueryAdapter")
            .field("store", &"PostgresDataStore")
            .finish()
    }
}

impl PartiesPrivacyScopeQueryAdapter {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    async fn execute_query(
        &self,
        definition: &CapabilityDefinition,
        request: QueryRequest,
    ) -> Result<QueryExecutionResult, SdkError> {
        validate_definition(definition)?;
        validate_request_contract(&request)?;
        let validated = validate_wire_request(&request.context, &request.input.bytes)?;

        let mut transaction = self
            .store
            .begin_bound_read_transaction(&request.context.tenant_id)
            .await?;
        prove_canonical_party_in_transaction(
            &mut transaction,
            &request.context.tenant_id,
            &validated.canonical_party,
            &validated.canonical_party,
            validated.identity_resolution_generation,
        )
        .await
        .map_err(map_lineage_error)?;

        let row = sqlx::query(
            r#"
            SELECT
              version,
              owner_module_id,
              schema_id,
              schema_version,
              descriptor_hash,
              data_class,
              payload_encoding,
              maximum_payload_size,
              retention_policy_id,
              payload_bytes
            FROM crm.records
            WHERE tenant_id = $1
              AND owner_module_id = $2
              AND record_type = $3
              AND record_id = $4
              AND deleted_at IS NULL
            FOR SHARE
            "#,
        )
        .bind(request.context.tenant_id.as_str())
        .bind(MODULE_ID)
        .bind(PARTY_RECORD_TYPE)
        .bind(validated.canonical_party_id.as_str())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_unavailable)?
        .ok_or_else(subject_not_found)?;

        let snapshot = strict_party_snapshot(&validated.canonical_party_id, row)?;
        let party = party_from_snapshot(&snapshot)?;
        let resource_version = u64::try_from(party.version())
            .map_err(|_| stored_state_invalid("persisted Party version must be positive"))?;
        if party.party_id().as_str() != validated.canonical_party_id.as_str() {
            return Err(stored_state_invalid(
                "persisted Party identity does not match the locked record",
            ));
        }

        let response = build_response(&validated, resource_version);
        let output = typed_output(response.encode_to_vec())?;
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(QueryExecutionResult { output })
    }
}

impl QueryExecutor for PartiesPrivacyScopeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move { self.execute_query(definition, request).await })
    }
}

fn strict_party_snapshot(
    canonical_party_id: &crm_module_sdk::RecordId,
    row: sqlx::postgres::PgRow,
) -> Result<RecordSnapshot, SdkError> {
    let version: i64 = row.try_get("version").map_err(row_decode_error)?;
    let owner_module_id: String = row.try_get("owner_module_id").map_err(row_decode_error)?;
    let stored_schema_id: String = row.try_get("schema_id").map_err(row_decode_error)?;
    let stored_schema_version: String = row.try_get("schema_version").map_err(row_decode_error)?;
    let stored_descriptor_hash: Vec<u8> =
        row.try_get("descriptor_hash").map_err(row_decode_error)?;
    let stored_data_class: String = row.try_get("data_class").map_err(row_decode_error)?;
    let stored_encoding: String = row.try_get("payload_encoding").map_err(row_decode_error)?;
    let stored_maximum_size: i64 = row
        .try_get("maximum_payload_size")
        .map_err(row_decode_error)?;
    let stored_retention: String = row
        .try_get("retention_policy_id")
        .map_err(row_decode_error)?;
    let payload_bytes: Vec<u8> = row.try_get("payload_bytes").map_err(row_decode_error)?;

    let expected_descriptor_hash = party_state_descriptor_hash();
    if version <= 0
        || owner_module_id != MODULE_ID
        || stored_schema_id != PARTY_STATE_SCHEMA_ID
        || stored_schema_version != PARTY_STATE_SCHEMA_VERSION
        || stored_descriptor_hash.as_slice() != expected_descriptor_hash
        || stored_data_class != "personal"
        || stored_encoding != "json"
        || stored_maximum_size != PARTY_STATE_MAXIMUM_BYTES as i64
        || stored_retention != PARTY_STATE_RETENTION_POLICY_ID
    {
        return Err(stored_state_invalid(
            "persisted Party metadata does not match the canonical state contract",
        ));
    }

    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: configured(RecordType::try_new(PARTY_RECORD_TYPE))?,
            record_id: canonical_party_id.clone(),
        },
        version,
        payload: TypedPayload {
            owner: configured(ModuleId::try_new(MODULE_ID))?,
            schema_id: configured(SchemaId::try_new(PARTY_STATE_SCHEMA_ID))?,
            schema_version: configured(SchemaVersion::try_new(PARTY_STATE_SCHEMA_VERSION))?,
            descriptor_hash: expected_descriptor_hash,
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: PARTY_STATE_MAXIMUM_BYTES,
            retention_policy_id: configured(RetentionPolicyId::try_new(
                PARTY_STATE_RETENTION_POLICY_ID,
            ))?,
            bytes: payload_bytes,
        },
    })
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    crate::errors::configured(value)
}
