#![forbid(unsafe_code)]

//! Published-contract adapter for governed Admin Studio metadata operations.
//!
//! The adapter owns exact public capability/query coordinates and the boundary
//! from caller-authored strict v1 definitions to immutable metadata runtime
//! documents. It deliberately does not accept caller-built `MetadataDocument`
//! values, dependency sets, revision hashes or persistence instructions.

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_metadata_runtime::{
    MetadataBundleDraft, MetadataChangeType, MetadataDocument, MetadataImpactReport,
    MetadataImpactSeverity, MetadataKey, MetadataKind, MetadataRevisionId, TenantMetadataSnapshot,
};
use crm_metadata_schema::{METADATA_DEFINITION_SCHEMA_VERSION, MetadataDefinition};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, SdkError,
};
use crm_proto_contracts::crm::metadata::v1 as wire;

pub const METADATA_MODULE_ID: &str = "crm.metadata";
pub const CONTRACT_VERSION: &str = support::CONTRACT_VERSION;

pub const PUBLISH_BUNDLE_CAPABILITY: &str = "metadata.bundle.publish";
pub const ACTIVATE_REVISION_CAPABILITY: &str = "metadata.revision.activate";
pub const ROLLBACK_REVISION_CAPABILITY: &str = "metadata.revision.rollback";

pub const IMPACT_QUERY_CAPABILITY: &str = "metadata.bundle.impact";
pub const REVISION_QUERY_CAPABILITY: &str = "metadata.revision.get";
pub const ACTIVATION_QUERY_CAPABILITY: &str = "metadata.activation.get";

pub const PUBLISH_REQUEST_SCHEMA: &str = "crm.metadata.v1.PublishMetadataBundleRequest";
pub const PUBLISH_RESPONSE_SCHEMA: &str = "crm.metadata.v1.PublishMetadataBundleResponse";
pub const ACTIVATE_REQUEST_SCHEMA: &str = "crm.metadata.v1.ActivateMetadataRevisionRequest";
pub const ACTIVATE_RESPONSE_SCHEMA: &str = "crm.metadata.v1.ActivateMetadataRevisionResponse";
pub const ROLLBACK_REQUEST_SCHEMA: &str = "crm.metadata.v1.RollbackMetadataRevisionRequest";
pub const ROLLBACK_RESPONSE_SCHEMA: &str = "crm.metadata.v1.RollbackMetadataRevisionResponse";
pub const IMPACT_REQUEST_SCHEMA: &str = "crm.metadata.v1.GetMetadataImpactRequest";
pub const IMPACT_RESPONSE_SCHEMA: &str = "crm.metadata.v1.GetMetadataImpactResponse";
pub const REVISION_REQUEST_SCHEMA: &str = "crm.metadata.v1.GetMetadataRevisionRequest";
pub const REVISION_RESPONSE_SCHEMA: &str = "crm.metadata.v1.GetMetadataRevisionResponse";
pub const ACTIVATION_REQUEST_SCHEMA: &str = "crm.metadata.v1.GetMetadataActivationRequest";
pub const ACTIVATION_RESPONSE_SCHEMA: &str = "crm.metadata.v1.GetMetadataActivationResponse";

pub const METADATA_MUTATION_CAPABILITY_IDS: [&str; 3] = [
    PUBLISH_BUNDLE_CAPABILITY,
    ACTIVATE_REVISION_CAPABILITY,
    ROLLBACK_REVISION_CAPABILITY,
];

pub const METADATA_QUERY_CAPABILITY_IDS: [&str; 3] = [
    IMPACT_QUERY_CAPABILITY,
    REVISION_QUERY_CAPABILITY,
    ACTIVATION_QUERY_CAPABILITY,
];

pub fn metadata_capability_definition(
    capability_id: &str,
) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema, mutation, risk) = match capability_id {
        PUBLISH_BUNDLE_CAPABILITY => (
            PUBLISH_REQUEST_SCHEMA,
            PUBLISH_RESPONSE_SCHEMA,
            true,
            CapabilityRisk::High,
        ),
        ACTIVATE_REVISION_CAPABILITY => (
            ACTIVATE_REQUEST_SCHEMA,
            ACTIVATE_RESPONSE_SCHEMA,
            true,
            CapabilityRisk::High,
        ),
        ROLLBACK_REVISION_CAPABILITY => (
            ROLLBACK_REQUEST_SCHEMA,
            ROLLBACK_RESPONSE_SCHEMA,
            true,
            CapabilityRisk::High,
        ),
        IMPACT_QUERY_CAPABILITY => (
            IMPACT_REQUEST_SCHEMA,
            IMPACT_RESPONSE_SCHEMA,
            false,
            CapabilityRisk::Low,
        ),
        REVISION_QUERY_CAPABILITY => (
            REVISION_REQUEST_SCHEMA,
            REVISION_RESPONSE_SCHEMA,
            false,
            CapabilityRisk::Low,
        ),
        ACTIVATION_QUERY_CAPABILITY => (
            ACTIVATION_REQUEST_SCHEMA,
            ACTIVATION_RESPONSE_SCHEMA,
            false,
            CapabilityRisk::Low,
        ),
        _ => return Err(unsupported_coordinate()),
    };

    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(METADATA_MODULE_ID))?,
        input_contract: support::protobuf_contract(
            METADATA_MODULE_ID,
            input_schema,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            METADATA_MODULE_ID,
            output_schema,
            vec![DataClass::Confidential],
        )?),
        risk,
        mutation,
        requires_idempotency: mutation,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn metadata_mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    METADATA_MUTATION_CAPABILITY_IDS
        .iter()
        .map(|capability_id| metadata_capability_definition(capability_id))
        .collect()
}

pub fn metadata_query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    METADATA_QUERY_CAPABILITY_IDS
        .iter()
        .map(|capability_id| metadata_capability_definition(capability_id))
        .collect()
}

pub fn decode_publish_bundle(request: &CapabilityRequest) -> Result<MetadataBundleDraft, SdkError> {
    let command: wire::PublishMetadataBundleRequest =
        support::decode_request(request, METADATA_MODULE_ID, PUBLISH_REQUEST_SCHEMA)?;
    publish_bundle_from_wire(command)
}

pub fn publish_bundle_from_wire(
    command: wire::PublishMetadataBundleRequest,
) -> Result<MetadataBundleDraft, SdkError> {
    let documents = command
        .definitions
        .into_iter()
        .map(definition_input_to_document)
        .collect::<Result<Vec<_>, _>>()?;

    MetadataBundleDraft::new(documents).map_err(|error| {
        SdkError::new(
            "METADATA_BUNDLE_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The metadata bundle is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

pub fn definition_input_to_document(
    input: wire::MetadataDefinitionInput,
) -> Result<MetadataDocument, SdkError> {
    if input.schema_version != METADATA_DEFINITION_SCHEMA_VERSION {
        return Err(SdkError::new(
            "METADATA_DEFINITION_SCHEMA_VERSION_UNSUPPORTED",
            ErrorCategory::InvalidArgument,
            false,
            "The metadata definition schema version is unsupported.",
        ));
    }

    let definition: MetadataDefinition = serde_json::from_slice(&input.definition_json).map_err(|error| {
        SdkError::new(
            "METADATA_DEFINITION_JSON_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The metadata definition is invalid.",
        )
        .with_internal_reference(error.to_string())
    })?;

    definition.to_document().map_err(|error| {
        SdkError::new(
            "METADATA_DEFINITION_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The metadata definition failed validation.",
        )
        .with_internal_reference(error.to_string())
    })
}

pub fn parse_revision_id(value: &str, field: &'static str) -> Result<MetadataRevisionId, SdkError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_revision_id(field));
    }

    let mut bytes = [0_u8; 32];
    for (index, slot) in bytes.iter_mut().enumerate() {
        let offset = index * 2;
        *slot = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| invalid_revision_id(field))?;
    }
    Ok(MetadataRevisionId::from_bytes(bytes))
}

pub fn key_to_wire(key: &MetadataKey) -> wire::MetadataKey {
    wire::MetadataKey {
        kind: kind_to_wire(key.kind()) as i32,
        id: key.id().as_str().to_owned(),
    }
}

pub fn document_to_wire(document: &MetadataDocument) -> wire::MetadataDocument {
    wire::MetadataDocument {
        key: Some(key_to_wire(document.key())),
        schema_version: document.schema_version().to_owned(),
        canonical_content_json: document.canonical_content().to_vec(),
        dependencies: document.dependencies().iter().map(key_to_wire).collect(),
    }
}

pub fn revision_to_wire(
    revision_id: &MetadataRevisionId,
    bundle: &MetadataBundleDraft,
) -> wire::MetadataRevision {
    wire::MetadataRevision {
        revision_id: revision_id.to_hex(),
        documents: bundle.documents().values().map(document_to_wire).collect(),
    }
}

pub fn impact_to_wire(impact: &MetadataImpactReport) -> wire::MetadataImpact {
    wire::MetadataImpact {
        current_revision_id: impact
            .current_revision
            .as_ref()
            .map(MetadataRevisionId::to_hex)
            .unwrap_or_default(),
        candidate_revision_id: impact.candidate_revision.to_hex(),
        changes: impact
            .changes
            .iter()
            .map(|change| wire::MetadataChange {
                key: Some(key_to_wire(&change.key)),
                change_type: change_type_to_wire(change.change_type) as i32,
                severity: severity_to_wire(change.severity) as i32,
            })
            .collect(),
        has_breaking_changes: impact.has_breaking_changes(),
        requires_review: impact.requires_review(),
    }
}

pub fn activation_state_to_wire(state: &TenantMetadataSnapshot) -> wire::MetadataActivationState {
    wire::MetadataActivationState {
        generation: state.generation,
        active_revision_id: state
            .active_revision
            .as_ref()
            .map(MetadataRevisionId::to_hex)
            .unwrap_or_default(),
        rollback_depth: u64::try_from(state.rollback_depth).unwrap_or(u64::MAX),
    }
}

fn kind_to_wire(kind: MetadataKind) -> wire::MetadataKind {
    match kind {
        MetadataKind::Object => wire::MetadataKind::Object,
        MetadataKind::Field => wire::MetadataKind::Field,
        MetadataKind::Relationship => wire::MetadataKind::Relationship,
        MetadataKind::Layout => wire::MetadataKind::Layout,
        MetadataKind::View => wire::MetadataKind::View,
        MetadataKind::Pipeline => wire::MetadataKind::Pipeline,
        MetadataKind::Permission => wire::MetadataKind::Permission,
        MetadataKind::Workflow => wire::MetadataKind::Workflow,
    }
}

fn change_type_to_wire(change_type: MetadataChangeType) -> wire::MetadataChangeType {
    match change_type {
        MetadataChangeType::Added => wire::MetadataChangeType::Added,
        MetadataChangeType::Modified => wire::MetadataChangeType::Modified,
        MetadataChangeType::Removed => wire::MetadataChangeType::Removed,
    }
}

fn severity_to_wire(severity: MetadataImpactSeverity) -> wire::MetadataImpactSeverity {
    match severity {
        MetadataImpactSeverity::Informational => wire::MetadataImpactSeverity::Informational,
        MetadataImpactSeverity::ReviewRequired => wire::MetadataImpactSeverity::ReviewRequired,
        MetadataImpactSeverity::Breaking => wire::MetadataImpactSeverity::Breaking,
    }
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "METADATA_API_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The metadata API configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn unsupported_coordinate() -> SdkError {
    SdkError::new(
        "METADATA_API_COORDINATE_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The metadata API coordinate is unsupported.",
    )
}

fn invalid_revision_id(field: &'static str) -> SdkError {
    SdkError::new(
        "METADATA_REVISION_ID_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The metadata revision identifier is invalid.",
    )
    .with_internal_reference(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object_definition_json(extra: &str) -> Vec<u8> {
        format!(
            r#"{{"kind":"object","definition":{{"id":"crm.sales.deal","owner_module_id":"crm.sales","label":"Deal","plural_label":"Deals","description":null,"tags":["sales","commercial"]{extra}}}}}"#
        )
        .into_bytes()
    }

    #[test]
    fn publishes_three_mutations_and_three_queries_with_exact_coordinates() {
        let mutations = metadata_mutation_capability_definitions().unwrap();
        let queries = metadata_query_capability_definitions().unwrap();

        assert_eq!(
            mutations
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            METADATA_MUTATION_CAPABILITY_IDS
        );
        assert_eq!(
            queries
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            METADATA_QUERY_CAPABILITY_IDS
        );
        assert!(mutations.iter().all(|definition| {
            definition.owner_module_id.as_str() == METADATA_MODULE_ID
                && definition.mutation
                && definition.requires_idempotency
        }));
        assert!(queries.iter().all(|definition| {
            definition.owner_module_id.as_str() == METADATA_MODULE_ID
                && !definition.mutation
                && !definition.requires_idempotency
                && !definition.requires_approval
        }));
    }

    #[test]
    fn strict_authoring_input_is_validated_and_canonicalized_server_side() {
        let bundle = publish_bundle_from_wire(wire::PublishMetadataBundleRequest {
            definitions: vec![wire::MetadataDefinitionInput {
                schema_version: METADATA_DEFINITION_SCHEMA_VERSION.to_owned(),
                definition_json: object_definition_json(""),
            }],
        })
        .unwrap();

        let document = bundle.documents().values().next().unwrap();
        let canonical = std::str::from_utf8(document.canonical_content()).unwrap();
        assert!(canonical.contains(r#""tags":["commercial","sales"]"#));
        assert_eq!(document.schema_version(), METADATA_DEFINITION_SCHEMA_VERSION);
    }

    #[test]
    fn unknown_authoring_fields_are_rejected_before_document_construction() {
        let error = publish_bundle_from_wire(wire::PublishMetadataBundleRequest {
            definitions: vec![wire::MetadataDefinitionInput {
                schema_version: METADATA_DEFINITION_SCHEMA_VERSION.to_owned(),
                definition_json: object_definition_json(",\"raw_sql\":\"select 1\""),
            }],
        })
        .unwrap_err();

        assert_eq!(error.code, "METADATA_DEFINITION_JSON_INVALID");
    }

    #[test]
    fn caller_cannot_choose_a_different_definition_schema_version() {
        let error = definition_input_to_document(wire::MetadataDefinitionInput {
            schema_version: "crm.metadata.definition/v999".to_owned(),
            definition_json: object_definition_json(""),
        })
        .unwrap_err();

        assert_eq!(
            error.code,
            "METADATA_DEFINITION_SCHEMA_VERSION_UNSUPPORTED"
        );
    }

    #[test]
    fn revision_identity_parser_requires_exact_lower_or_upper_hex_bytes() {
        let value = "ab".repeat(32);
        assert_eq!(parse_revision_id(&value, "revision_id").unwrap().to_hex(), value);
        assert_eq!(
            parse_revision_id("not-a-revision", "revision_id")
                .unwrap_err()
                .code,
            "METADATA_REVISION_ID_INVALID"
        );
    }
}
