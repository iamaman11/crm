use crate::contract::{
    MAX_PRIVACY_ACTIVE_REDIRECT_EDGES, MAX_PRIVACY_ALIAS_HOPS, MAX_PRIVACY_ALIAS_NODES,
    MAX_PRIVACY_CANDIDATE_RECORDS_REHYDRATED, MAX_PRIVACY_MERGE_RECORDS_REHYDRATED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAX_PRIVACY_RELATIONSHIP_CANDIDATES,
    identity_resolution_privacy_scope_definition, validate_definition,
};
use crate::errors::{
    database_unavailable, limit_exceeded, map_canonical_party_claim_error, row_decode_error,
    stored_candidate_state_invalid, stored_merge_state_invalid, topology_invalid,
};
use crate::request::{
    CursorState, ResourceFamily, ValidatedRequest, validate_request_contract, validate_wire_request,
};
use crate::response::{VerifiedIdentityResolutionResource, build_response, typed_output};
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::{BoundReadTransaction, PostgresDataStore};
use crm_customer_privacy_owner_scope_support::prove_canonical_party_claim;
use crm_identity_resolution::{
    CanonicalPartyGraph, DUPLICATE_CANDIDATE_CASE_STATE_MAXIMUM_BYTES,
    DUPLICATE_CANDIDATE_CASE_STATE_RETENTION_POLICY_ID, DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_ID,
    DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_VERSION, DuplicateCandidateCase,
    MERGE_OPERATION_STATE_MAXIMUM_BYTES, MERGE_OPERATION_STATE_RETENTION_POLICY_ID,
    MERGE_OPERATION_STATE_SCHEMA_ID, MERGE_OPERATION_STATE_SCHEMA_VERSION, MergeOperation,
    MergeOperationStatus, PartyReference, duplicate_candidate_case_state_descriptor_hash,
    merge_operation_state_descriptor_hash,
};
use crm_identity_resolution_capability_adapter::{
    CANONICAL_REDIRECT_PARTY_RECORD_TYPE, CANONICAL_REDIRECT_RELATIONSHIP_TYPE,
    MERGE_OPERATION_RECORD_TYPE, PARTY_CANDIDATE_RELATIONSHIP_TYPE,
    PARTY_CANDIDATE_SOURCE_RECORD_TYPE, PARTY_MERGE_RELATIONSHIP_TYPE,
    PARTY_MERGE_SOURCE_RECORD_TYPE, RECORD_TYPE, duplicate_candidate_case_from_snapshot,
    merge_operation_from_snapshot,
};
use crm_module_sdk::{
    DataClass, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef, RecordSnapshot,
    RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TypedPayload,
};
use crm_query_runtime::{QueryExecutionResult, QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone)]
pub struct IdentityResolutionPrivacyScopeQueryAdapter {
    store: PostgresDataStore,
}

impl std::fmt::Debug for IdentityResolutionPrivacyScopeQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentityResolutionPrivacyScopeQueryAdapter")
            .field("store", &"PostgresDataStore")
            .finish()
    }
}

impl IdentityResolutionPrivacyScopeQueryAdapter {
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

        let page = read_identity_resolution_page(&mut transaction, &validated).await?;
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

impl QueryExecutor for IdentityResolutionPrivacyScopeQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move { self.execute_query(definition, request).await })
    }
}

struct IdentityResolutionPage {
    resources: Vec<VerifiedIdentityResolutionResource>,
    scanned_resource_count: u64,
    next_state: Option<CursorState>,
}

struct CandidateRecord {
    record_id: RecordId,
    candidate: DuplicateCandidateCase,
}

struct MergeRecord {
    record_id: RecordId,
    operation: MergeOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RedirectPair {
    source: PartyReference,
    survivor: PartyReference,
}

async fn read_identity_resolution_page(
    transaction: &mut BoundReadTransaction<'_>,
    request: &ValidatedRequest,
) -> Result<IdentityResolutionPage, SdkError> {
    let merge_records = load_merge_records(transaction, &request.lineage.tenant_id).await?;
    let redirects = load_redirect_pairs(transaction, &request.lineage.tenant_id).await?;
    validate_redirects_against_operations(&redirects, &merge_records)?;

    let graph = CanonicalPartyGraph::try_new(
        merge_records
            .iter()
            .filter_map(|record| record.operation.active_edge()),
    )
    .map_err(|error| {
        topology_invalid(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_GRAPH_INVALID",
            format!("{}: {}", error.code, error.safe_message),
        )
    })?;
    let aliases = reverse_alias_closure(&request.canonical_party_id, &redirects, &graph)?;
    let alias_ids = aliases
        .iter()
        .map(|party| party.as_str().to_owned())
        .collect::<Vec<_>>();

    let candidate_links = load_related_record_ids(
        transaction,
        &request.lineage.tenant_id,
        PARTY_CANDIDATE_RELATIONSHIP_TYPE,
        PARTY_CANDIDATE_SOURCE_RECORD_TYPE,
        RECORD_TYPE,
        &alias_ids,
    )
    .await?;
    let merge_links = load_related_record_ids(
        transaction,
        &request.lineage.tenant_id,
        PARTY_MERGE_RELATIONSHIP_TYPE,
        PARTY_MERGE_SOURCE_RECORD_TYPE,
        MERGE_OPERATION_RECORD_TYPE,
        &alias_ids,
    )
    .await?;
    if redirects.len() + candidate_links.len() + merge_links.len()
        > MAX_PRIVACY_RELATIONSHIP_CANDIDATES
    {
        return Err(limit_exceeded(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_RELATIONSHIP_LIMIT_EXCEEDED",
            "relationship candidate count exceeded the frozen privacy bound",
        ));
    }

    let candidate_records = load_candidate_records(transaction, &request.lineage.tenant_id).await?;
    let relevant_candidates = validate_and_select_candidates(
        &candidate_records,
        &candidate_links,
        &aliases,
        &graph,
        &request.canonical_party_id,
    )?;
    let relevant_merges = validate_and_select_merges(
        &merge_records,
        &merge_links,
        &aliases,
        &graph,
        &request.canonical_party_id,
    )?;

    let scanned = candidate_records
        .len()
        .checked_add(merge_records.len())
        .ok_or_else(|| {
            limit_exceeded(
                "IDENTITY_RESOLUTION_PRIVACY_SCOPE_OWNER_SCAN_LIMIT_EXCEEDED",
                "owner scan count overflowed",
            )
        })?;
    if candidate_records.len() > MAX_PRIVACY_OWNER_RECORDS_SCANNED
        || merge_records.len() > MAX_PRIVACY_OWNER_RECORDS_SCANNED
    {
        return Err(limit_exceeded(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_OWNER_SCAN_LIMIT_EXCEEDED",
            "owner record scan exceeded the frozen privacy bound",
        ));
    }

    let mut ordered = Vec::with_capacity(relevant_candidates.len() + relevant_merges.len());
    ordered.extend(relevant_candidates.into_iter().map(|record| {
        VerifiedIdentityResolutionResource {
            family: ResourceFamily::CandidateCase,
            record_id: record.record_id,
            resource_version: record.resource_version,
        }
    }));
    ordered.extend(
        relevant_merges
            .into_iter()
            .map(|record| VerifiedIdentityResolutionResource {
                family: ResourceFamily::MergeOperation,
                record_id: record.record_id,
                resource_version: record.resource_version,
            }),
    );

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
            stored_merge_state_invalid("Identity Resolution page continuation has no anchor")
        })?;
        Some(CursorState {
            family: last.family,
            after_record_id: Some(last.record_id.clone()),
        })
    } else {
        None
    };

    Ok(IdentityResolutionPage {
        resources: matching,
        scanned_resource_count: u64::try_from(scanned).map_err(|_| {
            limit_exceeded(
                "IDENTITY_RESOLUTION_PRIVACY_SCOPE_OWNER_SCAN_LIMIT_EXCEEDED",
                "owner scan count does not fit in u64",
            )
        })?,
        next_state,
    })
}

fn resource_after_cursor(
    resource: &VerifiedIdentityResolutionResource,
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

async fn load_redirect_pairs(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &str,
) -> Result<Vec<RedirectPair>, SdkError> {
    let fetch_limit = i64::try_from(MAX_PRIVACY_ACTIVE_REDIRECT_EDGES + 1).map_err(|_| {
        limit_exceeded(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_EDGE_LIMIT_EXCEEDED",
            "redirect SQL limit does not fit in i64",
        )
    })?;
    let rows = sqlx::query(
        r#"
        SELECT source_record_id, target_record_id
        FROM crm.relationships
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND relationship_type = $3
          AND source_record_type = $4
          AND target_record_type = $4
        ORDER BY source_record_id ASC, target_record_id ASC
        LIMIT $5
        "#,
    )
    .bind(tenant_id)
    .bind(crm_identity_resolution::MODULE_ID)
    .bind(CANONICAL_REDIRECT_RELATIONSHIP_TYPE)
    .bind(CANONICAL_REDIRECT_PARTY_RECORD_TYPE)
    .bind(fetch_limit)
    .fetch_all(&mut ***transaction)
    .await
    .map_err(database_unavailable)?;
    if rows.len() > MAX_PRIVACY_ACTIVE_REDIRECT_EDGES {
        return Err(limit_exceeded(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_EDGE_LIMIT_EXCEEDED",
            "active redirect edge count exceeded the frozen privacy bound",
        ));
    }
    rows.into_iter()
        .map(|row| {
            let source = PartyReference::try_new(
                row.try_get::<String, _>("source_record_id")
                    .map_err(row_decode_error)?,
            )
            .map_err(|error| {
                topology_invalid(
                    "IDENTITY_RESOLUTION_PRIVACY_SCOPE_GRAPH_INVALID",
                    error.to_string(),
                )
            })?;
            let survivor = PartyReference::try_new(
                row.try_get::<String, _>("target_record_id")
                    .map_err(row_decode_error)?,
            )
            .map_err(|error| {
                topology_invalid(
                    "IDENTITY_RESOLUTION_PRIVACY_SCOPE_GRAPH_INVALID",
                    error.to_string(),
                )
            })?;
            Ok(RedirectPair { source, survivor })
        })
        .collect()
}

fn validate_redirects_against_operations(
    redirects: &[RedirectPair],
    merge_records: &[MergeRecord],
) -> Result<(), SdkError> {
    let redirect_set = redirects.iter().cloned().collect::<BTreeSet<_>>();
    if redirect_set.len() != redirects.len() {
        return Err(topology_invalid(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_REDIRECT_DUPLICATE",
            "duplicate canonical redirect relationship exists",
        ));
    }
    let active_pairs = merge_records
        .iter()
        .filter(|record| record.operation.status() == MergeOperationStatus::Active)
        .map(|record| RedirectPair {
            source: record.operation.source_party_ref().clone(),
            survivor: record.operation.survivor_party_ref().clone(),
        })
        .collect::<Vec<_>>();
    let active_set = active_pairs.iter().cloned().collect::<BTreeSet<_>>();
    if active_set.len() != active_pairs.len() {
        return Err(topology_invalid(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_REDIRECT_OPERATION_DUPLICATE",
            "multiple Active operations describe one canonical redirect edge",
        ));
    }
    if redirect_set != active_set {
        return Err(topology_invalid(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_REDIRECT_OPERATION_MISMATCH",
            "canonical redirect relationships and authoritative Active operations disagree",
        ));
    }
    Ok(())
}

fn reverse_alias_closure(
    canonical_party_id: &RecordId,
    redirects: &[RedirectPair],
    graph: &CanonicalPartyGraph,
) -> Result<BTreeSet<PartyReference>, SdkError> {
    let canonical = PartyReference::try_new(canonical_party_id.as_str()).map_err(|error| {
        topology_invalid(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_GRAPH_INVALID",
            error.to_string(),
        )
    })?;
    let mut incoming = BTreeMap::<PartyReference, Vec<PartyReference>>::new();
    for redirect in redirects {
        incoming
            .entry(redirect.survivor.clone())
            .or_default()
            .push(redirect.source.clone());
    }
    for sources in incoming.values_mut() {
        sources.sort();
        sources.dedup();
    }

    let mut aliases = BTreeSet::from([canonical.clone()]);
    let mut queue = VecDeque::from([(canonical.clone(), 0_usize)]);
    while let Some((target, depth)) = queue.pop_front() {
        if let Some(sources) = incoming.get(&target) {
            for source in sources {
                let next_depth = depth.checked_add(1).ok_or_else(|| {
                    limit_exceeded(
                        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_ALIAS_DEPTH_EXCEEDED",
                        "alias depth overflowed",
                    )
                })?;
                if next_depth > MAX_PRIVACY_ALIAS_HOPS {
                    return Err(limit_exceeded(
                        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_ALIAS_DEPTH_EXCEEDED",
                        "reverse alias closure exceeded 64 hops",
                    ));
                }
                if aliases.insert(source.clone()) {
                    if aliases.len() > MAX_PRIVACY_ALIAS_NODES {
                        return Err(limit_exceeded(
                            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_ALIAS_NODE_LIMIT_EXCEEDED",
                            "reverse alias closure exceeded the frozen node bound",
                        ));
                    }
                    queue.push_back((source.clone(), next_depth));
                }
            }
        }
    }

    for alias in &aliases {
        let resolution = graph.resolve(alias).map_err(|error| {
            topology_invalid(
                "IDENTITY_RESOLUTION_PRIVACY_SCOPE_GRAPH_INVALID",
                format!("{}: {}", error.code, error.safe_message),
            )
        })?;
        if resolution.canonical_party_ref() != &canonical {
            return Err(topology_invalid(
                "IDENTITY_RESOLUTION_PRIVACY_SCOPE_ALIAS_COMPLETENESS_INVALID",
                "discovered alias does not resolve to the accepted canonical Party",
            ));
        }
    }
    Ok(aliases)
}

async fn load_related_record_ids(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &str,
    relationship_type: &str,
    source_record_type: &str,
    target_record_type: &str,
    alias_ids: &[String],
) -> Result<BTreeSet<RecordId>, SdkError> {
    if alias_ids.is_empty() {
        return Ok(BTreeSet::new());
    }
    let fetch_limit = i64::try_from(MAX_PRIVACY_RELATIONSHIP_CANDIDATES + 1).map_err(|_| {
        limit_exceeded(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_RELATIONSHIP_LIMIT_EXCEEDED",
            "relationship SQL limit does not fit in i64",
        )
    })?;
    let rows = sqlx::query(
        r#"
        SELECT source_record_id, target_record_id
        FROM crm.relationships
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND relationship_type = $3
          AND source_record_type = $4
          AND target_record_type = $5
          AND source_record_id = ANY($6::text[])
        ORDER BY target_record_id ASC, source_record_id ASC
        LIMIT $7
        "#,
    )
    .bind(tenant_id)
    .bind(crm_identity_resolution::MODULE_ID)
    .bind(relationship_type)
    .bind(source_record_type)
    .bind(target_record_type)
    .bind(alias_ids)
    .bind(fetch_limit)
    .fetch_all(&mut ***transaction)
    .await
    .map_err(database_unavailable)?;
    if rows.len() > MAX_PRIVACY_RELATIONSHIP_CANDIDATES {
        return Err(limit_exceeded(
            "IDENTITY_RESOLUTION_PRIVACY_SCOPE_RELATIONSHIP_LIMIT_EXCEEDED",
            "relationship candidate count exceeded the frozen privacy bound",
        ));
    }
    rows.into_iter()
        .map(|row| {
            let source: String = row.try_get("source_record_id").map_err(row_decode_error)?;
            if !alias_ids.iter().any(|alias| alias == &source) {
                return Err(topology_invalid(
                    "IDENTITY_RESOLUTION_PRIVACY_SCOPE_RELATIONSHIP_INVALID",
                    "relationship query returned a source outside the alias set",
                ));
            }
            RecordId::try_new(
                row.try_get::<String, _>("target_record_id")
                    .map_err(row_decode_error)?,
            )
            .map_err(|error| stored_merge_state_invalid(error.to_string()))
        })
        .collect()
}

async fn load_candidate_records(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &str,
) -> Result<Vec<CandidateRecord>, SdkError> {
    let rows = load_record_rows(
        transaction,
        tenant_id,
        RECORD_TYPE,
        MAX_PRIVACY_CANDIDATE_RECORDS_REHYDRATED,
        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_CANDIDATE_SCAN_LIMIT_EXCEEDED",
    )
    .await?;
    rows.into_iter().map(strict_candidate_record).collect()
}

async fn load_merge_records(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &str,
) -> Result<Vec<MergeRecord>, SdkError> {
    let rows = load_record_rows(
        transaction,
        tenant_id,
        MERGE_OPERATION_RECORD_TYPE,
        MAX_PRIVACY_MERGE_RECORDS_REHYDRATED,
        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_MERGE_SCAN_LIMIT_EXCEEDED",
    )
    .await?;
    rows.into_iter().map(strict_merge_record).collect()
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
    limit_error_code: &'static str,
) -> Result<Vec<StoredRecordRow>, SdkError> {
    let fetch_limit = i64::try_from(maximum + 1)
        .map_err(|_| limit_exceeded(limit_error_code, "record SQL limit does not fit in i64"))?;
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
        ORDER BY record_id ASC
        LIMIT $4
        "#,
    )
    .bind(tenant_id)
    .bind(crm_identity_resolution::MODULE_ID)
    .bind(record_type)
    .bind(fetch_limit)
    .fetch_all(&mut ***transaction)
    .await
    .map_err(database_unavailable)?;
    if rows.len() > maximum {
        return Err(limit_exceeded(
            limit_error_code,
            "authoritative record count exceeded the frozen privacy bound",
        ));
    }
    rows.into_iter()
        .map(|row| {
            Ok(StoredRecordRow {
                record_id: RecordId::try_new(
                    row.try_get::<String, _>("record_id")
                        .map_err(row_decode_error)?,
                )
                .map_err(|error| stored_merge_state_invalid(error.to_string()))?,
                version: row.try_get("version").map_err(row_decode_error)?,
                owner_module_id: row.try_get("owner_module_id").map_err(row_decode_error)?,
                schema_id: row.try_get("schema_id").map_err(row_decode_error)?,
                schema_version: row.try_get("schema_version").map_err(row_decode_error)?,
                descriptor_hash: row.try_get("descriptor_hash").map_err(row_decode_error)?,
                data_class: row.try_get("data_class").map_err(row_decode_error)?,
                payload_encoding: row.try_get("payload_encoding").map_err(row_decode_error)?,
                maximum_payload_size: row
                    .try_get("maximum_payload_size")
                    .map_err(row_decode_error)?,
                retention_policy_id: row
                    .try_get("retention_policy_id")
                    .map_err(row_decode_error)?,
                payload_bytes: row.try_get("payload_bytes").map_err(row_decode_error)?,
            })
        })
        .collect()
}

fn strict_candidate_record(row: StoredRecordRow) -> Result<CandidateRecord, SdkError> {
    let expected_descriptor_hash = duplicate_candidate_case_state_descriptor_hash();
    if row.version <= 0
        || row.owner_module_id != crm_identity_resolution::MODULE_ID
        || row.schema_id != DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_ID
        || row.schema_version != DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_VERSION
        || row.descriptor_hash.as_slice() != expected_descriptor_hash
        || row.data_class != "personal"
        || row.payload_encoding != "json"
        || row.maximum_payload_size != DUPLICATE_CANDIDATE_CASE_STATE_MAXIMUM_BYTES as i64
        || row.retention_policy_id != DUPLICATE_CANDIDATE_CASE_STATE_RETENTION_POLICY_ID
    {
        return Err(stored_candidate_state_invalid(
            "persisted candidate metadata does not match the canonical state contract",
        ));
    }
    let snapshot = record_snapshot(
        RECORD_TYPE,
        row.record_id.clone(),
        row.version,
        DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_ID,
        DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_VERSION,
        expected_descriptor_hash,
        DUPLICATE_CANDIDATE_CASE_STATE_MAXIMUM_BYTES,
        DUPLICATE_CANDIDATE_CASE_STATE_RETENTION_POLICY_ID,
        row.payload_bytes,
    )?;
    let candidate = duplicate_candidate_case_from_snapshot(&snapshot).map_err(|error| {
        stored_candidate_state_invalid(format!("{}: {}", error.code, error.safe_message))
    })?;
    if candidate.case_id().as_str() != row.record_id.as_str() || candidate.version() != row.version
    {
        return Err(stored_candidate_state_invalid(
            "candidate record identity/version disagrees with its authoritative payload",
        ));
    }
    Ok(CandidateRecord {
        record_id: row.record_id,
        candidate,
    })
}

fn strict_merge_record(row: StoredRecordRow) -> Result<MergeRecord, SdkError> {
    let expected_descriptor_hash = merge_operation_state_descriptor_hash();
    if row.version <= 0
        || row.owner_module_id != crm_identity_resolution::MODULE_ID
        || row.schema_id != MERGE_OPERATION_STATE_SCHEMA_ID
        || row.schema_version != MERGE_OPERATION_STATE_SCHEMA_VERSION
        || row.descriptor_hash.as_slice() != expected_descriptor_hash
        || row.data_class != "personal"
        || row.payload_encoding != "json"
        || row.maximum_payload_size != MERGE_OPERATION_STATE_MAXIMUM_BYTES as i64
        || row.retention_policy_id != MERGE_OPERATION_STATE_RETENTION_POLICY_ID
    {
        return Err(stored_merge_state_invalid(
            "persisted merge metadata does not match the canonical state contract",
        ));
    }
    let snapshot = record_snapshot(
        MERGE_OPERATION_RECORD_TYPE,
        row.record_id.clone(),
        row.version,
        MERGE_OPERATION_STATE_SCHEMA_ID,
        MERGE_OPERATION_STATE_SCHEMA_VERSION,
        expected_descriptor_hash,
        MERGE_OPERATION_STATE_MAXIMUM_BYTES,
        MERGE_OPERATION_STATE_RETENTION_POLICY_ID,
        row.payload_bytes,
    )?;
    let operation = merge_operation_from_snapshot(&snapshot).map_err(|error| {
        stored_merge_state_invalid(format!("{}: {}", error.code, error.safe_message))
    })?;
    if operation.operation_id().as_str() != row.record_id.as_str()
        || operation.version() != row.version
    {
        return Err(stored_merge_state_invalid(
            "merge record identity/version disagrees with its authoritative payload",
        ));
    }
    Ok(MergeRecord {
        record_id: row.record_id,
        operation,
    })
}

#[allow(clippy::too_many_arguments)]
fn record_snapshot(
    record_type: &str,
    record_id: RecordId,
    version: i64,
    schema: &str,
    schema_version_value: &str,
    descriptor_hash: [u8; 32],
    maximum_size: u64,
    retention: &str,
    bytes: Vec<u8>,
) -> Result<RecordSnapshot, SdkError> {
    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: configured(RecordType::try_new(record_type))?,
            record_id,
        },
        version,
        payload: TypedPayload {
            owner: configured(ModuleId::try_new(crm_identity_resolution::MODULE_ID))?,
            schema_id: configured(SchemaId::try_new(schema))?,
            schema_version: configured(SchemaVersion::try_new(schema_version_value))?,
            descriptor_hash,
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: maximum_size,
            retention_policy_id: configured(RetentionPolicyId::try_new(retention))?,
            bytes,
        },
    })
}

struct RelevantResource {
    record_id: RecordId,
    resource_version: u64,
}

fn validate_and_select_candidates(
    records: &[CandidateRecord],
    relationship_ids: &BTreeSet<RecordId>,
    aliases: &BTreeSet<PartyReference>,
    graph: &CanonicalPartyGraph,
    canonical_party_id: &RecordId,
) -> Result<Vec<RelevantResource>, SdkError> {
    let canonical = PartyReference::try_new(canonical_party_id.as_str())
        .map_err(|error| stored_candidate_state_invalid(error.to_string()))?;
    let mut matched = BTreeSet::new();
    let mut output = Vec::new();
    for record in records {
        let endpoints = [
            record.candidate.pair().left(),
            record.candidate.pair().right(),
        ];
        let relevant = endpoints.iter().any(|party| {
            aliases.contains(*party)
                && graph
                    .resolve(party)
                    .is_ok_and(|resolution| resolution.canonical_party_ref() == &canonical)
        });
        if relevant {
            if !relationship_ids.contains(&record.record_id) {
                return Err(stored_candidate_state_invalid(
                    "relevant candidate case is missing its owner relationship index",
                ));
            }
            matched.insert(record.record_id.clone());
            output.push(RelevantResource {
                record_id: record.record_id.clone(),
                resource_version: u64::try_from(record.candidate.version()).map_err(|_| {
                    stored_candidate_state_invalid("candidate version must be positive")
                })?,
            });
        }
    }
    if &matched != relationship_ids {
        return Err(stored_candidate_state_invalid(
            "candidate relationship index contains missing or unrelated records",
        ));
    }
    Ok(output)
}

fn validate_and_select_merges(
    records: &[MergeRecord],
    relationship_ids: &BTreeSet<RecordId>,
    aliases: &BTreeSet<PartyReference>,
    graph: &CanonicalPartyGraph,
    canonical_party_id: &RecordId,
) -> Result<Vec<RelevantResource>, SdkError> {
    let canonical = PartyReference::try_new(canonical_party_id.as_str())
        .map_err(|error| stored_merge_state_invalid(error.to_string()))?;
    let mut matched_direct = BTreeSet::new();
    let mut output = Vec::new();
    for record in records {
        let operation = &record.operation;
        let source_relevant = reference_relevant(
            operation.source_party_ref(),
            operation.status(),
            aliases,
            graph,
            &canonical,
        );
        let survivor_relevant = reference_relevant(
            operation.survivor_party_ref(),
            operation.status(),
            aliases,
            graph,
            &canonical,
        );
        let provenance_relevant = operation.survivorship().iter().any(|selection| {
            reference_relevant(
                selection.provenance_party_ref(),
                operation.status(),
                aliases,
                graph,
                &canonical,
            )
        });
        let direct = source_relevant || survivor_relevant;
        if direct {
            if !relationship_ids.contains(&record.record_id) {
                return Err(stored_merge_state_invalid(
                    "directly relevant merge operation is missing its owner relationship index",
                ));
            }
            matched_direct.insert(record.record_id.clone());
        }
        if direct || provenance_relevant {
            output.push(RelevantResource {
                record_id: record.record_id.clone(),
                resource_version: u64::try_from(operation.version()).map_err(|_| {
                    stored_merge_state_invalid("merge operation version must be positive")
                })?,
            });
        }
    }
    if &matched_direct != relationship_ids {
        return Err(stored_merge_state_invalid(
            "merge relationship index contains missing or unrelated records",
        ));
    }
    Ok(output)
}

fn reference_relevant(
    party: &PartyReference,
    status: MergeOperationStatus,
    aliases: &BTreeSet<PartyReference>,
    graph: &CanonicalPartyGraph,
    canonical: &PartyReference,
) -> bool {
    if !aliases.contains(party) {
        return false;
    }
    match status {
        MergeOperationStatus::Unmerged => true,
        MergeOperationStatus::Active => graph
            .resolve(party)
            .is_ok_and(|resolution| resolution.canonical_party_ref() == canonical),
    }
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    crate::errors::configured(value)
}

#[allow(dead_code)]
fn _definition_smoke() -> Result<CapabilityDefinition, SdkError> {
    identity_resolution_privacy_scope_definition()
}
