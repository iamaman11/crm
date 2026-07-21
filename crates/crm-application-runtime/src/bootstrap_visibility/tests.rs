use super::registry::{
    ACTIVITIES_MODULE_ID, BootstrapVisibilityResource, SALES_MODULE_ID,
    build_bootstrap_visibility_registry,
};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk, PayloadContract};
use crm_customer_data_operations_capability_adapter::MODULE_ID as CUSTOMER_DATA_OPERATIONS_MODULE_ID;
use crm_customer_data_operations_query_adapter::LIST_IMPORT_ROWS_CAPABILITY;
use crm_customer_enrichment::MODULE_ID as CUSTOMER_ENRICHMENT_MODULE_ID;
use crm_customer_enrichment_visibility::{
    QUERY_VISIBILITY_CAPABILITY_IDS, query_visibility_resources,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ModuleId, PayloadEncoding, SchemaId, SchemaVersion,
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

    let activities = registry
        .resources_for(&definition(ACTIVITIES_MODULE_ID, "activities.task.list"))
        .unwrap();
    assert_eq!(activities.len(), 1);
    assert!(activities[0].allowed_fields.contains("subject"));

    let rows = registry
        .resources_for(&definition(
            CUSTOMER_DATA_OPERATIONS_MODULE_ID,
            LIST_IMPORT_ROWS_CAPABILITY,
        ))
        .unwrap();
    assert_eq!(rows.len(), 2);

    assert_eq!(QUERY_VISIBILITY_CAPABILITY_IDS.len(), 6);
    for capability in QUERY_VISIBILITY_CAPABILITY_IDS {
        let actual = registry
            .resources_for(&definition(CUSTOMER_ENRICHMENT_MODULE_ID, capability))
            .unwrap();
        let expected = query_visibility_resources(capability)
            .into_iter()
            .map(|resource| BootstrapVisibilityResource {
                owner_module_id: resource.owner_module_id,
                resource_type: resource.resource_type,
                allowed_fields: resource.allowed_fields,
            })
            .collect::<Vec<_>>();
        assert!(!actual.is_empty(), "missing visibility for {capability}");
        assert_eq!(actual, expected);
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
