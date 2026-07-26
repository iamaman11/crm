use crate::contract::{
    MAXIMUM_SCAN_RECORDS_PER_PAGE, party_relationships_privacy_scope_definition,
    validate_definition,
};
use crate::errors::{
    database_unavailable, map_canonical_party_claim_error, row_decode_error, stored_state_invalid,
};
use crate::request::{ValidatedRequest, validate_request_contract, validate_wire_request};
use crate::response::{VerifiedPartyRelationshipResource, build_response, typed_output};
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::{BoundReadTransaction, PostgresDataStore};
use crm_customer_privacy_owner_scope_support::prove_canonical_party_claim;
use crm_module_sdk::{
    DataClass, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef, RecordSnapshot,
    RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
};
use crm_party_relationships::{
    MODULE_ID, PARTY_RELATIONSHIP_STATE_MAXIMUM_BYTES,
    PARTY_RELATIONSHIP_STATE_RETENTION_POLICY_ID, PARTY_RELATIONSHIP_STATE_SCHEMA_ID,
    PARTY_RELATIONSHIP_STATE_SCHEMA_VERSION, party_relationship_state_descriptor_hash,
};
use crm_party_relationships_capability_adapter::{RECORD_TYPE, party_relationship_from_snapshot};
use crm_query_runtime::{QueryExecutionResult, QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::Row;

const SCAN_CHUNK_SIZE: usize = 128;

#[derive(Clone)]
pub struct PartyRelationshipsPrivacyScopeQueryAdapter {
    store: PostgresDataStore,
}

impl std::fmt::Debug for PartyRelationshipsPrivacyScopeQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyRelationshipsPrivacyScopeQueryAdapter")
            .field("store", &"PostgresDataStore")
            .finish()
    }
}

impl PartyRelationshipsPrivacyScopeQueryAdapter {
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
        prove_canonical_party_claim(
            &mut transaction,
            &request.context.tenant_id,
            &validated.canonical_party_id,
            validated.identity_resolution_generation,
        )
        .await
        .map_err(map_canonical_party_claim_error)?;

        let page = read_party_relationship_page(&mut transaction, &validated).await?;
        let response = build_response(
            &validated,
            &page.resources,
            page.scanned_resource_count,
            page.next_after_record_id.as_ref(),
        )?;
        let output = typed_output(response.encode_to_vec())?;
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(QueryExecutionResult { output })
    }
}

impl QueryExecutor for PartyRelationshipsPrivacyScopeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move { self.execute_query(definition, request).await })
    }
}

struct PartyRelationshipPage {
    resources: Vec<VerifiedPartyRelationshipResource>,
    scanned_resource_count: u64,
    next_after_record_id: Option<RecordId>,
}

async fn read_party_relationship_page(
    transaction: &mut BoundReadTransaction<'_>,
    request: &ValidatedRequest,
) -> Result<PartyRelationshipPage, SdkError> {
    let mut resources = Vec::with_capacity(request.page_size as usize);
    let mut scanned = 0_usize;
    let mut after_record_id = request.after_record_id.clone();

    loop {
        let remaining_scan = MAXIMUM_SCAN_RECORDS_PER_PAGE.saturating_sub(scanned);
        if remaining_scan == 0 {
            return Ok(PartyRelationshipPage {
                resources,
                scanned_resource_count: u64::try_from(scanned).map_err(|_| {
                    stored_state_invalid(
                        "Party Relationships privacy scope scan count does not fit in u64",
                    )
                })?,
                next_after_record_id: after_record_id,
            });
        }

        let scan_limit = remaining_scan.min(SCAN_CHUNK_SIZE);
        let fetch_limit = i64::try_from(scan_limit.saturating_add(1)).map_err(|_| {
            stored_state_invalid("Party Relationships privacy scope SQL limit does not fit in i64")
        })?;
        let rows = sqlx::query(
            r#"
            SELECT
              record_id,
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
              AND deleted_at IS NULL
              AND ($4::text IS NULL OR record_id > $4)
            ORDER BY record_id ASC
            LIMIT $5
            "#,
        )
        .bind(request.lineage.tenant_id.as_str())
        .bind(MODULE_ID)
        .bind(RECORD_TYPE)
        .bind(after_record_id.as_ref().map(RecordId::as_str))
        .bind(fetch_limit)
        .fetch_all(&mut ***transaction)
        .await
        .map_err(database_unavailable)?;

        let available = rows.len().min(scan_limit);
        let more_after_batch = rows.len() > scan_limit;
        if available == 0 {
            return Ok(PartyRelationshipPage {
                resources,
                scanned_resource_count: u64::try_from(scanned).map_err(|_| {
                    stored_state_invalid(
                        "Party Relationships privacy scope scan count does not fit in u64",
                    )
                })?,
                next_after_record_id: None,
            });
        }

        for (index, row) in rows.into_iter().take(available).enumerate() {
            let record_id = RecordId::try_new(
                row.try_get::<String, _>("record_id")
                    .map_err(row_decode_error)?,
            )
            .map_err(|error| stored_state_invalid(error.to_string()))?;
            let resource =
                strict_party_relationship_resource(&request.canonical_party_id, &record_id, row)?;
            scanned = scanned.saturating_add(1);
            after_record_id = Some(record_id);

            if let Some(resource) = resource {
                resources.push(resource);
            }
            if resources.len() == request.page_size as usize {
                let more_in_batch = index + 1 < available;
                return Ok(PartyRelationshipPage {
                    resources,
                    scanned_resource_count: u64::try_from(scanned).map_err(|_| {
                        stored_state_invalid(
                            "Party Relationships privacy scope scan count does not fit in u64",
                        )
                    })?,
                    next_after_record_id: if more_in_batch || more_after_batch {
                        after_record_id
                    } else {
                        None
                    },
                });
            }
        }

        if !more_after_batch {
            return Ok(PartyRelationshipPage {
                resources,
                scanned_resource_count: u64::try_from(scanned).map_err(|_| {
                    stored_state_invalid(
                        "Party Relationships privacy scope scan count does not fit in u64",
                    )
                })?,
                next_after_record_id: None,
            });
        }
    }
}

fn strict_party_relationship_resource(
    canonical_party_id: &RecordId,
    record_id: &RecordId,
    row: sqlx::postgres::PgRow,
) -> Result<Option<VerifiedPartyRelationshipResource>, SdkError> {
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

    let expected_descriptor_hash = party_relationship_state_descriptor_hash();
    if version <= 0
        || owner_module_id != MODULE_ID
        || stored_schema_id != PARTY_RELATIONSHIP_STATE_SCHEMA_ID
        || stored_schema_version != PARTY_RELATIONSHIP_STATE_SCHEMA_VERSION
        || stored_descriptor_hash.as_slice() != expected_descriptor_hash
        || stored_data_class != "personal"
        || stored_encoding != "json"
        || stored_maximum_size != PARTY_RELATIONSHIP_STATE_MAXIMUM_BYTES as i64
        || stored_retention != PARTY_RELATIONSHIP_STATE_RETENTION_POLICY_ID
    {
        return Err(stored_state_invalid(
            "persisted Party Relationship metadata does not match the canonical state contract",
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
            schema_id: configured(SchemaId::try_new(PARTY_RELATIONSHIP_STATE_SCHEMA_ID))?,
            schema_version: configured(SchemaVersion::try_new(
                PARTY_RELATIONSHIP_STATE_SCHEMA_VERSION,
            ))?,
            descriptor_hash: expected_descriptor_hash,
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: PARTY_RELATIONSHIP_STATE_MAXIMUM_BYTES,
            retention_policy_id: configured(RetentionPolicyId::try_new(
                PARTY_RELATIONSHIP_STATE_RETENTION_POLICY_ID,
            ))?,
            bytes: payload_bytes,
        },
    };
    let party_relationship = party_relationship_from_snapshot(&snapshot)
        .map_err(|error| stored_state_invalid(format!("{}: {}", error.code, error.safe_message)))?;
    if party_relationship.party_ref().as_str() != canonical_party_id.as_str() {
        return Ok(None);
    }
    let resource_version = u64::try_from(party_relationship.version()).map_err(|_| {
        stored_state_invalid("persisted Party Relationship version must be positive")
    })?;
    Ok(Some(VerifiedPartyRelationshipResource {
        record_id: record_id.clone(),
        resource_version,
    }))
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    crate::errors::configured(value)
}

#[allow(dead_code)]
fn _definition_smoke() -> Result<CapabilityDefinition, SdkError> {
    party_relationships_privacy_scope_definition()
}
