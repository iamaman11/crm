#![forbid(unsafe_code)]

//! Permission-aware search mechanics over rebuildable projection documents.
//!
//! The search store returns tenant-scoped candidates from a rebuildable index.
//! Every candidate is then checked with the live query visibility authorizer
//! before any resource identity, field value or match metadata is returned.

use crm_core_events::ProjectionDocumentWrite;
use crm_module_sdk::{
    ErrorCategory, ModuleId, PortFuture, RecordId, RecordRef, RecordType, SdkError, TenantId,
};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, CursorError, PageSizePolicy, QueryRequest,
    QueryVisibilityAuthorizer,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

pub const DEFAULT_SEARCH_PAGE_SIZE: u32 = 25;
pub const MAXIMUM_SEARCH_PAGE_SIZE: u32 = 100;
pub const DEFAULT_CANDIDATE_BATCH_SIZE: u32 = 100;
pub const MAXIMUM_SEARCH_TEXT_BYTES: usize = 512;
pub const MAXIMUM_SEARCH_FIELD_BYTES: usize = 8_192;
pub const SEARCH_RESULT_CURSOR_RESOURCE_TYPE: &str = "search.result";
pub const SEARCH_SORT_ID: &str = "rank_desc_resource_type_asc_resource_id_asc";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SearchIndexId(String);

impl SearchIndexId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        let value = value.into();
        validate_coordinate(&value, 180, "SEARCH_INDEX_ID_INVALID")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SearchIndexId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchProjectionDocument {
    pub index_id: SearchIndexId,
    pub generation_id: String,
    pub schema_version: String,
    pub owner_module_id: ModuleId,
    pub resource: RecordRef,
    pub source_version: i64,
    pub searchable_fields: BTreeMap<String, String>,
    pub display_fields: BTreeMap<String, String>,
}

impl SearchProjectionDocument {
    pub fn validate(&self) -> Result<(), SdkError> {
        validate_coordinate(&self.generation_id, 180, "SEARCH_GENERATION_ID_INVALID")?;
        validate_coordinate(&self.schema_version, 120, "SEARCH_SCHEMA_VERSION_INVALID")?;
        if self.source_version <= 0 {
            return Err(search_invalid(
                "SEARCH_SOURCE_VERSION_INVALID",
                "The search source version is invalid.",
            ));
        }
        if self.searchable_fields.is_empty() {
            return Err(search_invalid(
                "SEARCH_FIELDS_EMPTY",
                "The search document has no searchable fields.",
            ));
        }
        validate_fields(&self.searchable_fields)?;
        validate_fields(&self.display_fields)?;
        Ok(())
    }

    pub fn into_projection_write(self) -> Result<ProjectionDocumentWrite, SdkError> {
        self.validate()?;
        let search_text = self
            .searchable_fields
            .values()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        Ok(ProjectionDocumentWrite {
            resource_type: self.resource.record_type.as_str().to_owned(),
            resource_id: self.resource.record_id.as_str().to_owned(),
            source_version: self.source_version,
            document: json!({
                "index_id": self.index_id.as_str(),
                "generation_id": self.generation_id,
                "schema_version": self.schema_version,
                "owner_module_id": self.owner_module_id.as_str(),
                "search_text": search_text,
                "searchable_fields": self.searchable_fields,
                "display_fields": self.display_fields,
            }),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    pub index_id: SearchIndexId,
    pub text: String,
    pub resource_types: BTreeSet<RecordType>,
    pub page_size: i32,
    pub cursor: Option<String>,
}

impl SearchRequest {
    pub fn validate(&self) -> Result<(), SdkError> {
        let normalized = normalize_search_text(&self.text)?;
        if normalized.is_empty() {
            return Err(search_invalid(
                "SEARCH_TEXT_EMPTY",
                "The search text must not be empty.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCandidateCursor {
    pub rank_micros: i64,
    pub resource_type: RecordType,
    pub resource_id: RecordId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCandidateRequest {
    pub tenant_id: TenantId,
    pub index_id: SearchIndexId,
    pub normalized_text: String,
    pub resource_types: BTreeSet<RecordType>,
    pub after: Option<SearchCandidateCursor>,
    pub page_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCandidate {
    pub owner_module_id: ModuleId,
    pub resource: RecordRef,
    pub source_version: i64,
    pub rank_micros: i64,
    pub searchable_fields: BTreeMap<String, String>,
    pub display_fields: BTreeMap<String, String>,
}

impl SearchCandidate {
    fn cursor(&self) -> SearchCandidateCursor {
        SearchCandidateCursor {
            rank_micros: self.rank_micros,
            resource_type: self.resource.record_type.clone(),
            resource_id: self.resource.record_id.clone(),
        }
    }

    fn validate(&self) -> Result<(), SdkError> {
        if self.source_version <= 0 || self.rank_micros <= 0 {
            return Err(search_internal(
                "SEARCH_CANDIDATE_INVALID",
                "The search service returned an invalid candidate.",
            ));
        }
        validate_fields(&self.searchable_fields)?;
        validate_fields(&self.display_fields)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCandidatePage {
    pub candidates: Vec<SearchCandidate>,
    pub next_after: Option<SearchCandidateCursor>,
}

pub trait SearchCandidateStore: Send + Sync {
    fn search_candidates<'a>(
        &'a self,
        request: SearchCandidateRequest,
    ) -> PortFuture<'a, Result<SearchCandidatePage, SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub owner_module_id: ModuleId,
    pub resource: RecordRef,
    pub source_version: i64,
    pub rank_micros: i64,
    pub fields: BTreeMap<String, String>,
    pub matched_fields: BTreeSet<String>,
    pub visibility_decision_id: String,
    pub visibility_policy_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub next_cursor: Option<String>,
}

#[derive(Clone)]
pub struct PermissionAwareSearch {
    store: Arc<dyn SearchCandidateStore>,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    cursor_codec: CursorCodec,
    page_policy: PageSizePolicy,
    candidate_batch_size: u32,
}

impl fmt::Debug for PermissionAwareSearch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PermissionAwareSearch")
            .field("store", &"dyn SearchCandidateStore")
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .field("candidate_batch_size", &self.candidate_batch_size)
            .finish()
    }
}

impl PermissionAwareSearch {
    pub fn new(
        store: Arc<dyn SearchCandidateStore>,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
        cursor_codec: CursorCodec,
    ) -> Result<Self, SdkError> {
        let page_policy = PageSizePolicy {
            default_size: DEFAULT_SEARCH_PAGE_SIZE,
            maximum_size: MAXIMUM_SEARCH_PAGE_SIZE,
        }
        .validate()
        .map_err(cursor_error)?;
        Ok(Self {
            store,
            visibility,
            cursor_codec,
            page_policy,
            candidate_batch_size: DEFAULT_CANDIDATE_BATCH_SIZE,
        })
    }

    pub async fn search(
        &self,
        query_request: &QueryRequest,
        request: SearchRequest,
    ) -> Result<SearchPage, SdkError> {
        request.validate()?;
        let normalized_text = normalize_search_text(&request.text)?;
        let page_size = self.page_policy.resolve(request.page_size).map_err(cursor_error)?;
        let binding = search_cursor_binding(query_request, &request, page_size)?;
        let mut after = request
            .cursor
            .as_deref()
            .map(|cursor| self.cursor_codec.decode(cursor, &binding).map_err(cursor_error))
            .transpose()?
            .map(decode_candidate_cursor)
            .transpose()?;
        let mut hits = Vec::with_capacity(page_size as usize);
        let mut continuation = after.clone();
        let mut has_more = false;

        loop {
            let page = self
                .store
                .search_candidates(SearchCandidateRequest {
                    tenant_id: query_request.context.tenant_id.clone(),
                    index_id: request.index_id.clone(),
                    normalized_text: normalized_text.clone(),
                    resource_types: request.resource_types.clone(),
                    after: after.clone(),
                    page_size: self.candidate_batch_size,
                })
                .await?;

            if page.candidates.is_empty() {
                if page.next_after.is_some() {
                    return Err(search_internal(
                        "SEARCH_STORE_CURSOR_INVALID",
                        "The search service returned an invalid continuation.",
                    ));
                }
                has_more = false;
                break;
            }

            for (position, candidate) in page.candidates.iter().enumerate() {
                candidate.validate()?;
                let candidate_cursor = candidate.cursor();
                continuation = Some(candidate_cursor.clone());
                let mut visibility_request = query_request.clone();
                visibility_request.owner_module_id = candidate.owner_module_id.clone();
                let decision = self
                    .visibility
                    .authorize_visibility(&visibility_request, &candidate.resource)
                    .await?;
                if !decision.resource_visible {
                    continue;
                }

                let searchable_fields = candidate
                    .searchable_fields
                    .iter()
                    .filter(|(field, _)| decision.allows_field(field))
                    .map(|(field, value)| (field.clone(), value.clone()))
                    .collect::<BTreeMap<_, _>>();
                let matched_fields = searchable_fields
                    .iter()
                    .filter(|(_, value)| normalized_contains(value, &normalized_text))
                    .map(|(field, _)| field.clone())
                    .collect::<BTreeSet<_>>();
                if matched_fields.is_empty() {
                    continue;
                }

                let fields = candidate
                    .display_fields
                    .iter()
                    .chain(candidate.searchable_fields.iter())
                    .filter(|(field, _)| decision.allows_field(field))
                    .map(|(field, value)| (field.clone(), value.clone()))
                    .collect::<BTreeMap<_, _>>();
                hits.push(SearchHit {
                    owner_module_id: candidate.owner_module_id.clone(),
                    resource: candidate.resource.clone(),
                    source_version: candidate.source_version,
                    rank_micros: candidate.rank_micros,
                    fields,
                    matched_fields,
                    visibility_decision_id: decision.decision_id,
                    visibility_policy_version: decision.policy_version,
                });

                if hits.len() == page_size as usize {
                    has_more = position + 1 < page.candidates.len() || page.next_after.is_some();
                    break;
                }
            }

            if hits.len() == page_size as usize {
                break;
            }
            let Some(next_after) = page.next_after else {
                has_more = false;
                break;
            };
            if after.as_ref() == Some(&next_after) {
                return Err(search_internal(
                    "SEARCH_STORE_CURSOR_STALLED",
                    "The search service returned an invalid continuation.",
                ));
            }
            after = Some(next_after);
            has_more = true;
        }

        let next_cursor = if has_more {
            continuation
                .as_ref()
                .map(|cursor| {
                    self.cursor_codec
                        .encode(&binding, &encode_candidate_cursor(cursor)?)
                        .map_err(cursor_error)
                })
                .transpose()?
        } else {
            None
        };

        Ok(SearchPage { hits, next_cursor })
    }
}

fn search_cursor_binding(
    query_request: &QueryRequest,
    request: &SearchRequest,
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    Ok(CursorBinding {
        tenant_id: query_request.context.tenant_id.clone(),
        actor_id: Some(query_request.context.actor_id.clone()),
        capability_id: query_request.context.capability_id.clone(),
        capability_version: query_request.context.capability_version.clone(),
        resource_type: RecordType::try_new(SEARCH_RESULT_CURSOR_RESOURCE_TYPE)
            .map_err(|error| search_internal("SEARCH_CURSOR_RESOURCE_INVALID", error.to_string()))?,
        normalized_filter_hash: search_filter_hash(request)?,
        sort_id: SEARCH_SORT_ID.to_owned(),
        page_size,
    })
}

fn search_filter_hash(request: &SearchRequest) -> Result<[u8; 32], SdkError> {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"crm.search-filter/v1");
    hash_field(&mut hasher, request.index_id.as_str().as_bytes());
    hash_field(&mut hasher, normalize_search_text(&request.text)?.as_bytes());
    for resource_type in &request.resource_types {
        hash_field(&mut hasher, resource_type.as_str().as_bytes());
    }
    Ok(hasher.finalize().into())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn encode_candidate_cursor(
    cursor: &SearchCandidateCursor,
) -> Result<CursorContinuation, SdkError> {
    let mut sort_key = Vec::with_capacity(8 + cursor.resource_type.as_str().len());
    sort_key.extend_from_slice(&cursor.rank_micros.to_be_bytes());
    sort_key.extend_from_slice(cursor.resource_type.as_str().as_bytes());
    Ok(CursorContinuation {
        sort_key,
        record_id: cursor.resource_id.clone(),
    })
}

fn decode_candidate_cursor(
    continuation: CursorContinuation,
) -> Result<SearchCandidateCursor, SdkError> {
    if continuation.sort_key.len() <= 8 {
        return Err(search_invalid(
            "SEARCH_CURSOR_INVALID",
            "The search cursor is invalid.",
        ));
    }
    let rank_micros = i64::from_be_bytes(
        continuation.sort_key[..8]
            .try_into()
            .map_err(|_| search_invalid("SEARCH_CURSOR_INVALID", "The search cursor is invalid."))?,
    );
    if rank_micros <= 0 {
        return Err(search_invalid(
            "SEARCH_CURSOR_INVALID",
            "The search cursor is invalid.",
        ));
    }
    let resource_type = std::str::from_utf8(&continuation.sort_key[8..])
        .map_err(|_| search_invalid("SEARCH_CURSOR_INVALID", "The search cursor is invalid."))?;
    Ok(SearchCandidateCursor {
        rank_micros,
        resource_type: RecordType::try_new(resource_type)
            .map_err(|_| search_invalid("SEARCH_CURSOR_INVALID", "The search cursor is invalid."))?,
        resource_id: continuation.record_id,
    })
}

fn normalize_search_text(value: &str) -> Result<String, SdkError> {
    if value.len() > MAXIMUM_SEARCH_TEXT_BYTES {
        return Err(search_invalid(
            "SEARCH_TEXT_TOO_LARGE",
            "The search text is too large.",
        ));
    }
    Ok(value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase())
}

fn normalized_contains(value: &str, normalized_text: &str) -> bool {
    value.to_lowercase().contains(normalized_text)
}

fn validate_coordinate(value: &str, maximum: usize, code: &'static str) -> Result<(), SdkError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(search_invalid(code, "The search coordinate is invalid."));
    }
    Ok(())
}

fn validate_fields(fields: &BTreeMap<String, String>) -> Result<(), SdkError> {
    for (name, value) in fields {
        if name.is_empty()
            || name.len() > 180
            || name.chars().any(char::is_control)
            || value.len() > MAXIMUM_SEARCH_FIELD_BYTES
        {
            return Err(search_invalid(
                "SEARCH_FIELD_INVALID",
                "The search document contains an invalid field.",
            ));
        }
    }
    Ok(())
}

fn cursor_error(error: CursorError) -> SdkError {
    SdkError::new(
        error.code(),
        ErrorCategory::InvalidArgument,
        false,
        error.safe_message(),
    )
}

fn search_invalid(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, safe_message)
}

fn search_internal(code: &'static str, internal: impl Into<String>) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Unavailable,
        true,
        "The search service is temporarily unavailable.",
    )
    .with_internal_reference(internal)
}

/// Architecture marker for `crm-search-runtime`.
pub const CRATE_NAME: &str = "crm-search-runtime";

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, PayloadEncoding,
        RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TraceId, TypedPayload,
    };
    use crm_query_runtime::{QueryExecutionContext, QueryVisibilityDecision};
    use std::sync::Mutex;

    #[derive(Default)]
    struct TestStore {
        candidates: Mutex<Vec<SearchCandidate>>,
    }

    impl SearchCandidateStore for TestStore {
        fn search_candidates<'a>(
            &'a self,
            request: SearchCandidateRequest,
        ) -> PortFuture<'a, Result<SearchCandidatePage, SdkError>> {
            Box::pin(async move {
                let candidates = self.candidates.lock().unwrap();
                let mut filtered = candidates
                    .iter()
                    .filter(|candidate| {
                        request.resource_types.is_empty()
                            || request.resource_types.contains(&candidate.resource.record_type)
                    })
                    .filter(|candidate| {
                        candidate
                            .searchable_fields
                            .values()
                            .any(|value| normalized_contains(value, &request.normalized_text))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                filtered.sort_by(|left, right| {
                    right
                        .rank_micros
                        .cmp(&left.rank_micros)
                        .then(left.resource.record_type.cmp(&right.resource.record_type))
                        .then(left.resource.record_id.cmp(&right.resource.record_id))
                });
                if let Some(after) = &request.after {
                    filtered.retain(|candidate| candidate_after(candidate, after));
                }
                let limit = request.page_size as usize;
                let has_more = filtered.len() > limit;
                filtered.truncate(limit);
                let next_after = has_more && !filtered.is_empty().then(|| ());
                let next_after = if has_more {
                    filtered.last().map(SearchCandidate::cursor)
                } else {
                    None
                };
                let _ = next_after;
                Ok(SearchCandidatePage {
                    candidates: filtered.clone(),
                    next_after: if has_more {
                        filtered.last().map(SearchCandidate::cursor)
                    } else {
                        None
                    },
                })
            })
        }
    }

    fn candidate_after(candidate: &SearchCandidate, after: &SearchCandidateCursor) -> bool {
        candidate.rank_micros < after.rank_micros
            || (candidate.rank_micros == after.rank_micros
                && (candidate.resource.record_type > after.resource_type
                    || (candidate.resource.record_type == after.resource_type
                        && candidate.resource.record_id > after.resource_id)))
    }

    #[derive(Default)]
    struct TestVisibility {
        decisions: Mutex<BTreeMap<String, BTreeSet<String>>>,
    }

    impl QueryVisibilityAuthorizer for TestVisibility {
        fn authorize_visibility<'a>(
            &'a self,
            _request: &'a QueryRequest,
            resource: &'a RecordRef,
        ) -> PortFuture<'a, Result<QueryVisibilityDecision, SdkError>> {
            Box::pin(async move {
                let decisions = self.decisions.lock().unwrap();
                let Some(fields) = decisions.get(resource.record_id.as_str()) else {
                    return Ok(QueryVisibilityDecision::denied("deny", "test/v1"));
                };
                Ok(QueryVisibilityDecision {
                    resource_visible: true,
                    allowed_fields: fields.clone(),
                    decision_id: format!("allow:{}", resource.record_id),
                    policy_version: "test/v1".to_owned(),
                })
            })
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn hidden_resource_and_hidden_matching_field_are_non_disclosing() {
        let store = Arc::new(TestStore {
            candidates: Mutex::new(vec![
                candidate("deal-hidden", "Secret merger", 3_000_000),
                candidate("deal-field-hidden", "Secret acquisition", 2_000_000),
                candidate("deal-visible", "Secret renewal", 1_000_000),
            ]),
        });
        let visibility = Arc::new(TestVisibility::default());
        visibility.decisions.lock().unwrap().insert(
            "deal-field-hidden".to_owned(),
            BTreeSet::from(["amount".to_owned()]),
        );
        visibility.decisions.lock().unwrap().insert(
            "deal-visible".to_owned(),
            BTreeSet::from(["name".to_owned()]),
        );
        let runtime = PermissionAwareSearch::new(
            store,
            visibility,
            CursorCodec::new([0x42; 32]).unwrap(),
        )
        .unwrap();

        let result = runtime
            .search(
                &query_request(),
                SearchRequest {
                    index_id: SearchIndexId::try_new("crm.global-search").unwrap(),
                    text: "secret".to_owned(),
                    resource_types: BTreeSet::new(),
                    page_size: 25,
                    cursor: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].resource.record_id.as_str(), "deal-visible");
        assert_eq!(result.hits[0].matched_fields, BTreeSet::from(["name".to_owned()]));
        assert!(!result.hits[0].fields.values().any(|value| value.contains("acquisition")));
    }

    #[test]
    fn projection_document_has_deterministic_search_text_and_no_permission_snapshot() {
        let write = SearchProjectionDocument {
            index_id: SearchIndexId::try_new("crm.global-search").unwrap(),
            generation_id: "generation-1".to_owned(),
            schema_version: "1".to_owned(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            resource: RecordRef {
                record_type: RecordType::try_new("sales.deal").unwrap(),
                record_id: RecordId::try_new("deal-1").unwrap(),
            },
            source_version: 1,
            searchable_fields: BTreeMap::from([("name".to_owned(), "Acme Renewal".to_owned())]),
            display_fields: BTreeMap::from([("stage".to_owned(), "proposal".to_owned())]),
        }
        .into_projection_write()
        .unwrap();

        assert_eq!(write.resource_type, "sales.deal");
        assert_eq!(write.resource_id, "deal-1");
        assert_eq!(write.document["search_text"], "Acme Renewal");
        assert!(write.document.get("allowed_fields").is_none());
        assert!(write.document.get("actor_id").is_none());
    }

    fn candidate(record_id: &str, name: &str, rank_micros: i64) -> SearchCandidate {
        SearchCandidate {
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            resource: RecordRef {
                record_type: RecordType::try_new("sales.deal").unwrap(),
                record_id: RecordId::try_new(record_id).unwrap(),
            },
            source_version: 1,
            rank_micros,
            searchable_fields: BTreeMap::from([("name".to_owned(), name.to_owned())]),
            display_fields: BTreeMap::from([("amount".to_owned(), "1000".to_owned())]),
        }
    }

    fn query_request() -> QueryRequest {
        QueryRequest {
            owner_module_id: ModuleId::try_new("crm.search").unwrap(),
            context: QueryExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new("request-a").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                trace_id: TraceId::try_new("trace-a").unwrap(),
                capability_id: CapabilityId::try_new("search.global.query").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 100,
            },
            input: TypedPayload {
                owner: ModuleId::try_new("crm.search").unwrap(),
                schema_id: SchemaId::try_new("crm.search.v1.SearchRequest").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Internal,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 1024,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: vec![1],
            },
            input_hash: [2; 32],
        }
    }
}
