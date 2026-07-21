#![forbid(unsafe_code)]

//! Non-runtime permission-aware suggestion queries for Customer Enrichment.
//!
//! Every disclosure is bound to live Party, suggestion and review-decision visibility. Persisted
//! records are strictly rehydrated before use and hidden resources are reported as not found.

mod list;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_customer_enrichment::{
    REVIEW_DECISION_RECORD_TYPE, ReviewDecision, SUGGESTION_RECORD_TYPE, Suggestion, SuggestionId,
    derive_suggestion_supersession,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    review_decision_from_snapshot, review_decision_to_wire, suggestion_from_snapshot,
    suggestion_to_wire_with_supersession,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer, QueryVisibilityDecision,
};
use prost::Message;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const CRATE_NAME: &str = "crm-customer-enrichment-suggestion-query-adapter";
pub const GET_SUGGESTION_CAPABILITY: &str = "customer_enrichment.suggestion.get";
pub const GET_SUGGESTION_REQUEST_SCHEMA: &str = "crm.customer_enrichment.v1.GetSuggestionRequest";
pub const GET_SUGGESTION_RESPONSE_SCHEMA: &str = "crm.customer_enrichment.v1.GetSuggestionResponse";
pub const LIST_SUGGESTIONS_BY_PARTY_CAPABILITY: &str =
    "customer_enrichment.suggestion.list_by_party";
pub const LIST_SUGGESTIONS_BY_PARTY_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.ListSuggestionsByPartyRequest";
pub const LIST_SUGGESTIONS_BY_PARTY_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.ListSuggestionsByPartyResponse";
pub const QUERY_CAPABILITY_IDS: &[&str] = &[
    GET_SUGGESTION_CAPABILITY,
    LIST_SUGGESTIONS_BY_PARTY_CAPABILITY,
];

pub(crate) const PARTY_RECORD_TYPE: &str = "parties.party";
pub(crate) const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 4_096;
pub(crate) const INTERNAL_SCAN_PAGE_SIZE: u32 = 100;

#[derive(Clone)]
pub struct CustomerEnrichmentSuggestionQueryAdapter {
    pub(crate) store: PostgresDataStore,
    pub(crate) visibility: Arc<dyn QueryVisibilityAuthorizer>,
    pub(crate) cursor_codec: Option<crm_query_runtime::CursorCodec>,
}

impl CustomerEnrichmentSuggestionQueryAdapter {
    pub fn new(
        store: PostgresDataStore,
        cursor_codec: crm_query_runtime::CursorCodec,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Self {
        Self {
            store,
            visibility,
            cursor_codec: Some(cursor_codec),
        }
    }

    pub fn new_get_only(
        store: PostgresDataStore,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Self {
        Self {
            store,
            visibility,
            cursor_codec: None,
        }
    }

    pub(crate) fn cursor_codec(&self) -> Result<&crm_query_runtime::CursorCodec, SdkError> {
        self.cursor_codec.as_ref().ok_or_else(|| {
            query_configuration_invalid("suggestion list cursor codec is not configured")
        })
    }

    async fn execute_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetSuggestionRequest =
            decode_input(request, GET_SUGGESTION_REQUEST_SCHEMA)?;
        let suggestion_id = suggestion_record_id(command.suggestion_ref)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: suggestion_record_type()?,
                record_id: suggestion_id,
            })
            .await?
            .ok_or_else(suggestion_not_found)?;
        let suggestion = suggestion_from_snapshot(&snapshot)?;
        self.ensure_party_visible(request, &suggestion).await?;
        let visible_suggestions = self
            .load_visible_suggestions_for_party(request, suggestion.target().resource_id.as_str())
            .await?;
        let visible = visible_suggestions
            .get(suggestion.suggestion_id().as_str())
            .ok_or_else(suggestion_not_found)?;
        if visible.suggestion != suggestion {
            return Err(query_state_invalid(
                "direct suggestion lookup differs from bounded lifecycle scan",
            ));
        }

        let reviews = self.load_visible_latest_reviews(request).await?;
        let latest_review = reviews.get(suggestion.suggestion_id().as_str());
        let at_unix_ms = request_started_at_unix_ms(request)?;
        let mut public_suggestion = suggestion_to_wire_with_supersession(
            &suggestion,
            latest_review.map(|review| &review.decision),
            visible.superseded_by.as_ref(),
            at_unix_ms,
        )?;
        redact_suggestion(&mut public_suggestion, |field| {
            visible.visibility.allows_field(field)
        });
        let latest_review_decision = latest_review
            .map(|review| {
                let mut output = review_decision_to_wire(&review.decision)?;
                redact_review_decision(&mut output, |field| review.visibility.allows_field(field));
                Ok(output)
            })
            .transpose()?;

        support::protobuf_payload(
            MODULE_ID,
            GET_SUGGESTION_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetSuggestionResponse {
                suggestion: Some(public_suggestion),
                latest_review_decision,
                latest_application_attempt: None,
            },
        )
    }

    pub(crate) async fn ensure_party_visible(
        &self,
        request: &QueryRequest,
        suggestion: &Suggestion,
    ) -> Result<(), SdkError> {
        let party_reference = support::record_ref(
            PARTY_RECORD_TYPE,
            suggestion.target().resource_id.as_str(),
            "customer_enrichment.suggestion.target.party_ref.party_id",
        )?;
        if self
            .visibility
            .authorize_visibility(request, &party_reference)
            .await?
            .resource_visible
        {
            Ok(())
        } else {
            Err(suggestion_not_found())
        }
    }

    pub(crate) async fn load_visible_latest_reviews(
        &self,
        request: &QueryRequest,
    ) -> Result<BTreeMap<String, VisibleReview>, SdkError> {
        let mut output = BTreeMap::<String, VisibleReview>::new();
        let mut after: Option<RecordQueryContinuation> = None;
        let mut scanned = 0_usize;
        loop {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: review_record_type()?,
                    page_size: INTERNAL_SCAN_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in page.records {
                let decision = review_decision_from_snapshot(&snapshot)?;
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if !visibility.resource_visible {
                    continue;
                }
                let public = review_decision_to_wire(&decision)?;
                let suggestion_id = public
                    .suggestion_ref
                    .as_ref()
                    .ok_or_else(|| {
                        query_state_invalid("review decision has no suggestion reference")
                    })?
                    .suggestion_id
                    .clone();
                let decision_id = public
                    .review_decision_ref
                    .as_ref()
                    .ok_or_else(|| query_state_invalid("review decision has no identity"))?
                    .review_decision_id
                    .clone();
                let candidate = VisibleReview {
                    decision,
                    visibility,
                    decided_at_unix_ms: public.decided_at_unix_ms,
                    decision_id,
                };
                let replace = output
                    .get(&suggestion_id)
                    .map(|current| candidate.ordering_key() > current.ordering_key())
                    .unwrap_or(true);
                if replace {
                    output.insert(suggestion_id, candidate);
                }
            }
            after = page.next;
            if after.is_none() {
                return Ok(output);
            }
        }
    }
    pub(crate) async fn load_visible_suggestions_for_party(
        &self,
        request: &QueryRequest,
        party_id: &str,
    ) -> Result<BTreeMap<String, VisibleSuggestion>, SdkError> {
        let mut values = Vec::<(Suggestion, QueryVisibilityDecision)>::new();
        let mut after: Option<RecordQueryContinuation> = None;
        let mut scanned = 0_usize;
        loop {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: suggestion_record_type()?,
                    page_size: INTERNAL_SCAN_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in page.records {
                let suggestion = suggestion_from_snapshot(&snapshot)?;
                if suggestion.target().resource_id.as_str() != party_id {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    values.push((suggestion, visibility));
                }
            }
            after = page.next;
            if after.is_none() {
                break;
            }
        }

        let supersession =
            derive_suggestion_supersession(values.iter().map(|(suggestion, _)| suggestion));
        let mut output = BTreeMap::new();
        for (suggestion, visibility) in values {
            let superseded_by = supersession.get(suggestion.suggestion_id()).cloned();
            output.insert(
                suggestion.suggestion_id().as_str().to_owned(),
                VisibleSuggestion {
                    suggestion,
                    visibility,
                    superseded_by,
                },
            );
        }
        Ok(output)
    }
}

impl std::fmt::Debug for CustomerEnrichmentSuggestionQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentSuggestionQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec_configured", &self.cursor_codec.is_some())
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .finish()
    }
}

impl QuerySemanticValidator for CustomerEnrichmentSuggestionQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            match definition.capability_id.as_str() {
                GET_SUGGESTION_CAPABILITY => {
                    let command: wire::GetSuggestionRequest =
                        decode_input(request, GET_SUGGESTION_REQUEST_SCHEMA)?;
                    suggestion_record_id(command.suggestion_ref).map(|_| ())
                }
                LIST_SUGGESTIONS_BY_PARTY_CAPABILITY => list::validate(self, request),
                _ => Err(unsupported_query()),
            }
        })
    }
}

impl QueryExecutor for CustomerEnrichmentSuggestionQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            let output = match definition.capability_id.as_str() {
                GET_SUGGESTION_CAPABILITY => self.execute_get(&request).await?,
                LIST_SUGGESTIONS_BY_PARTY_CAPABILITY => list::execute(self, &request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

#[derive(Clone)]
pub(crate) struct VisibleSuggestion {
    pub(crate) suggestion: Suggestion,
    pub(crate) visibility: QueryVisibilityDecision,
    pub(crate) superseded_by: Option<SuggestionId>,
}

#[derive(Clone)]
pub(crate) struct VisibleReview {
    pub(crate) decision: ReviewDecision,
    pub(crate) visibility: QueryVisibilityDecision,
    decided_at_unix_ms: i64,
    decision_id: String,
}

impl VisibleReview {
    fn ordering_key(&self) -> (i64, &str) {
        (self.decided_at_unix_ms, self.decision_id.as_str())
    }
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        get_suggestion_capability_definition()?,
        list_suggestions_by_party_capability_definition()?,
    ])
}

pub fn get_suggestion_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(
        GET_SUGGESTION_CAPABILITY,
        GET_SUGGESTION_REQUEST_SCHEMA,
        GET_SUGGESTION_RESPONSE_SCHEMA,
    )
}

pub fn list_suggestions_by_party_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(
        LIST_SUGGESTIONS_BY_PARTY_CAPABILITY,
        LIST_SUGGESTIONS_BY_PARTY_REQUEST_SCHEMA,
        LIST_SUGGESTIONS_BY_PARTY_RESPONSE_SCHEMA,
    )
}

fn query_definition(
    capability_id: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            request_schema,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            response_schema,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub(crate) fn decode_input<T: Message + Default>(
    request: &QueryRequest,
    schema: &'static str,
) -> Result<T, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The suggestion query input does not match the required contract.",
        ));
    }
    T::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The suggestion query input is not valid Protobuf.",
        )
    })
}

fn suggestion_record_id(value: Option<wire::SuggestionRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion_ref",
            "Suggestion reference is required",
        )
    })?;
    RecordId::try_new(value.suggestion_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion_ref.suggestion_id",
            error.to_string(),
        )
    })
}

pub(crate) fn request_started_at_unix_ms(request: &QueryRequest) -> Result<u64, SdkError> {
    let nanos = request.context.request_started_at_unix_nanos;
    if nanos <= 0 {
        return Err(query_configuration_invalid(
            "query start timestamp must be positive",
        ));
    }
    u64::try_from(nanos / 1_000_000)
        .map_err(|_| query_configuration_invalid("query timestamp exceeds u64"))
}

pub(crate) fn module_id() -> Result<ModuleId, SdkError> {
    configured(ModuleId::try_new(MODULE_ID))
}

pub(crate) fn suggestion_record_type() -> Result<RecordType, SdkError> {
    configured(RecordType::try_new(SUGGESTION_RECORD_TYPE))
}

fn review_record_type() -> Result<RecordType, SdkError> {
    configured(RecordType::try_new(REVIEW_DECISION_RECORD_TYPE))
}

pub(crate) fn party_record_type() -> Result<RecordType, SdkError> {
    configured(RecordType::try_new(PARTY_RECORD_TYPE))
}

pub(crate) fn redact_suggestion(
    output: &mut wire::Suggestion,
    allows_field: impl Fn(&str) -> bool,
) {
    if !allows_field("enrichment_request_ref") {
        output.enrichment_request_ref = None;
    }
    if !allows_field("provider_response_receipt_ref") {
        output.provider_response_receipt_ref = None;
    }
    if !allows_field("provider_profile_version_ref") {
        output.provider_profile_version_ref = None;
    }
    if !allows_field("mapping_version_ref") {
        output.mapping_version_ref = None;
    }
    if !allows_field("target") {
        output.target = None;
    }
    if !allows_field("proposed_value") {
        output.proposed_value.clear();
    }
    if !allows_field("proposed_value_digest") {
        output.proposed_value_digest.clear();
    }
    if !allows_field("observed_at_unix_ms") {
        output.observed_at_unix_ms = None;
    }
    if !allows_field("retrieved_at_unix_ms") {
        output.retrieved_at_unix_ms = 0;
    }
    if !allows_field("effective_at_unix_ms") {
        output.effective_at_unix_ms = 0;
    }
    if !allows_field("fresh_until_unix_ms") {
        output.fresh_until_unix_ms = 0;
    }
    if !allows_field("expires_at_unix_ms") {
        output.expires_at_unix_ms = 0;
    }
    if !allows_field("confidence_basis_points") {
        output.confidence_basis_points = None;
    }
    if !allows_field("policy_evidence") {
        output.policy_evidence = None;
    }
    if !allows_field("evidence_references") {
        output.evidence_references.clear();
    }
    if !allows_field("lifecycle_status") {
        output.lifecycle_status = wire::SuggestionLifecycleStatus::Unspecified as i32;
    }
    if !allows_field("superseded_by_suggestion_ref") {
        output.superseded_by_suggestion_ref = None;
    }
}

fn redact_review_decision(output: &mut wire::ReviewDecision, allows_field: impl Fn(&str) -> bool) {
    if !allows_field("suggestion_ref") {
        output.suggestion_ref = None;
    }
    if !allows_field("target_party_resource_version") {
        output.target_party_resource_version = 0;
    }
    if !allows_field("proposed_value_digest") {
        output.proposed_value_digest.clear();
    }
    if !allows_field("reviewed_by_actor_id") {
        output.reviewed_by_actor_id.clear();
    }
    if !allows_field("kind") {
        output.kind = wire::SuggestionReviewDecisionKind::Unspecified as i32;
    }
    if !allows_field("policy_version") {
        output.policy_version.clear();
    }
    if !allows_field("safe_reason_code") {
        output.safe_reason_code.clear();
    }
    if !allows_field("approval_evidence_reference") {
        output.approval_evidence_reference = None;
    }
    if !allows_field("decided_at_unix_ms") {
        output.decided_at_unix_ms = 0;
    }
    if !allows_field("expires_at_unix_ms") {
        output.expires_at_unix_ms = None;
    }
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || !QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || definition.mutation
    {
        return Err(unsupported_query());
    }
    Ok(())
}

pub(crate) fn enforce_scan_limit(scanned: usize) -> Result<(), SdkError> {
    if scanned > MAXIMUM_VISIBILITY_SCAN_RECORDS {
        Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_SCAN_LIMIT_EXCEEDED",
            ErrorCategory::Unavailable,
            true,
            "The suggestion query is temporarily unavailable.",
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn configured<T>(
    value: Result<T, crm_module_sdk::IdentifierError>,
) -> Result<T, SdkError> {
    value.map_err(query_configuration_invalid)
}

pub(crate) fn suggestion_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested suggestion was not found.",
    )
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The suggestion query is not configured.",
    )
}

pub(crate) fn query_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored suggestion query evidence is invalid.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn query_configuration_invalid(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The suggestion query configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_two_personal_low_risk_queries() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        for definition in definitions {
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(definition.capability_version.as_str(), "1.0.0");
            assert_eq!(
                definition.input_contract.allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert_eq!(definition.risk, CapabilityRisk::Low);
            assert!(!definition.mutation);
            assert!(!definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
    }

    #[test]
    fn suggestion_redaction_is_field_specific() {
        let mut suggestion = wire::Suggestion {
            suggestion_ref: Some(wire::SuggestionRef {
                suggestion_id: "suggestion-a".to_owned(),
            }),
            enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                enrichment_request_id: "request-a".to_owned(),
            }),
            proposed_value: "Private Company".to_owned(),
            proposed_value_digest: vec![7; 32],
            lifecycle_status: wire::SuggestionLifecycleStatus::Accepted as i32,
            evidence_references: vec!["evidence-a".to_owned()],
            ..Default::default()
        };
        redact_suggestion(&mut suggestion, |field| field == "lifecycle_status");
        assert!(suggestion.suggestion_ref.is_some());
        assert!(suggestion.enrichment_request_ref.is_none());
        assert!(suggestion.proposed_value.is_empty());
        assert!(suggestion.proposed_value_digest.is_empty());
        assert!(suggestion.evidence_references.is_empty());
        assert_eq!(
            suggestion.lifecycle_status,
            wire::SuggestionLifecycleStatus::Accepted as i32
        );
    }
}
