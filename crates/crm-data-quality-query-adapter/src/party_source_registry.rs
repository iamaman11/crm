use crm_module_sdk::{ErrorCategory, SdkError};
use crm_parties_query_adapter::PartyQueryAdapter;
use std::sync::{Arc, OnceLock, RwLock};

static PARTY_QUERY_ADAPTER: OnceLock<RwLock<Option<Arc<PartyQueryAdapter>>>> = OnceLock::new();

pub fn register_party_quality_query_adapter(
    adapter: Arc<PartyQueryAdapter>,
) -> Result<(), SdkError> {
    let registry = PARTY_QUERY_ADAPTER.get_or_init(|| RwLock::new(None));
    let mut current = registry.write().map_err(|_| registry_error())?;
    *current = Some(adapter);
    Ok(())
}

pub fn registered_party_quality_query_adapter() -> Result<Arc<PartyQueryAdapter>, SdkError> {
    PARTY_QUERY_ADAPTER
        .get()
        .ok_or_else(registry_unavailable)?
        .read()
        .map_err(|_| registry_error())?
        .clone()
        .ok_or_else(registry_unavailable)
}

fn registry_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PARTY_SOURCE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The governed Party source for Data Quality is temporarily unavailable.",
    )
}

fn registry_error() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PARTY_SOURCE_REGISTRY_INVALID",
        ErrorCategory::Internal,
        false,
        "The governed Party source registry is invalid.",
    )
}
