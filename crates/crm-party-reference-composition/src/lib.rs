#![forbid(unsafe_code)]

//! Shared application-side Party reference reader.
//!
//! Owner modules consume stable Party references but never read Party storage.
//! Application composition uses this port before final live authorization to
//! conceal missing and cross-tenant references behind one bounded result.

use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_module_sdk::{ModuleId, PortFuture, RecordId, RecordType, SdkError, TenantId};
use crm_parties_capability_adapter::{MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE};

pub trait PartyReferenceReader: Send + Sync {
    fn exists<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_id: &'a str,
    ) -> PortFuture<'a, Result<bool, SdkError>>;
}

#[derive(Debug, Clone)]
pub struct PostgresPartyReferenceReader {
    store: PostgresDataStore,
}

impl PostgresPartyReferenceReader {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl PartyReferenceReader for PostgresPartyReferenceReader {
    fn exists<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_id: &'a str,
    ) -> PortFuture<'a, Result<bool, SdkError>> {
        Box::pin(async move {
            let owner_module_id =
                ModuleId::try_new(PARTIES_MODULE_ID).map_err(configuration_error)?;
            let record_type = RecordType::try_new(RECORD_TYPE).map_err(configuration_error)?;
            let record_id = RecordId::try_new(party_id).map_err(configuration_error)?;
            Ok(self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id,
                    record_type,
                    record_id,
                })
                .await?
                .is_some())
        })
    }
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "PARTY_REFERENCE_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Party reference validation boundary is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

pub const CRATE_NAME: &str = "crm-party-reference-composition";
