use crate::contract::validate_definition;
use crate::errors::{
    database_unavailable, lineage_invalid, row_decode_error, stored_state_invalid,
};
use crate::request::{ValidatedRequest, validate_request_contract, validate_wire_request};
use crate::response::{VerifiedConsentResource, build_response, typed_output};
use crm_capability_runtime::CapabilityDefinition;
use crm_consents::{
    CONSENT_AUTHORIZATION_STATE_MAXIMUM_BYTES, CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID,
    CONSENT_AUTHORIZATION_STATE_SCHEMA_ID, CONSENT_AUTHORIZATION_STATE_SCHEMA_VERSION, MODULE_ID,
    consent_authorization_state_descriptor_hash,
};
use crm_consents_capability_adapter::{
    PARTY_AUTHORIZATION_RELATIONSHIP_TYPE, PARTY_AUTHORIZATION_SOURCE_RECORD_TYPE, RECORD_TYPE,
    consent_authorization_from_snapshot,
};
use crm_core_data::{BoundReadTransaction, PostgresDataStore};
use crm_identity_resolution_capability_adapter::{
    CANONICAL_REDIRECT_PARTY_RECORD_TYPE, CANONICAL_REDIRECT_RELATIONSHIP_TYPE,
    MODULE_ID as IDENTITY_RESOLUTION_MODULE_ID,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId,
    TypedPayload,
};
use crm_parties::MODULE_ID as PARTIES_MODULE_ID;
use crm_parties_capability_adapter::RECORD_TYPE as PARTY_RECORD_TYPE;
use crm_query_runtime::{QueryExecutionResult, QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::Row;

#[derive(Clone)]
pub struct ConsentsPrivacyScopeQueryAdapter {
    store: PostgresDataStore,
}

impl std::fmt::Debug for ConsentsPrivacyScopeQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConsentsPrivacyScopeQueryAdapter")
            .field("store", &"PostgresDataStore")
            .finish()
    }
}

impl ConsentsPrivacyScopeQueryAdapter {
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
        prove_canonical_claim(
            &mut transaction,
            &request.context.tenant_id,
            &validated.canonical_party_id,
            validated.identity_resolution_generation,
        )
        .await?;
        let page = read_consent_page(&mut transaction, &validated).await?;
        let response = build_response(
            &validated,
            &page.resources,
            page.scanned_resource_count,
            page.has_more,
        )?;
        let output = typed_output(response.encode_to_vec())?;
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(QueryExecutionResult { output })
    }
}

impl QueryExecutor for ConsentsPrivacyScopeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move { self.execute_query(definition, request).await })
    }
}

struct ConsentPage {
    resources: Vec<VerifiedConsentResource>,
    scanned_resource_count: u64,
    has_more: bool,
}

async fn read_consent_page(
    transaction: &mut BoundReadTransaction<'_>,
    request: &ValidatedRequest,
) -> Result<ConsentPage, SdkError> {
    let fetch_limit = i64::from(request.page_size) + 1;
    let after_record_id = request.after_record_id.as_ref().map(RecordId::as_str);
    let rows = sqlx::query(
        r#"
        SELECT
          target.record_id,
          target.version,
          target.owner_module_id,
          target.schema_id,
          target.schema_version,
          target.descriptor_hash,
          target.data_class,
          target.payload_encoding,
          target.maximum_payload_size,
          target.retention_policy_id,
          target.payload_bytes
        FROM crm.relationships AS relation
        JOIN crm.records AS target
          ON target.tenant_id = relation.tenant_id
         AND target.record_type = relation.target_record_type
         AND target.record_id = relation.target_record_id
        WHERE relation.tenant_id = $1
          AND relation.owner_module_id = $2
          AND relation.relationship_type = $3
          AND relation.source_record_type = $4
          AND relation.source_record_id = $5
          AND relation.target_record_type = $6
          AND target.owner_module_id = $2
          AND target.record_type = $6
          AND target.deleted_at IS NULL
          AND ($7::text IS NULL OR target.record_id > $7)
        ORDER BY target.record_id ASC
        LIMIT $8
        "#,
    )
    .bind(request.lineage.tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(PARTY_AUTHORIZATION_RELATIONSHIP_TYPE)
    .bind(PARTY_AUTHORIZATION_SOURCE_RECORD_TYPE)
    .bind(request.canonical_party_id.as_str())
    .bind(RECORD_TYPE)
    .bind(after_record_id)
    .bind(fetch_limit)
    .fetch_all(&mut ***transaction)
    .await
    .map_err(database_unavailable)?;

    let scanned_resource_count = u64::try_from(rows.len()).map_err(|_| {
        stored_state_invalid("Consent privacy scope scan count does not fit in u64")
    })?;
    let mut resources = rows
        .into_iter()
        .map(|row| strict_consent_resource(&request.canonical_party_id, row))
        .collect::<Result<Vec<_>, SdkError>>()?;
    let has_more = resources.len() > request.page_size as usize;
    if has_more {
        resources.pop();
    }
    Ok(ConsentPage {
        resources,
        scanned_resource_count,
        has_more,
    })
}

async fn prove_canonical_claim(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
    claimed_generation: u64,
) -> Result<(), SdkError> {
    sqlx::query("SELECT crm.lock_identity_resolution_topology($1)")
        .bind(tenant_id.as_str())
        .execute(&mut ***transaction)
        .await
        .map_err(database_unavailable)?;

    let actual_generation: i64 =
        sqlx::query_scalar("SELECT crm.current_identity_resolution_generation($1)")
            .bind(tenant_id.as_str())
            .fetch_one(&mut ***transaction)
            .await
            .map_err(database_unavailable)?;
    let actual_generation = u64::try_from(actual_generation).map_err(|_| {
        lineage_invalid(
            ErrorCategory::Conflict,
            true,
            "authoritative Identity Resolution generation is not positive",
        )
    })?;
    if actual_generation != claimed_generation {
        return Err(lineage_invalid(
            ErrorCategory::Conflict,
            true,
            "claimed Identity Resolution generation is stale",
        ));
    }

    let party_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
          SELECT 1
          FROM crm.records
          WHERE tenant_id = $1
            AND owner_module_id = $2
            AND record_type = $3
            AND record_id = $4
            AND deleted_at IS NULL
        )
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(PARTIES_MODULE_ID)
    .bind(PARTY_RECORD_TYPE)
    .bind(canonical_party_id.as_str())
    .fetch_one(&mut ***transaction)
    .await
    .map_err(database_unavailable)?;
    if !party_exists {
        return Err(lineage_invalid(
            ErrorCategory::NotFound,
            false,
            "claimed canonical Party is not visible in the tenant snapshot",
        ));
    }

    let outgoing_redirects: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)::bigint
        FROM crm.relationships
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND relationship_type = $3
          AND source_record_type = $4
          AND source_record_id = $5
          AND target_record_type = $4
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(IDENTITY_RESOLUTION_MODULE_ID)
    .bind(CANONICAL_REDIRECT_RELATIONSHIP_TYPE)
    .bind(CANONICAL_REDIRECT_PARTY_RECORD_TYPE)
    .bind(canonical_party_id.as_str())
    .fetch_one(&mut ***transaction)
    .await
    .map_err(database_unavailable)?;
    if outgoing_redirects != 0 {
        return Err(lineage_invalid(
            ErrorCategory::Conflict,
            false,
            "claimed Party has an active canonical redirect",
        ));
    }
    Ok(())
}

fn strict_consent_resource(
    canonical_party_id: &RecordId,
    row: sqlx::postgres::PgRow,
) -> Result<VerifiedConsentResource, SdkError> {
    let record_id = RecordId::try_new(
        row.try_get::<String, _>("record_id")
            .map_err(row_decode_error)?,
    )
    .map_err(|error| stored_state_invalid(error.to_string()))?;
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

    let expected_descriptor_hash = consent_authorization_state_descriptor_hash();
    if version <= 0
        || owner_module_id != MODULE_ID
        || stored_schema_id != CONSENT_AUTHORIZATION_STATE_SCHEMA_ID
        || stored_schema_version != CONSENT_AUTHORIZATION_STATE_SCHEMA_VERSION
        || stored_descriptor_hash.as_slice() != expected_descriptor_hash
        || stored_data_class != "personal"
        || stored_encoding != "json"
        || stored_maximum_size != CONSENT_AUTHORIZATION_STATE_MAXIMUM_BYTES as i64
        || stored_retention != CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID
    {
        return Err(stored_state_invalid(
            "persisted Consent metadata does not match the canonical state contract",
        ));
    }

    let snapshot = RecordSnapshot {
        reference: RecordRef {
            record_type: configured(RecordType::try_new(RECORD_TYPE))?,
            record_id: record_id.clone(),
        },
        version,
        payload: TypedPayload {
            owner: configured(ModuleId::try_new(MODULE_ID))?,
            schema_id: configured(SchemaId::try_new(CONSENT_AUTHORIZATION_STATE_SCHEMA_ID))?,
            schema_version: configured(SchemaVersion::try_new(
                CONSENT_AUTHORIZATION_STATE_SCHEMA_VERSION,
            ))?,
            descriptor_hash: expected_descriptor_hash,
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: CONSENT_AUTHORIZATION_STATE_MAXIMUM_BYTES,
            retention_policy_id: configured(RetentionPolicyId::try_new(
                CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID,
            ))?,
            bytes: payload_bytes,
        },
    };
    let authorization = consent_authorization_from_snapshot(&snapshot)
        .map_err(|error| stored_state_invalid(format!("{}: {}", error.code, error.safe_message)))?;
    if authorization.party_ref().as_str() != canonical_party_id.as_str() {
        return Err(stored_state_invalid(
            "persisted Consent Party reference does not match the authoritative relationship source",
        ));
    }
    let resource_version = u64::try_from(authorization.version())
        .map_err(|_| stored_state_invalid("persisted Consent version must be positive"))?;
    Ok(VerifiedConsentResource {
        record_id,
        resource_version,
    })
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    crate::errors::configured(value)
}
