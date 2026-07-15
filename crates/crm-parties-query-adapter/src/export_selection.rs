use super::{PartyQueryAdapter, party_from_snapshot, party_record_type};
use crm_core_data::{RecordListQuery, RecordQueryContinuation, RecordQuerySort};
use crm_module_sdk::{ErrorCategory, ModuleId, RecordId, SdkError};
use crm_parties::PartyKind;
use crm_parties_capability_adapter::MODULE_ID;
use crm_query_runtime::{QueryRequest, QueryVisibilityDecision};
use std::collections::BTreeSet;

pub const MAXIMUM_PARTY_EXPORT_SELECTION_PAGE_SIZE: u32 = 1_000;
const MAXIMUM_PARTY_EXPORT_SELECTION_SCAN_RECORDS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyExportSelectionKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedPartyExportSelectionCandidate {
    pub party_id: RecordId,
    pub resource_version: i64,
    pub created_at_unix_nanos: i64,
    pub allowed_fields: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedPartyExportSelectionPage {
    pub candidates: Vec<GovernedPartyExportSelectionCandidate>,
    pub next: Option<RecordQueryContinuation>,
}

impl PartyQueryAdapter {
    /// Worker-private governed Party selection used by bounded customer export.
    ///
    /// This is intentionally not registered as a public query capability. It stays inside the
    /// Party-owned query adapter, uses the same tenant/RLS storage port and repeats the same live
    /// resource/field visibility authorization as public Party reads. The returned projection is
    /// minimal: exact Party identity/version plus visibility evidence required to build export-owned
    /// selection evidence. Mutable Party values are not copied into customer-data operations state.
    pub async fn list_for_export_selection(
        &self,
        request: &QueryRequest,
        selection_cutoff_unix_nanos: i64,
        kind: Option<PartyExportSelectionKind>,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
    ) -> Result<GovernedPartyExportSelectionPage, SdkError> {
        validate_export_selection_request(selection_cutoff_unix_nanos, page_size, after.as_ref())?;

        let mut candidates = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - candidates.len();
            if remaining == 0 {
                return Ok(GovernedPartyExportSelectionPage {
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
                    page_size: u32::try_from(remaining)
                        .map_err(|_| export_selection_unavailable())?,
                    sort: RecordQuerySort::CreatedAtAscending,
                    after: after.clone(),
                })
                .await?;

            scanned = scanned.saturating_add(page.records.len());
            if scanned > MAXIMUM_PARTY_EXPORT_SELECTION_SCAN_RECORDS {
                return Err(export_selection_unavailable());
            }

            for snapshot in &page.records {
                let party = party_from_snapshot(snapshot)?;
                if party.created_at_unix_nanos() > selection_cutoff_unix_nanos
                    || !selection_kind_matches(party.kind(), kind)
                {
                    continue;
                }

                let visibility: QueryVisibilityDecision = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if !visibility.resource_visible {
                    continue;
                }

                candidates.push(GovernedPartyExportSelectionCandidate {
                    party_id: snapshot.reference.record_id.clone(),
                    resource_version: snapshot.version,
                    created_at_unix_nanos: party.created_at_unix_nanos(),
                    allowed_fields: visibility.allowed_fields,
                });
            }

            after = page.next;
            if after.is_none() {
                return Ok(GovernedPartyExportSelectionPage {
                    candidates,
                    next: None,
                });
            }
        }
    }
}

fn validate_export_selection_request(
    selection_cutoff_unix_nanos: i64,
    page_size: u32,
    after: Option<&RecordQueryContinuation>,
) -> Result<(), SdkError> {
    if selection_cutoff_unix_nanos <= 0 {
        return Err(SdkError::invalid_argument(
            "customer_data.export.selection_cutoff_unix_nanos",
            "selection cutoff must be positive Unix nanoseconds",
        ));
    }
    if page_size == 0 || page_size > MAXIMUM_PARTY_EXPORT_SELECTION_PAGE_SIZE {
        return Err(SdkError::invalid_argument(
            "customer_data.export.selection.page_size",
            format!(
                "selection page size must be between 1 and {MAXIMUM_PARTY_EXPORT_SELECTION_PAGE_SIZE}"
            ),
        ));
    }
    if let Some(after) = after {
        after.validate()?;
    }
    Ok(())
}

fn selection_kind_matches(kind: PartyKind, filter: Option<PartyExportSelectionKind>) -> bool {
    match filter {
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
        "PARTIES_EXPORT_SELECTION_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The governed Party export selection is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_cutoff_page_size_and_continuation() {
        assert!(validate_export_selection_request(1, 1, None).is_ok());
        assert!(validate_export_selection_request(0, 1, None).is_err());
        assert!(validate_export_selection_request(1, 0, None).is_err());
        assert!(
            validate_export_selection_request(
                1,
                MAXIMUM_PARTY_EXPORT_SELECTION_PAGE_SIZE + 1,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn kind_filter_is_exact() {
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
