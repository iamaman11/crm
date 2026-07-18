mod cursor;
mod scan;

use crate::{
    CustomerEnrichmentSuggestionQueryAdapter, LIST_SUGGESTIONS_BY_PARTY_REQUEST_SCHEMA,
    LIST_SUGGESTIONS_BY_PARTY_RESPONSE_SCHEMA, decode_input, party_record_type,
};
use crm_capability_plan_support as support;
use crm_module_sdk::{DataClass, RecordId, RecordRef, SdkError, TypedPayload};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use crm_query_runtime::QueryRequest;

pub(crate) fn validate(
    adapter: &CustomerEnrichmentSuggestionQueryAdapter,
    request: &QueryRequest,
) -> Result<(), SdkError> {
    let command: wire::ListSuggestionsByPartyRequest =
        decode_input(request, LIST_SUGGESTIONS_BY_PARTY_REQUEST_SCHEMA)?;
    let party_id = cursor::party_id(command.party_ref)?;
    let profile_id = cursor::profile_id(command.provider_profile_version_ref)?;
    let status = command.status.map(cursor::status).transpose()?;
    let page_size = cursor::page_size(command.page_size)?;
    let binding = cursor::binding(request, &party_id, &profile_id, status, page_size)?;
    cursor::decode_after(adapter, &command.cursor, &binding).map(|_| ())
}

pub(crate) async fn execute(
    adapter: &CustomerEnrichmentSuggestionQueryAdapter,
    request: &QueryRequest,
) -> Result<TypedPayload, SdkError> {
    let command: wire::ListSuggestionsByPartyRequest =
        decode_input(request, LIST_SUGGESTIONS_BY_PARTY_REQUEST_SCHEMA)?;
    let party_id = cursor::party_id(command.party_ref)?;
    let profile_id = cursor::profile_id(command.provider_profile_version_ref)?;
    let status = command.status.map(cursor::status).transpose()?;
    let page_size = cursor::page_size(command.page_size)?;
    let binding = cursor::binding(request, &party_id, &profile_id, status, page_size)?;
    let after = cursor::decode_after(adapter, &command.cursor, &binding)?;

    let party = RecordRef {
        record_type: party_record_type()?,
        record_id: party_id.clone(),
    };
    if !adapter
        .visibility
        .authorize_visibility(request, &party)
        .await?
        .resource_visible
    {
        return response(Vec::new(), String::new());
    }

    let reviews = adapter.load_visible_latest_reviews(request).await?;
    let (items, next) = scan::collect(
        adapter,
        request,
        &party_id,
        &profile_id,
        status,
        page_size,
        after,
        &reviews,
    )
    .await?;
    response(
        items,
        cursor::encode_next(adapter, &binding, next.as_ref())?,
    )
}

pub(super) fn matches(
    suggestion: &wire::Suggestion,
    party_id: &RecordId,
    profile_id: &RecordId,
    status: Option<i32>,
) -> bool {
    suggestion
        .target
        .as_ref()
        .and_then(|target| target.party_ref.as_ref())
        .is_some_and(|party| party.party_id == party_id.as_str())
        && suggestion
            .provider_profile_version_ref
            .as_ref()
            .is_some_and(|profile| profile.provider_profile_version_id == profile_id.as_str())
        && status.is_none_or(|value| suggestion.lifecycle_status == value)
}

fn response(items: Vec<wire::Suggestion>, cursor: String) -> Result<TypedPayload, SdkError> {
    support::protobuf_payload(
        crate::MODULE_ID,
        LIST_SUGGESTIONS_BY_PARTY_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::ListSuggestionsByPartyResponse {
            suggestions: items,
            next_cursor: cursor,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_proto_contracts::crm::customer::v1::PartyRef;

    #[test]
    fn exact_filters_are_required() {
        let value = wire::Suggestion {
            target: Some(wire::EnrichmentTargetSnapshot {
                party_ref: Some(PartyRef {
                    party_id: "party-a".to_owned(),
                }),
                ..Default::default()
            }),
            provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
                provider_profile_version_id: "profile-a".to_owned(),
            }),
            lifecycle_status: wire::SuggestionLifecycleStatus::Accepted as i32,
            ..Default::default()
        };
        assert!(matches(
            &value,
            &RecordId::try_new("party-a").unwrap(),
            &RecordId::try_new("profile-a").unwrap(),
            Some(wire::SuggestionLifecycleStatus::Accepted as i32),
        ));
        assert!(!matches(
            &value,
            &RecordId::try_new("party-b").unwrap(),
            &RecordId::try_new("profile-a").unwrap(),
            None,
        ));
    }
}
