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
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID as CUSTOMER_ENRICHMENT_MODULE_ID,
    PROVIDER_PROFILE_VERSION_RECORD_TYPE as CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_RECORD_TYPE,
};
use crm_customer_enrichment_query_adapter::{
    GET_ENRICHMENT_REQUEST_CAPABILITY, GET_MAPPING_CAPABILITY, GET_PROVIDER_PROFILE_CAPABILITY,
};
use crm_customer_enrichment_request_list_query_adapter::LIST_ENRICHMENT_REQUESTS_CAPABILITY;
use crm_customer_enrichment_suggestion_query_adapter::GET_SUGGESTION_CAPABILITY;
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

const SALES_MODULE_ID: &str = "crm.sales";
const ACTIVITIES_MODULE_ID: &str = "crm.activities";
const DATA_QUALITY_MODULE_ID: &str = "crm.data-quality";
const DATA_QUALITY_RULE_SET_RECORD_TYPE: &str = "data_quality.party_rule_set_version";
const CUSTOMER_ENRICHMENT_REQUEST_RECORD_TYPE: &str = "customer_enrichment.request";
const CUSTOMER_ENRICHMENT_SUGGESTION_RECORD_TYPE: &str = "customer_enrichment.suggestion";
const CUSTOMER_ENRICHMENT_REVIEW_DECISION_RECORD_TYPE: &str =
    "customer_enrichment.review_decision";
const CUSTOMER_ENRICHMENT_SUGGESTION_RECORD_TYPE: &str = "customer_enrichment.suggestion";
const CUSTOMER_ENRICHMENT_REVIEW_DECISION_RECORD_TYPE: &str = "customer_enrichment.review_decision";

type VisibilityProvider = fn(&CapabilityDefinition) -> Vec<BootstrapVisibilityResource>;

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
    match definition.capability_id.as_str() {
        GET_PROVIDER_PROFILE_CAPABILITY | GET_MAPPING_CAPABILITY => vec![resource(
            CUSTOMER_ENRICHMENT_MODULE_ID,
            CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_RECORD_TYPE,
            fields(["definition"]),
        )],
        GET_ENRICHMENT_REQUEST_CAPABILITY | LIST_ENRICHMENT_REQUESTS_CAPABILITY => vec![
            // Live visibility keys are scoped by the query owner. These routes use Party visibility
            // only as a resource-existence gate and disclose no Party fields.
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                PARTY_RECORD_TYPE,
                BTreeSet::new(),
            ),
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                CUSTOMER_ENRICHMENT_REQUEST_RECORD_TYPE,
                customer_enrichment_request_fields(),
            ),
        GET_SUGGESTION_CAPABILITY => vec![
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                PARTY_RECORD_TYPE,
                BTreeSet::new(),
            ),
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                CUSTOMER_ENRICHMENT_SUGGESTION_RECORD_TYPE,
                customer_enrichment_suggestion_fields(),
            ),
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                CUSTOMER_ENRICHMENT_REVIEW_DECISION_RECORD_TYPE,
                customer_enrichment_review_decision_fields(),
            ),
        ],
        _ => Vec::new(),
    }
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

fn fields<const N: usize>(values: [&str; N]) -> BTreeSet<String> {
    values.into_iter().map(str::to_owned).collect()
}

fn sales_fields() -> BTreeSet<String> {
    fields([
        "name",
        "stage",
        "amount",
        "owner",
        "account",
        "primary_contact",
        "expected_close_date",
        "probability_basis_points",
        "status",
        "close_outcome",
        "created_at",
        "updated_at",
    ])
}

fn customer_data_import_job_fields() -> BTreeSet<String> {
    fields(["source", "mapping", "status", "counters", "checkpoint"])
}

fn customer_data_import_row_fields() -> BTreeSet<String> {
    fields([
        "row_position",
        "source_identity",
        "status",
        "prepared_party",
        "diagnostics",
        "execution",
        "target_party_ref",
    ])
}

fn customer_enrichment_request_fields() -> BTreeSet<String> {
    fields([
        "requested_by_actor_id",
        "target",
        "provider_profile_version_ref",
        "mapping_version_ref",
        "requested_fields",
        "policy_evidence",
        "created_at_unix_ms",
        "deadline_at_unix_ms",
        "expires_at_unix_ms",
        "status",
        "retry_generation",
        "provider_response_receipt_ref",
        "last_safe_failure_code",
        "updated_at_unix_ms",
    ])
}

fn customer_360_party_fields() -> BTreeSet<String> {
    fields(["display_name"])
}

fn customer_360_account_fields() -> BTreeSet<String> {
    fields(["name", "status"])
}

fn customer_360_contact_point_fields() -> BTreeSet<String> {
    fields([
        "party_ref",
        "kind",
        "normalized_value",
        "status",
        "preferred",
        "validity",
        "verification",
    ])
}

fn customer_360_party_relationship_fields() -> BTreeSet<String> {
    fields(["from_party_ref", "to_party_ref", "status", "validity"])
}

fn party_fields() -> BTreeSet<String> {
    fields(["kind", "display_name"])
}

fn account_fields() -> BTreeSet<String> {
    fields(["name", "status", "party_associations"])
}

fn contact_point_fields() -> BTreeSet<String> {
    fields([
        "party_ref",
        "kind",
        "normalized_value",
        "display_value",
        "status",
        "preferred",
        "validity",
        "verification",
    ])
}

fn consent_fields() -> BTreeSet<String> {
    fields([
        "party_ref",
        "contact_point_ref",
        "purpose",
        "channel",
        "effect",
        "legal_basis",
        "jurisdiction",
        "source",
        "evidence_ref",
        "validity",
        "status",
        "resource_version",
    ])
}

fn identity_resolution_fields() -> BTreeSet<String> {
    fields([
        "party_pair",
        "evidence_history",
        "status",
        "decision_reason",
    ])
}

fn identity_resolution_merge_fields() -> BTreeSet<String> {
    fields([
        "party_pair",
        "decision",
        "survivorship",
        "status",
        "unmerge_decision",
    ])
}

fn party_relationship_fields() -> BTreeSet<String> {
    fields([
        "from_party_ref",
        "to_party_ref",
        "relationship_type",
        "status",
        "validity",
    ])
}

fn task_fields() -> BTreeSet<String> {
    fields([
        "subject",
        "description",
        "owner",
        "related_resources",
        "priority",
        "status",
        "due_at",
        "reminder_at",
        "completed_at",
        "created_at",
        "updated_at",
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{
        CapabilityId, CapabilityVersion, DataClass, ModuleId, PayloadEncoding, SchemaId,
        SchemaVersion,
    };

    fn definition(owner: &str, capability: &str) -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new(capability).unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new(owner).unwrap(),
            input_contract: contract(owner, format!("{capability}.request")),
            output_contract: None,
            risk: CapabilityRisk::Low,
            mutation: false,
            requires_idempotency: false,
            requires_approval: false,
            authorization_policy_id: "test".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn contract(owner: &str, schema: String) -> PayloadContract {
        PayloadContract {
            owner: ModuleId::try_new(owner).unwrap(),
            schema_id: SchemaId::try_new(schema).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [7; 32],
            allowed_data_classes: vec![DataClass::Internal],
            allowed_encodings: vec![PayloadEncoding::Protobuf],
            maximum_size_bytes: 4096,
        }
    }

    #[test]
    fn registry_resolves_module_owned_resources_without_owner_switches() {
        let registry = build_bootstrap_visibility_registry().unwrap();
        let sales = registry
            .resources_for(&definition(SALES_MODULE_ID, "sales.deal.list"))
            .unwrap();
        assert_eq!(sales.len(), 1);
        assert!(sales[0].allowed_fields.contains("amount"));

        let rows = registry
            .resources_for(&definition(
                CUSTOMER_DATA_OPERATIONS_MODULE_ID,
                LIST_IMPORT_ROWS_CAPABILITY,
            ))
            .unwrap();
        assert_eq!(rows.len(), 2);

        for capability in [GET_PROVIDER_PROFILE_CAPABILITY, GET_MAPPING_CAPABILITY] {
            let enrichment = registry
                .resources_for(&definition(CUSTOMER_ENRICHMENT_MODULE_ID, capability))
                .unwrap();
            assert_eq!(enrichment.len(), 1);
            assert_eq!(
                enrichment[0].resource_type,
                CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_RECORD_TYPE
            );
            assert_eq!(enrichment[0].allowed_fields, fields(["definition"]));
        }

        for capability in [
            GET_ENRICHMENT_REQUEST_CAPABILITY,
            LIST_ENRICHMENT_REQUESTS_CAPABILITY,
        ] {
            let enrichment = registry
                .resources_for(&definition(CUSTOMER_ENRICHMENT_MODULE_ID, capability))
                .unwrap();
            assert_eq!(enrichment.len(), 2);
            assert_eq!(
                enrichment[0],
                resource(
                    CUSTOMER_ENRICHMENT_MODULE_ID,
                    PARTY_RECORD_TYPE,
                    BTreeSet::new(),
                )
            );
            assert_eq!(
                enrichment[1],
                resource(
                    CUSTOMER_ENRICHMENT_MODULE_ID,
                    CUSTOMER_ENRICHMENT_REQUEST_RECORD_TYPE,
                    customer_enrichment_request_fields(),
                )
            );
        }
    }

    #[test]
    fn registry_rejects_undeclared_query_owner() {
        let registry = build_bootstrap_visibility_registry().unwrap();
        let error = registry
            .resources_for(&definition("crm.unknown", "unknown.query"))
            .unwrap_err();
        assert_eq!(
            error.code,
            "APPLICATION_BOOTSTRAP_VISIBILITY_OWNER_UNSUPPORTED"
        );
    }
}
