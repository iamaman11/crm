use crate::contract::{
    MAXIMUM_SCAN_RECORDS_PER_PAGE, contact_points_privacy_scope_definition, validate_definition,
};
use crate::errors::{
    database_unavailable, map_canonical_party_claim_error, row_decode_error, stored_state_invalid,
};
use crate::request::{ValidatedRequest, validate_request_contract, validate_wire_request};
use crate::response::{VerifiedContactPointResource, build_response, typed_output};
use crm_capability_runtime::CapabilityDefinition;
use crm_contact_points::{
    CONTACT_POINT_STATE_MAXIMUM_BYTES, CONTACT_POINT_STATE_RETENTION_POLICY_ID,
    CONTACT_POINT_STATE_SCHEMA_ID, CONTACT_POINT_STATE_SCHEMA_VERSION, MODULE_ID,
    contact_point_state_descriptor_hash,
};
use crm_contact_points_capability_adapter::{RECORD_TYPE, contact_point_from_snapshot};
use crm_core_data::{BoundReadTransaction, PostgresDataStore};
use crm_customer_privacy_owner_scope_support::prove_canonical_party_claim;
use crm_module_sdk::{
    DataClass, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef, RecordSnapshot,
    RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
};
use crm_query_runtime::{QueryExecutionResult, QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::Row;

const SCAN_CHUNK_SIZE: usize = 128;

#[derive(Clone)]
pub struct ContactPointsPrivacyScopeQueryAdapter {
    store: PostgresDataStore,
}

impl std::fmt::Debug for ContactPointsPrivacyScopeQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContactPointsPrivacyScopeQueryAdapter")
            .field("store", &"PostgresDataStore")
            .finish()
    }
}

impl ContactPointsPrivacyScopeQueryAdapter {
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

        let page = read_contact_point_page(&mut transaction, &validated).await?;
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

impl QueryExecutor for ContactPointsPrivacyScopeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move { self.execute_query(definition, request).await })
    }
}

struct ContactPointPage {
    resources: Vec<VerifiedContactPointResource>,
    scanned_resource_count: u64,
    next_after_record_id: Option<RecordId>,
}

async fn read_contact_point_page(
    transaction: &mut BoundReadTransaction<'_>,
    request: &ValidatedRequest,
) -> Result<ContactPointPage, SdkError> {
    let mut resources = Vec::with_capacity(request.page_size as usize);
    let mut scanned = 0_usize;
    let mut after_record_id = request.after_record_id.clone();

    loop {
        let remaining_scan = MAXIMUM_SCAN_RECORDS_PER_PAGE.saturating_sub(scanned);
        if remaining_scan == 0 {
            return Ok(ContactPointPage {
                resources,
                scanned_resource_count: u64::try_from(scanned).map_err(|_| {
                    stored_state_invalid(
                        "Contact Points privacy scope scan count does not fit in u64",
                    )
                })?,
                next_after_record_id: after_record_id,
            });
        }

        let scan_limit = remaining_scan.min(SCAN_CHUNK_SIZE);
        let fetch_limit = i64::try_from(scan_limit.saturating_add(1)).map_err(|_| {
            stored_state_invalid("Contact Points privacy scope SQL limit does not fit in i64")
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
            return Ok(ContactPointPage {
                resources,
                scanned_resource_count: u64::try_from(scanned).map_err(|_| {
                    stored_state_invalid(
                        "Contact Points privacy scope scan count does not fit in u64",
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
                strict_contact_point_resource(&request.canonical_party_id, &record_id, row)?;
            scanned = scanned.saturating_add(1);
            after_record_id = Some(record_id);

            if let Some(resource) = resource {
                resources.push(resource);
            }
            if resources.len() == request.page_size as usize {
                let more_in_batch = index + 1 < available;
                return Ok(ContactPointPage {
                    resources,
                    scanned_resource_count: u64::try_from(scanned).map_err(|_| {
                        stored_state_invalid(
                            "Contact Points privacy scope scan count does not fit in u64",
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
            return Ok(ContactPointPage {
                resources,
                scanned_resource_count: u64::try_from(scanned).map_err(|_| {
                    stored_state_invalid(
                        "Contact Points privacy scope scan count does not fit in u64",
                    )
                })?,
                next_after_record_id: None,
            });
        }
    }
}

fn strict_contact_point_resource(
    canonical_party_id: &RecordId,
    record_id: &RecordId,
    row: sqlx::postgres::PgRow,
) -> Result<Option<VerifiedContactPointResource>, SdkError> {
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

    let expected_descriptor_hash = contact_point_state_descriptor_hash();
    if version <= 0
        || owner_module_id != MODULE_ID
        || stored_schema_id != CONTACT_POINT_STATE_SCHEMA_ID
        || stored_schema_version != CONTACT_POINT_STATE_SCHEMA_VERSION
        || stored_descriptor_hash.as_slice() != expected_descriptor_hash
        || stored_data_class != "personal"
        || stored_encoding != "json"
        || stored_maximum_size != CONTACT_POINT_STATE_MAXIMUM_BYTES as i64
        || stored_retention != CONTACT_POINT_STATE_RETENTION_POLICY_ID
    {
        return Err(stored_state_invalid(
            "persisted Contact Point metadata does not match the canonical state contract",
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
            schema_id: configured(SchemaId::try_new(CONTACT_POINT_STATE_SCHEMA_ID))?,
            schema_version: configured(SchemaVersion::try_new(CONTACT_POINT_STATE_SCHEMA_VERSION))?,
            descriptor_hash: expected_descriptor_hash,
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: CONTACT_POINT_STATE_MAXIMUM_BYTES,
            retention_policy_id: configured(RetentionPolicyId::try_new(
                CONTACT_POINT_STATE_RETENTION_POLICY_ID,
            ))?,
            bytes: payload_bytes,
        },
    };
    let contact_point = contact_point_from_snapshot(&snapshot)
        .map_err(|error| stored_state_invalid(format!("{}: {}", error.code, error.safe_message)))?;
    if contact_point.party_ref().as_str() != canonical_party_id.as_str() {
        return Ok(None);
    }
    let resource_version = u64::try_from(contact_point.version())
        .map_err(|_| stored_state_invalid("persisted Contact Point version must be positive"))?;
    Ok(Some(VerifiedContactPointResource {
        record_id: record_id.clone(),
        resource_version,
    }))
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    crate::errors::configured(value)
}

#[allow(dead_code)]
fn _definition_smoke() -> Result<CapabilityDefinition, SdkError> {
    contact_points_privacy_scope_definition()
}
