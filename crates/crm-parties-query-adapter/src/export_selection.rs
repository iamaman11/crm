use super::{
    LIST_CAPABILITY, LIST_REQUEST_SCHEMA, PartyQueryAdapter, party_filter_hash,
    party_from_snapshot, party_record_type,
};
use crm_capability_plan_support as support;
use crm_core_data::{RecordListQuery, RecordQueryContinuation, RecordQuerySort};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ErrorCategory, ModuleId,
    RecordId, RequestId, SchemaVersion, SdkError, TenantId, TraceId,
};
use crm_parties::PartyKind;
use crm_parties_capability_adapter::MODULE_ID;
use crm_proto_contracts::crm::parties::v1 as wire;
use crm_query_runtime::{QueryExecutionContext, QueryRequest, QueryVisibilityAuthorizer};

pub const MAXIMUM_PARTY_EXPORT_SELECTION_PAGE_SIZE: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyExportSelectionKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSelectionCandidate {
    pub party_id: RecordId,
    pub resource_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportSelectionPage {
    pub candidates: Vec<PartyExportSelectionCandidate>,
    pub next: Option<RecordQueryContinuation>,
}

/// Builds the exact Party list query context used by the private export-selection source.
///
/// The runtime authorizes this request before any authoritative Party read. The same request is then
/// passed to `list_for_export_selection`, so tenant, actor and capability identity remain identical
/// for live per-resource visibility decisions.
pub fn export_selection_query_request(
    tenant_id: &TenantId,
    actor_id: &ActorId,
    request_identity: &str,
    kind: Option<PartyExportSelectionKind>,
    request_started_at_unix_nanos: i64,
) -> Result<QueryRequest, SdkError> {
    let kind = match kind {
        None => None,
        Some(PartyExportSelectionKind::Person) => Some(wire::PartyKind::Person as i32),
        Some(PartyExportSelectionKind::Organization) => Some(wire::PartyKind::Organization as i32),
    };
    let command = wire::ListPartiesRequest {
        page: None,
        kind,
        sort: wire::PartySort::Unspecified as i32,
    };
    let input = support::protobuf_payload(
        MODULE_ID,
        LIST_REQUEST_SCHEMA,
        DataClass::Personal,
        &command,
    )?;
    let input_hash = party_filter_hash(&command);
    Ok(QueryRequest {
        owner_module_id: ModuleId::try_new(MODULE_ID).map_err(selection_config_error)?,
        context: QueryExecutionContext {
            tenant_id: tenant_id.clone(),
            actor_id: actor_id.clone(),
            request_id: RequestId::try_new(request_identity).map_err(selection_config_error)?,
            correlation_id: CorrelationId::try_new(request_identity)
                .map_err(selection_config_error)?,
            trace_id: TraceId::try_new(request_identity).map_err(selection_config_error)?,
            capability_id: CapabilityId::try_new(LIST_CAPABILITY)
                .map_err(selection_config_error)?,
            capability_version: CapabilityVersion::try_new(support::CONTRACT_VERSION)
                .map_err(selection_config_error)?,
            schema_version: SchemaVersion::try_new(support::CONTRACT_VERSION)
                .map_err(selection_config_error)?,
            request_started_at_unix_nanos,
        },
        input,
        input_hash,
    })
}

impl PartyQueryAdapter {
    /// Worker-private, governed Party selection primitive for bounded exports.
    ///
    /// This is intentionally not a public capability and does not expose arbitrary bulk discovery.
    /// The export worker supplies one immutable cutoff, a bounded page size and the exact opaque
    /// continuation previously persisted by the export-owned selection progress record. Reads use
    /// the existing tenant/RLS core-data port and re-run live Party visibility for every candidate.
    pub fn list_for_export_selection<'a>(
        &'a self,
        request: &'a QueryRequest,
        selection_cutoff_unix_nanos: i64,
        kind: Option<PartyExportSelectionKind>,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
    ) -> crm_module_sdk::PortFuture<'a, Result<PartyExportSelectionPage, SdkError>> {
        Box::pin(async move {
            request.context.validate()?;
            if selection_cutoff_unix_nanos <= 0 {
                return Err(SdkError::invalid_argument(
                    "party.export_selection.cutoff",
                    "Party export selection cutoff must be positive",
                ));
            }
            if page_size == 0 || page_size > MAXIMUM_PARTY_EXPORT_SELECTION_PAGE_SIZE {
                return Err(SdkError::invalid_argument(
                    "party.export_selection.page_size",
                    "Party export selection page size is invalid",
                ));
            }

            let mut candidates = Vec::with_capacity(page_size as usize);
            let mut scanned = 0_u32;
            loop {
                let remaining = page_size.saturating_sub(candidates.len() as u32);
                if remaining == 0 {
                    return Ok(PartyExportSelectionPage {
                        candidates,
                        next: after,
                    });
                }
                let page = self
                    .store
                    .list_records_for_query(&RecordListQuery {
                        tenant_id: request.context.tenant_id.clone(),
                        owner_module_id: ModuleId::try_new(MODULE_ID)
                            .map_err(selection_config_error)?,
                        record_type: party_record_type()?,
                        page_size: remaining,
                        sort: RecordQuerySort::CreatedAtAscending,
                        after: after.clone(),
                    })
                    .await?;
                scanned = scanned.saturating_add(page.records.len() as u32);
                if scanned > MAXIMUM_PARTY_EXPORT_SELECTION_PAGE_SIZE {
                    return Err(export_selection_unavailable());
                }

                for snapshot in &page.records {
                    let created_at_unix_nanos = snapshot
                        .reference
                        .created_at_unix_nanos
                        .ok_or_else(export_selection_unavailable)?;
                    if created_at_unix_nanos > selection_cutoff_unix_nanos {
                        return Ok(PartyExportSelectionPage {
                            candidates,
                            next: None,
                        });
                    }
                    let party = party_from_snapshot(snapshot)?;
                    if !selection_kind_matches(party.kind(), kind) {
                        continue;
                    }
                    let visibility = self
                        .visibility
                        .authorize_visibility(request, &snapshot.reference)
                        .await?;
                    if visibility.resource_visible {
                        candidates.push(PartyExportSelectionCandidate {
                            party_id: snapshot.reference.record_id.clone(),
                            resource_version: snapshot.version,
                        });
                    }
                }

                after = page.next;
                if after.is_none() {
                    return Ok(PartyExportSelectionPage {
                        candidates,
                        next: None,
                    });
                }
            }
        })
    }
}

fn selection_kind_matches(kind: PartyKind, requested: Option<PartyExportSelectionKind>) -> bool {
    match requested {
        None => true,
        Some(PartyExportSelectionKind::Person) => kind == PartyKind::Person,
        Some(PartyExportSelectionKind::Organization) => kind == PartyKind::Organization,
    }
}

fn selection_config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "PARTIES_EXPORT_SELECTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party export selection query is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn export_selection_unavailable() -> SdkError {
    SdkError::new(
        "PARTIES_EXPORT_SELECTION_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Party export selection is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_filter_mapping_is_exact() {
        assert!(selection_kind_matches(PartyKind::Person, None));
        assert!(selection_kind_matches(
            PartyKind::Person,
            Some(PartyExportSelectionKind::Person)
        ));
        assert!(!selection_kind_matches(
            PartyKind::Person,
            Some(PartyExportSelectionKind::Organization)
        ));
    }
}
