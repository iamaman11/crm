use crate::{ApplicationConfig, ApplicationRuntimeError};
use crm_capability_adapters::{
    AuthorizationGrant, LiveAuthorizationStore, LiveQueryVisibilityStore, QueryVisibilityGrant,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{ActorId, ModuleId, RecordType};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,
};
use crm_parties_query_adapter::GET_CAPABILITY as PARTY_GET_QUERY_CAPABILITY;
use std::collections::BTreeSet;

const POLICY_VERSION: &str = "application-bootstrap/v1";
const LIFETIME_NANOS: i64 = 365_i64 * 24 * 60 * 60 * 1_000_000_000;

pub(crate) fn bootstrap_export_execution_worker_access(
    config: &ApplicationConfig,
    now_unix_nanos: i64,
    authorization_store: &LiveAuthorizationStore,
    visibility_store: &LiveQueryVisibilityStore,
    query_definitions: &[CapabilityDefinition],
    internal_definitions: &[CapabilityDefinition],
    worker_actor_id: &ActorId,
) -> Result<(), ApplicationRuntimeError> {
    let expires_at = now_unix_nanos.checked_add(LIFETIME_NANOS).ok_or_else(|| {
        ApplicationRuntimeError::Assembly(
            "export execution worker grant expiry overflow".to_owned(),
        )
    })?;
    let party_get = query_definitions
        .iter()
        .find(|definition| {
            definition.owner_module_id.as_str() == PARTIES_MODULE_ID
                && definition.capability_id.as_str() == PARTY_GET_QUERY_CAPABILITY
        })
        .ok_or_else(|| {
            ApplicationRuntimeError::Assembly(
                "Party get capability is missing from the production query catalog".to_owned(),
            )
        })?;

    for tenant_id in &config.tenant_ids {
        for definition in std::iter::once(party_get).chain(internal_definitions.iter()) {
            authorization_store
                .upsert(AuthorizationGrant {
                    tenant_id: tenant_id.clone(),
                    actor_id: worker_actor_id.clone(),
                    policy_id: definition.authorization_policy_id.clone(),
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    owner_module_id: definition.owner_module_id.clone(),
                    policy_version: POLICY_VERSION.to_owned(),
                    expires_at_unix_nanos: Some(expires_at),
                })
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        }

        visibility_store
            .upsert(QueryVisibilityGrant {
                tenant_id: tenant_id.clone(),
                actor_id: worker_actor_id.clone(),
                capability_id: party_get.capability_id.clone(),
                capability_version: party_get.capability_version.clone(),
                owner_module_id: ModuleId::try_new(PARTIES_MODULE_ID)
                    .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
                record_type: RecordType::try_new(PARTY_RECORD_TYPE)
                    .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
                record_id: None,
                allowed_fields: ["kind", "display_name"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<BTreeSet<_>>(),
                policy_version: POLICY_VERSION.to_owned(),
                expires_at_unix_nanos: Some(expires_at),
            })
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
    }
    Ok(())
}
