use super::matches;
use crate::{
    CustomerEnrichmentSuggestionQueryAdapter, VisibleReview, enforce_scan_limit, module_id,
    query_configuration_invalid, request_started_at_unix_ms, suggestion_record_type,
};
use crm_core_data::{RecordListQuery, RecordQueryContinuation, RecordQuerySort};
use crm_customer_enrichment_review_adapter::{suggestion_from_snapshot, suggestion_to_wire};
use crm_module_sdk::{RecordId, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use crm_query_runtime::QueryRequest;
use std::collections::BTreeMap;

pub(super) async fn collect(
    adapter: &CustomerEnrichmentSuggestionQueryAdapter,
    request: &QueryRequest,
    party_id: &RecordId,
    profile_id: &RecordId,
    status: Option<i32>,
    page_size: u32,
    mut after: Option<RecordQueryContinuation>,
    reviews: &BTreeMap<String, VisibleReview>,
) -> Result<(Vec<wire::Suggestion>, Option<RecordQueryContinuation>), SdkError> {
    let mut output = Vec::with_capacity(page_size as usize);
    let mut scanned = 0_usize;
    loop {
        let remaining = page_size as usize - output.len();
        if remaining == 0 {
            let anchor = after.clone();
            let more = has_more(
                adapter,
                request,
                party_id,
                profile_id,
                status,
                anchor.clone(),
                reviews,
                &mut scanned,
            )
            .await?;
            return Ok((output, more.then_some(anchor).flatten()));
        }

        let page = adapter
            .store
            .list_records_for_query(&RecordListQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: suggestion_record_type()?,
                page_size: u32::try_from(remaining).map_err(query_configuration_invalid)?,
                sort: RecordQuerySort::UpdatedAtDescending,
                after: after.clone(),
            })
            .await?;
        scanned = scanned.saturating_add(page.records.len());
        enforce_scan_limit(scanned)?;

        for snapshot in &page.records {
            let suggestion = suggestion_from_snapshot(snapshot)?;
            let review = reviews.get(suggestion.suggestion_id().as_str());
            let mut public = suggestion_to_wire(
                &suggestion,
                review.map(|value| &value.decision),
                request_started_at_unix_ms(request)?,
            )?;
            if !matches(&public, party_id, profile_id, status) {
                continue;
            }
            let visibility = adapter
                .visibility
                .authorize_visibility(request, &snapshot.reference)
                .await?;
            if !visibility.resource_visible {
                continue;
            }
            crate::redact_suggestion(&mut public, |field| visibility.allows_field(field));
            output.push(public);
        }

        after = page.next;
        if after.is_none() {
            return Ok((output, None));
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn has_more(
    adapter: &CustomerEnrichmentSuggestionQueryAdapter,
    request: &QueryRequest,
    party_id: &RecordId,
    profile_id: &RecordId,
    status: Option<i32>,
    mut after: Option<RecordQueryContinuation>,
    reviews: &BTreeMap<String, VisibleReview>,
    scanned: &mut usize,
) -> Result<bool, SdkError> {
    while after.is_some() {
        let page = adapter
            .store
            .list_records_for_query(&RecordListQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: suggestion_record_type()?,
                page_size: super::cursor::MAXIMUM_PAGE_SIZE,
                sort: RecordQuerySort::UpdatedAtDescending,
                after: after.clone(),
            })
            .await?;
        *scanned = scanned.saturating_add(page.records.len());
        enforce_scan_limit(*scanned)?;

        for snapshot in &page.records {
            let suggestion = suggestion_from_snapshot(snapshot)?;
            let review = reviews.get(suggestion.suggestion_id().as_str());
            let public = suggestion_to_wire(
                &suggestion,
                review.map(|value| &value.decision),
                request_started_at_unix_ms(request)?,
            )?;
            if matches(&public, party_id, profile_id, status)
                && adapter
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?
                    .resource_visible
            {
                return Ok(true);
            }
        }
        after = page.next;
    }
    Ok(false)
}
