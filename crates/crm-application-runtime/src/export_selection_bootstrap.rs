use crate::{ApplicationConfig, ApplicationRuntimeError};
use crm_capability_adapters::{
    AuthorizationGrant, LiveAuthorizationStore, LiveQueryVisibilityStore, QueryVisibilityGrant,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_data_operations_capability_adapter::internal_export_execution_capability_definitions;
use crm_customer_data_operations_execution_composition::EXPORT_EXECUTION_WORKER_ACTOR_ID;
use crm_module_sdk::{ActorId, ModuleId, RecordType};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,
};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_QUERY_CAPABILITY, LIST_CAPABILITY as PARTY_LIST_QUERY_CAPABILITY,
};
use std::collections::BTreeSet;

const POLICY_VERSION: &str = "application-bootstrap/v1";
const LIFETIME_NANOS: i64 = 365_i64 * 24 * 60 * 60 * 1_000_000_000;

pub(crate) fn bootstrap_export_selection_worker_access(
    config: &ApplicationConfig,
    now_unix_nanos: i64,
    authorization_store: &LiveAuthorizationStore,
    visibility_store: &LiveQueryVisibilityStore,
    query_definitions: &[CapabilityDefinition],
    internal_definitions: &[CapabilityDefinition],
    worker_actor_id: &ActorId,
) -> Result<(), ApplicationRuntimeError> {
    let expires_at = now_unix_nanos.checked_add(LIFETIME_NANOS).ok_or_else(|| {
        ApplicationRuntimeError::Assembly("export worker grant expiry overflow".to_owned())
    })?;
    let party_list = find_party_query(query_definitions, PARTY_LIST_QUERY_CAPABILITY, "list")?;
    let party_get = find_party_query(query_definitions, PARTY_GET_QUERY_CAPABILITY, "get")?;
    let execution_definitions = internal_export_execution_capability_definitions()
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
    let execution_actor_id = ActorId::try_new(EXPORT_EXECUTION_WORKER_ACTOR_ID)
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;

    for tenant_id in &config.tenant_ids {
        grant_capabilities(
            authorization_store,
            tenant_id,
            worker_actor_id,
            std::iter::once(party_list).chain(internal_definitions.iter()),
            expires_at,
        )?;
        grant_visibility(
            visibility_store,
            tenant_id,
            worker_actor_id,
            party_list,
            expires_at,
        )?;

        grant_capabilities(
            authorization_store,
            tenant_id,
            &execution_actor_id,
            std::iter::once(party_get).chain(execution_definitions.iter()),
            expires_at,
        )?;
        grant_visibility(
            visibility_store,
            tenant_id,
            &execution_actor_id,
            party_get,
            expires_at,
        )?;
    }
    Ok(())
}

fn find_party_query<'a>(
    query_definitions: &'a [CapabilityDefinition],
    capability_id: &str,
    name: &str,
) -> Result<&'a CapabilityDefinition, ApplicationRuntimeError> {
    query_definitions
        .iter()
        .find(|definition| {
            definition.owner_module_id.as_str() == PARTIES_MODULE_ID
                && definition.capability_id.as_str() == capability_id
        })
        .ok_or_else(|| {
            ApplicationRuntimeError::Assembly(format!(
                "Party {name} capability is missing from the production query catalog"
            ))
        })
}

fn grant_capabilities<'a>(
    authorization_store: &LiveAuthorizationStore,
    tenant_id: &crm_module_sdk::TenantId,
    actor_id: &ActorId,
    definitions: impl Iterator<Item = &'a CapabilityDefinition>,
    expires_at_unix_nanos: i64,
) -> Result<(), ApplicationRuntimeError> {
    for definition in definitions {
        authorization_store
            .upsert(AuthorizationGrant {
                tenant_id: tenant_id.clone(),
                actor_id: actor_id.clone(),
                policy_id: definition.authorization_policy_id.clone(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                owner_module_id: definition.owner_module_id.clone(),
                policy_version: POLICY_VERSION.to_owned(),
                expires_at_unix_nanos: Some(expires_at_unix_nanos),
            })
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
    }
    Ok(())
}

fn grant_visibility(
    visibility_store: &LiveQueryVisibilityStore,
    tenant_id: &crm_module_sdk::TenantId,
    actor_id: &ActorId,
    definition: &CapabilityDefinition,
    expires_at_unix_nanos: i64,
) -> Result<(), ApplicationRuntimeError> {
    visibility_store
        .upsert(QueryVisibilityGrant {
            tenant_id: tenant_id.clone(),
            actor_id: actor_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
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
            expires_at_unix_nanos: Some(expires_at_unix_nanos),
        })
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
    Ok(())
}
