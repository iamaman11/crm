use crate::contract::{
    MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED, MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS,
    MAX_PRIVACY_EXPORT_SELECTION_ITEMS_SCANNED, MAX_PRIVACY_IMPORT_ROWS_SCANNED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, customer_data_privacy_scope_definition,
    validate_definition,
};
use crate::errors::{
    association_state_invalid, canonical_resolution_unavailable, database_unavailable,
    export_outcome_state_invalid, export_selection_state_invalid, export_stage_state_invalid,
    import_row_state_invalid, lineage_invalid, map_canonical_party_claim_error,
    scan_limit_exceeded,
};
use crate::request::{
    CursorState, ResourceFamily, ValidatedRequest, validate_request_contract, validate_wire_request,
};
use crate::response::{VerifiedCustomerDataResource, build_response, typed_output};
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::{BoundReadTransaction, PostgresDataStore};
use crm_customer_data_operations::{
    ExportJobId, ImportRow, PartyExportExecutionOutcome, PartyExportExecutionStage,
    PartyExportSelectionItem, decode_export_selection_item_state, decode_import_row_state,
};
use crm_customer_data_operations_capability_adapter::{
    EXPORT_EXECUTION_OUTCOME_RECORD_TYPE, EXPORT_EXECUTION_STAGE_RECORD_TYPE, IMPORT_ROW_RECORD_TYPE,
    MODULE_ID, export_execution_outcome_from_snapshot, export_execution_outcome_persisted_contract,
    export_execution_stage_from_snapshot, export_execution_stage_persisted_contract,
    export_selection_item_persisted_contract, import_row_persisted_contract,
};
use crm_customer_privacy_owner_scope_support::prove_canonical_party_claim;
use crm_identity_resolution::PartyReference;
use crm_identity_resolution_topology_composition::prove_canonical_party_in_transaction;
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
};
use crm_query_runtime::{QueryExecutionResult, QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};

const EXPORT_SELECTION_ITEM_RECORD_TYPE: &str = "customer_data.export_selection_item";
const RECORD_SCAN_BATCH_SIZE: i64 = 512;

#[derive(Clone)]
pub struct CustomerDataOperationsPrivacyScopeQueryAdapter {
    store: PostgresDataStore,
}

impl std::fmt::Debug for CustomerDataOperationsPrivacyScopeQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerDataOperationsPrivacyScopeQueryAdapter")
            .field("store", &"PostgresDataStore")
            .finish()
    }
}

impl CustomerDataOperationsPrivacyScopeQueryAdapter {
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

        let page = read_customer_data_page(
            &mut transaction,
            &request.context.tenant_id,
            &validated,
        )
        .await?;
        let response = build_response(
            &validated,
            &page.resources,
            page.scanned_resource_count,
            page.next_state.as_ref(),
        )?;
        let output = typed_output(response.encode_to_vec())?;
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(QueryExecutionResult { output })
    }
}

impl QueryExecutor for CustomerDataOperationsPrivacyScopeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move { self.execute_query(definition, request).await })
    }
}

struct CustomerDataPage {
    resources: Vec<VerifiedCustomerDataResource>,
    scanned_resource_count: u64,
    next_state: Option<CursorState>,
}

struct ImportRecord {
    record_id: RecordId,
    version: u64,
    row: ImportRow,
}

struct SelectionRecord {
    record_id: RecordId,
    version: u64,
    item: PartyExportSelectionItem,
}

struct StageRecord {
    record_id: RecordId,
    version: u64,
    stage: PartyExportExecutionStage,
}

struct OutcomeRecord {
    record_id: RecordId,
    version: u64,
    outcome: PartyExportExecutionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SelectionKey {
    job_id: String,
    manifest_position: u32,
}

struct CanonicalResolutionCache {
    values: BTreeMap<String, bool>,
    examined: usize,
}

impl CanonicalResolutionCache {
    fn new() -> Self {
        Self {
            values: BTreeMap::new(),
            examined: 0,
        }
    }

    async fn resolves_to_subject(
        &mut self,
        transaction: &mut BoundReadTransaction<'_>,
        tenant_id: &crm_module_sdk::TenantId,
        party_id: &str,
        request: &ValidatedRequest,
    ) -> Result<bool, SdkError> {
        if let Some(relevant) = self.values.get(party_id) {
            return Ok(*relevant);
        }
        self.examined = self.examined.checked_add(1).ok_or_else(|| {
            scan_limit_exceeded("canonical Party resolution counter overflowed")
        })?;
        if self.examined > MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS {
            return Err(scan_limit_exceeded(
                "canonical Party resolution count exceeded the frozen privacy bound",
            ));
        }

        let requested = PartyReference::try_new(party_id).map_err(|error| {
            canonical_resolution_unavailable(format!(
                "persisted Customer Data Party reference is invalid: {error}"
            ))
        })?;
        let canonical = PartyReference::try_new(request.canonical_party_id.as_str()).map_err(
            |error| {
                canonical_resolution_unavailable(format!(
                    "accepted canonical Party reference is invalid: {error}"
                ))
            },
        )?;
        let relevant = match prove_canonical_party_in_transaction(
            &mut ***transaction,
            tenant_id,
            &requested,
            &canonical,
            request.identity_resolution_generation,
        )
        .await
        {
            Ok(_) => true,
            Err(error) if error.code == "IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH" => false,
            Err(error) if error.code == "IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE" => {
                return Err(lineage_invalid(
                    ErrorCategory::Conflict,
                    true,
                    "Identity Resolution topology generation changed during Customer Data scope discovery",
                ));
            }
            Err(error) => {
                return Err(canonical_resolution_unavailable(format!(
                    "{}: {}",
                    error.code, error.safe_message
                )));
            }
        };
        self.values.insert(party_id.to_owned(), relevant);
        Ok(relevant)
    }
}

async fn read_customer_data_page(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &crm_module_sdk::TenantId,
    request: &ValidatedRequest,
) -> Result<CustomerDataPage, SdkError> {
    let import_rows = load_record_rows(
        transaction,
        &request.lineage.tenant_id,
        IMPORT_ROW_RECORD_TYPE,
        MAX_PRIVACY_IMPORT_ROWS_SCANNED,
    )
    .await?;
    let selection_rows = load_record_rows(
        transaction,
        &request.lineage.tenant_id,
        EXPORT_SELECTION_ITEM_RECORD_TYPE,
        MAX_PRIVACY_EXPORT_SELECTION_ITEMS_SCANNED,
    )
    .await?;
    let stage_rows = load_record_rows(
        transaction,
        &request.lineage.tenant_id,
        EXPORT_EXECUTION_STAGE_RECORD_TYPE,
        MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED,
    )
    .await?;
    let outcome_rows = load_record_rows(
        transaction,
        &request.lineage.tenant_id,
        EXPORT_EXECUTION_OUTCOME_RECORD_TYPE,
        MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED,
    )
    .await?;

    let associated_count = stage_rows
        .len()
        .checked_add(outcome_rows.len())
        .ok_or_else(|| scan_limit_exceeded("associated export evidence count overflowed"))?;
    if associated_count > MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED {
        return Err(scan_limit_exceeded(
            "associated export evidence exceeded the frozen privacy bound",
        ));
    }
    let scanned = import_rows
        .len()
        .checked_add(selection_rows.len())
        .and_then(|value| value.checked_add(stage_rows.len()))
        .and_then(|value| value.checked_add(outcome_rows.len()))
        .ok_or_else(|| scan_limit_exceeded("owner record scan count overflowed"))?;
    if scanned > MAX_PRIVACY_OWNER_RECORDS_SCANNED {
        return Err(scan_limit_exceeded(
            "owner record scan exceeded the frozen privacy bound",
        ));
    }

    let import_records = import_rows
        .into_iter()
        .map(strict_import_record)
        .collect::<Result<Vec<_>, _>>()?;
    let selection_records = selection_rows
        .into_iter()
        .map(strict_selection_record)
        .collect::<Result<Vec<_>, _>>()?;
    let stage_records = stage_rows
        .into_iter()
        .map(strict_stage_record)
        .collect::<Result<Vec<_>, _>>()?;
    let outcome_records = outcome_rows
        .into_iter()
        .map(strict_outcome_record)
        .collect::<Result<Vec<_>, _>>()?;

    let mut resolution_cache = CanonicalResolutionCache::new();
    let mut ordered = Vec::new();

    for record in import_records {
        let snapshot = record.row.snapshot();
        let mut party_ids = BTreeSet::new();
        if let Some(prepared) = snapshot.prepared_party.as_ref() {
            party_ids.insert(prepared.party_id().as_str().to_owned());
        }
        if let Some(target) = snapshot.target_party_id.as_ref() {
            party_ids.insert(target.as_str().to_owned());
        }
        let mut relevant = false;
        for party_id in party_ids {
            if resolution_cache
                .resolves_to_subject(transaction, tenant_id, &party_id, request)
                .await?
            {
                relevant = true;
            }
        }
        if relevant {
            ordered.push(VerifiedCustomerDataResource {
                family: ResourceFamily::ImportRow,
                record_id: record.record_id,
                resource_version: record.version,
            });
        }
    }

    let mut relevant_selection_keys = BTreeSet::new();
    for record in selection_records {
        let relevant = resolution_cache
            .resolves_to_subject(
                transaction,
                tenant_id,
                record.item.party_id().as_str(),
                request,
            )
            .await?;
        if relevant {
            relevant_selection_keys.insert(selection_key(
                record.item.job_id(),
                record.item.manifest_position(),
            ));
            ordered.push(VerifiedCustomerDataResource {
                family: ResourceFamily::ExportSelectionItem,
                record_id: record.record_id,
                resource_version: record.version,
            });
        }
    }

    for record in stage_records {
        if relevant_selection_keys.contains(&selection_key(
            record.stage.job_id(),
            record.stage.manifest_position(),
        )) {
            ordered.push(VerifiedCustomerDataResource {
                family: ResourceFamily::ExportExecutionStage,
                record_id: record.record_id,
                resource_version: record.version,
            });
        }
    }
    for record in outcome_records {
        if relevant_selection_keys.contains(&selection_key(
            record.outcome.job_id(),
            record.outcome.manifest_position(),
        )) {
            ordered.push(VerifiedCustomerDataResource {
                family: ResourceFamily::ExportExecutionOutcome,
                record_id: record.record_id,
                resource_version: record.version,
            });
        }
    }

    let mut matching = ordered
        .into_iter()
        .filter(|resource| resource_after_cursor(resource, &request.cursor_state))
        .take(request.page_size as usize + 1)
        .collect::<Vec<_>>();
    let has_more = matching.len() > request.page_size as usize;
    if has_more {
        matching.pop();
    }
    let next_state = if has_more {
        let last = matching.last().ok_or_else(|| {
            association_state_invalid("Customer Data page continuation has no anchor")
        })?;
        Some(CursorState {
            family: last.family,
            after_record_id: Some(last.record_id.clone()),
        })
    } else {
        None
    };

    Ok(CustomerDataPage {
        resources: matching,
        scanned_resource_count: u64::try_from(scanned)
            .map_err(|_| scan_limit_exceeded("owner scan count does not fit in u64"))?,
        next_state,
    })
}

fn resource_after_cursor(
    resource: &VerifiedCustomerDataResource,
    cursor: &CursorState,
) -> bool {
    match resource.family.cmp(&cursor.family) {
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Equal => cursor
            .after_record_id
            .as_ref()
            .is_none_or(|after| resource.record_id.as_str() > after.as_str()),
    }
}

fn selection_key(job_id: &ExportJobId, manifest_position: u32) -> SelectionKey {
    SelectionKey {
        job_id: job_id.as_str().to_owned(),
        manifest_position,
    }
}

struct StoredRecordRow {
    record_id: RecordId,
    version: i64,
    owner_module_id: String,
    schema_id: String,
    schema_version: String,
    descriptor_hash: Vec<u8>,
    data_class: String,
    payload_encoding: String,
    maximum_payload_size: i64,
    retention_policy_id: String,
    payload_bytes: Vec<u8>,
}

async fn load_record_rows(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &str,
    record_type: &str,
    maximum: usize,
) -> Result<Vec<StoredRecordRow>, SdkError> {
    let mut after_record_id = String::new();
    let mut output = Vec::new();
    loop {
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
              AND record_id > $4
              AND deleted_at IS NULL
            ORDER BY record_id ASC
            LIMIT $5
            "#,
        )
        .bind(tenant_id)
        .bind(MODULE_ID)
        .bind(record_type)
        .bind(&after_record_id)
        .bind(RECORD_SCAN_BATCH_SIZE)
        .fetch_all(&mut ***transaction)
        .await
        .map_err(database_unavailable)?;
        if rows.is_empty() {
            break;
        }
        let batch_len = rows.len();
        for row in rows {
            let decoded = decode_stored_row(row)?;
            after_record_id = decoded.record_id.as_str().to_owned();
            output.push(decoded);
            if output.len() > maximum {
                return Err(scan_limit_exceeded(format!(
                    "{record_type} scan exceeded the frozen privacy bound"
                )));
            }
        }
        if batch_len < RECORD_SCAN_BATCH_SIZE as usize {
            break;
        }
    }
    Ok(output)
}

fn decode_stored_row(row: sqlx::postgres::PgRow) -> Result<StoredRecordRow, SdkError> {
    let invalid = |reference: String| association_state_invalid(reference);
    Ok(StoredRecordRow {
        record_id: RecordId::try_new(
            row.try_get::<String, _>("record_id")
                .map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(|error| invalid(error.to_string()))?,
        version: row
            .try_get("version")
            .map_err(|error| invalid(error.to_string()))?,
        owner_module_id: row
            .try_get("owner_module_id")
            .map_err(|error| invalid(error.to_string()))?,
        schema_id: row
            .try_get("schema_id")
            .map_err(|error| invalid(error.to_string()))?,
        schema_version: row
            .try_get("schema_version")
            .map_err(|error| invalid(error.to_string()))?,
        descriptor_hash: row
            .try_get("descriptor_hash")
            .map_err(|error| invalid(error.to_string()))?,
        data_class: row
            .try_get("data_class")
            .map_err(|error| invalid(error.to_string()))?,
        payload_encoding: row
            .try_get("payload_encoding")
            .map_err(|error| invalid(error.to_string()))?,
        maximum_payload_size: row
            .try_get("maximum_payload_size")
            .map_err(|error| invalid(error.to_string()))?,
        retention_policy_id: row
            .try_get("retention_policy_id")
            .map_err(|error| invalid(error.to_string()))?,
        payload_bytes: row
            .try_get("payload_bytes")
            .map_err(|error| invalid(error.to_string()))?,
    })
}

fn strict_import_record(row: StoredRecordRow) -> Result<ImportRecord, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        IMPORT_ROW_RECORD_TYPE,
        import_row_persisted_contract(),
        import_row_state_invalid,
    )?;
    let import_row = decode_import_row_state(&snapshot.payload.bytes).map_err(|error| {
        import_row_state_invalid(format!("{}: {}", error.code, error.safe_message))
    })?;
    if import_row.row_id().as_str() != row.record_id.as_str()
        || import_row.version() != row.version
    {
        return Err(import_row_state_invalid(
            "import-row identity/version disagrees with its authoritative payload",
        ));
    }
    Ok(ImportRecord {
        record_id: row.record_id,
        version: positive_version(row.version, import_row_state_invalid)?,
        row: import_row,
    })
}

fn strict_selection_record(row: StoredRecordRow) -> Result<SelectionRecord, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        EXPORT_SELECTION_ITEM_RECORD_TYPE,
        export_selection_item_persisted_contract(),
        export_selection_state_invalid,
    )?;
    let item = decode_export_selection_item_state(&snapshot.payload.bytes).map_err(|error| {
        export_selection_state_invalid(format!("{}: {}", error.code, error.safe_message))
    })?;
    if item.item_id().as_str() != row.record_id.as_str() || item.version() != row.version {
        return Err(export_selection_state_invalid(
            "selection-item identity/version disagrees with its authoritative payload",
        ));
    }
    Ok(SelectionRecord {
        record_id: row.record_id,
        version: positive_version(row.version, export_selection_state_invalid)?,
        item,
    })
}

fn strict_stage_record(row: StoredRecordRow) -> Result<StageRecord, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        EXPORT_EXECUTION_STAGE_RECORD_TYPE,
        export_execution_stage_persisted_contract(),
        export_stage_state_invalid,
    )?;
    let stage = export_execution_stage_from_snapshot(&snapshot).map_err(|error| {
        export_stage_state_invalid(format!("{}: {}", error.code, error.safe_message))
    })?;
    Ok(StageRecord {
        record_id: row.record_id,
        version: positive_version(row.version, export_stage_state_invalid)?,
        stage,
    })
}

fn strict_outcome_record(row: StoredRecordRow) -> Result<OutcomeRecord, SdkError> {
    let snapshot = strict_snapshot(
        &row,
        EXPORT_EXECUTION_OUTCOME_RECORD_TYPE,
        export_execution_outcome_persisted_contract(),
        export_outcome_state_invalid,
    )?;
    let outcome = export_execution_outcome_from_snapshot(&snapshot).map_err(|error| {
        export_outcome_state_invalid(format!("{}: {}", error.code, error.safe_message))
    })?;
    Ok(OutcomeRecord {
        record_id: row.record_id,
        version: positive_version(row.version, export_outcome_state_invalid)?,
        outcome,
    })
}

fn strict_snapshot(
    row: &StoredRecordRow,
    record_type: &str,
    contract: crm_capability_plan_support::PersistedPayloadContract<'_>,
    invalid: fn(String) -> SdkError,
) -> Result<RecordSnapshot, SdkError> {
    if row.version <= 0
        || row.owner_module_id != contract.owner
        || row.schema_id != contract.schema_id
        || row.schema_version != contract.schema_version
        || row.descriptor_hash.as_slice() != contract.descriptor_hash
        || row.data_class != "personal"
        || row.payload_encoding != "json"
        || row.maximum_payload_size != contract.maximum_size_bytes as i64
        || row.retention_policy_id != contract.retention_policy_id
    {
        return Err(invalid(
            "persisted metadata does not match the canonical Customer Data state contract"
                .to_owned(),
        ));
    }
    let snapshot = RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(record_type).map_err(|error| invalid(error.to_string()))?,
            record_id: row.record_id.clone(),
        },
        version: row.version,
        payload: TypedPayload {
            owner: ModuleId::try_new(contract.owner).map_err(|error| invalid(error.to_string()))?,
            schema_id: SchemaId::try_new(contract.schema_id)
                .map_err(|error| invalid(error.to_string()))?,
            schema_version: SchemaVersion::try_new(contract.schema_version)
                .map_err(|error| invalid(error.to_string()))?,
            descriptor_hash: contract.descriptor_hash,
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: contract.maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new(contract.retention_policy_id)
                .map_err(|error| invalid(error.to_string()))?,
            bytes: row.payload_bytes.clone(),
        },
    };
    snapshot
        .payload
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    Ok(snapshot)
}

fn positive_version(
    version: i64,
    invalid: fn(String) -> SdkError,
) -> Result<u64, SdkError> {
    u64::try_from(version).map_err(|_| invalid("resource version must be positive".to_owned()))
}
