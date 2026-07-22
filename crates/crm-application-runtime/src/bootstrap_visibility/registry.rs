use super::fields::{
    account_fields, consent_fields, contact_point_fields, customer_360_account_fields,
    customer_360_contact_point_fields, customer_360_party_fields,
    customer_360_party_relationship_fields, customer_data_import_job_fields,
    customer_data_import_row_fields, fields, identity_resolution_fields,
    identity_resolution_merge_fields, party_fields, party_relationship_fields, sales_fields,
    task_fields,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_consents_capability_adapter::{
    MODULE_ID as CONSENTS_MODULE_ID, RECORD_TYPE as CONSENT_RECORD_TYPE,
};
use crm_contact_points_capability_adapter::{
    MODULE_ID as CONTACT_POINTS_MODULE_ID, RECORD_TYPE as CONTACT_POINT_RECORD_TYPE,
};
use crm_customer_360_query_adapter::MODULE_ID as CUSTOMER_360_MODULE_ID;
use crm_customer_accounts_capability_adapter::{
    MODULE_ID as ACCOUNTS_MODULE_ID, RECORD_TYPE as ACCOUNT_RECORD_TYPE,
};
use crm_customer_data_operations_capability_adapter::{
    IMPORT_JOB_RECORD_TYPE as CUSTOMER_DATA_IMPORT_JOB_RECORD_TYPE,
    IMPORT_ROW_RECORD_TYPE as CUSTOMER_DATA_IMPORT_ROW_RECORD_TYPE,
    MODULE_ID as CUSTOMER_DATA_OPERATIONS_MODULE_ID,
};
use crm_customer_data_operations_query_adapter::LIST_IMPORT_ROWS_CAPABILITY;
use crm_customer_enrichment::MODULE_ID as CUSTOMER_ENRICHMENT_MODULE_ID;
use crm_customer_enrichment_visibility::query_visibility_resources as customer_enrichment_query_visibility_resources;
use crm_customer_privacy_query_adapter::{
    GET_PRIVACY_CASE_CAPABILITY, LIST_PRIVACY_CASES_CAPABILITY,
};
use crm_customer_privacy_query_adapter::query_visibility_resources as customer_privacy_query_visibility_resources;
use crm_identity_resolution_capability_adapter::{
    MERGE_OPERATION_RECORD_TYPE as IDENTITY_RESOLUTION_MERGE_RECORD_TYPE,
    MODULE_ID as IDENTITY_RESOLUTION_MODULE_ID, RECORD_TYPE as IDENTITY_RESOLUTION_RECORD_TYPE,
};
use crm_metadata_api_adapter::METADATA_MODULE_ID;
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,
};
use crm_party_relationships_capability_adapter::{
    MODULE_ID as PARTY_RELATIONSHIPS_MODULE_ID, RECORD_TYPE as PARTY_RELATIONSHIP_RECORD_TYPE,
};
use crm_sales_activities_query_adapter::{ACTIVITIES_RECORD_TYPE, SALES_RECORD_TYPE};
use crm_search_query_adapter::SEARCH_MODULE_ID;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const SALES_MODULE_ID: &str = "crm.sales";
pub(super) const ACTIVITIES_MODULE_ID: &str = "crm.activities";
pub(super) const DATA_QUALITY_MODULE_ID: &str = "crm.data-quality";
pub(super) const DATA_QUALITY_RULE_SET_RECORD_TYPE: &str = "data_quality.party_rule_set_version";
pub(super) const CUSTOMER_PRIVACY_MODULE_ID: &str = "crm.customer-privacy";

pub(super) type VisibilityProvider = fn(&CapabilityDefinition) -> Vec<BootstrapVisibilityResource>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BootstrapVisibilityResource {
    pub owner_module_id: &'static str,
    pub resource_type: &'static str,
    pub allowed_fields: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BootstrapVisibilityRegistry {
    providers: BTreeMap<&'static str, VisibilityProvider>,
}

impl BootstrapVisibilityRegistry {
    pub fn resources_for(
        &self,
        definition: &CapabilityDefinition,
    ) -> Result<Vec<BootstrapVisibilityResource>, SdkError> {
        self.providers
            .get(definition.owner_module_id.as_str())
            .copied()
            .map(|provider| provider(definition))
            .ok_or_else(|| unsupported_owner(definition.owner_module_id.as_str()))
    }
}

pub(crate) fn build_bootstrap_visibility_registry() -> Result<BootstrapVisibilityRegistry, SdkError>
{
    let mut providers = BTreeMap::new();
    register(&mut providers, SALES_MODULE_ID, sales_visibility)?;
    register(&mut providers, ACTIVITIES_MODULE_ID, activities_visibility)?;
    register(&mut providers, PARTIES_MODULE_ID, parties_visibility)?;
    register(&mut providers, ACCOUNTS_MODULE_ID, accounts_visibility)?;
    register(
        &mut providers,
        CONTACT_POINTS_MODULE_ID,
        contact_points_visibility,
    )?;
    register(&mut providers, CONSENTS_MODULE_ID, consents_visibility)?;
    register(
        &mut providers,
        IDENTITY_RESOLUTION_MODULE_ID,
        identity_resolution_visibility,
    )?;
    register(
        &mut providers,
        PARTY_RELATIONSHIPS_MODULE_ID,
        party_relationships_visibility,
    )?;
    register(
        &mut providers,
        CUSTOMER_DATA_OPERATIONS_MODULE_ID,
        customer_data_operations_visibility,
    )?;
    register(
        &mut providers,
        CUSTOMER_ENRICHMENT_MODULE_ID,
        customer_enrichment_visibility,
    )?;
    register(
        &mut providers,
        CUSTOMER_PRIVACY_MODULE_ID,
        customer_privacy_visibility,
    )?;
    register(
        &mut providers,
        DATA_QUALITY_MODULE_ID,
        data_quality_visibility,
    )?;
    register(&mut providers, METADATA_MODULE_ID, no_visibility)?;
    register(
        &mut providers,
        CUSTOMER_360_MODULE_ID,
        customer_360_visibility,
    )?;
    register(&mut providers, SEARCH_MODULE_ID, search_visibility)?;
    Ok(BootstrapVisibilityRegistry { providers })
}

fn register(
    providers: &mut BTreeMap<&'static str, VisibilityProvider>,
    module_id: &'static str,
    provider: VisibilityProvider,
) -> Result<(), SdkError> {
    if providers.insert(module_id, provider).is_some() {
        return Err(SdkError::new(
            "APPLICATION_BOOTSTRAP_VISIBILITY_DUPLICATE",
            ErrorCategory::Internal,
            false,
            "A bootstrap visibility contribution was registered more than once.",
        )
        .with_internal_reference(module_id));
    }
    Ok(())
}

fn resource(
    owner_module_id: &'static str,
    resource_type: &'static str,
    allowed_fields: BTreeSet<String>,
) -> BootstrapVisibilityResource {
    BootstrapVisibilityResource {
        owner_module_id,
        resource_type,
        allowed_fields,
    }
}

fn sales_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![resource(SALES_MODULE_ID, SALES_RECORD_TYPE, sales_fields())]
}

fn activities_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![resource(
        ACTIVITIES_MODULE_ID,
        ACTIVITIES_RECORD_TYPE,
        task_fields(),
    )]
}

fn parties_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![resource(
        PARTIES_MODULE_ID,
        PARTY_RECORD_TYPE,
        party_fields(),
    )]
}

fn accounts_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![resource(
        ACCOUNTS_MODULE_ID,
        ACCOUNT_RECORD_TYPE,
        account_fields(),
    )]
}

fn contact_points_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![resource(
        CONTACT_POINTS_MODULE_ID,
        CONTACT_POINT_RECORD_TYPE,
        contact_point_fields(),
    )]
}

fn consents_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![resource(
        CONSENTS_MODULE_ID,
        CONSENT_RECORD_TYPE,
        consent_fields(),
    )]
}

fn identity_resolution_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![
        resource(
            IDENTITY_RESOLUTION_MODULE_ID,
            IDENTITY_RESOLUTION_RECORD_TYPE,
            identity_resolution_fields(),
        ),
        resource(
            IDENTITY_RESOLUTION_MODULE_ID,
            IDENTITY_RESOLUTION_MERGE_RECORD_TYPE,
            identity_resolution_merge_fields(),
        ),
    ]
}

fn party_relationships_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![resource(
        PARTY_RELATIONSHIPS_MODULE_ID,
        PARTY_RELATIONSHIP_RECORD_TYPE,
        party_relationship_fields(),
    )]
}

fn customer_data_operations_visibility(
    definition: &CapabilityDefinition,
) -> Vec<BootstrapVisibilityResource> {
    let mut resources = vec![resource(
        CUSTOMER_DATA_OPERATIONS_MODULE_ID,
        CUSTOMER_DATA_IMPORT_JOB_RECORD_TYPE,
        customer_data_import_job_fields(),
    )];
    if definition.capability_id.as_str() == LIST_IMPORT_ROWS_CAPABILITY {
        resources.push(resource(
            CUSTOMER_DATA_OPERATIONS_MODULE_ID,
            CUSTOMER_DATA_IMPORT_ROW_RECORD_TYPE,
            customer_data_import_row_fields(),
        ));
    }
    resources
}

fn customer_enrichment_visibility(
    definition: &CapabilityDefinition,
) -> Vec<BootstrapVisibilityResource> {
    customer_enrichment_query_visibility_resources(definition.capability_id.as_str())
        .into_iter()
        .map(|resource| BootstrapVisibilityResource {
            owner_module_id: resource.owner_module_id,
            resource_type: resource.resource_type,
            allowed_fields: resource.allowed_fields,
        })
        .collect()
}

fn customer_privacy_visibility(
    definition: &CapabilityDefinition,
) -> Vec<BootstrapVisibilityResource> {
    debug_assert!(matches!(
        definition.capability_id.as_str(),
        GET_PRIVACY_CASE_CAPABILITY | LIST_PRIVACY_CASES_CAPABILITY
    ));
    customer_privacy_query_visibility_resources(definition.capability_id.as_str())
        .into_iter()
        .map(|resource| BootstrapVisibilityResource {
            owner_module_id: resource.owner_module_id,
            resource_type: resource.resource_type,
            allowed_fields: resource.allowed_fields,
        })
        .collect()
}

fn data_quality_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![resource(
        DATA_QUALITY_MODULE_ID,
        DATA_QUALITY_RULE_SET_RECORD_TYPE,
        fields(["definition"]),
    )]
}

fn no_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    Vec::new()
}

fn customer_360_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![
        resource(
            PARTIES_MODULE_ID,
            PARTY_RECORD_TYPE,
            customer_360_party_fields(),
        ),
        resource(
            ACCOUNTS_MODULE_ID,
            ACCOUNT_RECORD_TYPE,
            customer_360_account_fields(),
        ),
        resource(
            CONTACT_POINTS_MODULE_ID,
            CONTACT_POINT_RECORD_TYPE,
            customer_360_contact_point_fields(),
        ),
        resource(
            PARTY_RELATIONSHIPS_MODULE_ID,
            PARTY_RELATIONSHIP_RECORD_TYPE,
            customer_360_party_relationship_fields(),
        ),
    ]
}

fn search_visibility(_: &CapabilityDefinition) -> Vec<BootstrapVisibilityResource> {
    vec![
        resource(SALES_MODULE_ID, SALES_RECORD_TYPE, fields(["name"])),
        resource(
            ACTIVITIES_MODULE_ID,
            ACTIVITIES_RECORD_TYPE,
            fields(["subject"]),
        ),
        resource(PARTIES_MODULE_ID, PARTY_RECORD_TYPE, party_fields()),
    ]
}

fn unsupported_owner(module_id: &str) -> SdkError {
    SdkError::new(
        "APPLICATION_BOOTSTRAP_VISIBILITY_OWNER_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "A production query module has no bootstrap visibility contribution.",
    )
    .with_internal_reference(module_id)
}
