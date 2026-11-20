#![forbid(unsafe_code)]

//! Module-owned declarative field-visibility contribution for Customer Enrichment queries.
//!
//! This crate is the single source of truth for the exact query-coordinate to governed-resource
//! mapping. The process host only registers and converts this declaration; it contains no
//! Customer Enrichment route switch or field vocabulary.

use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, MODULE_ID, PROVIDER_PROFILE_VERSION_RECORD_TYPE,
    REVIEW_DECISION_RECORD_TYPE, SUGGESTION_RECORD_TYPE,
};
use crm_customer_enrichment_query_adapter::{
    GET_ENRICHMENT_REQUEST_CAPABILITY, GET_MAPPING_CAPABILITY, GET_PROVIDER_PROFILE_CAPABILITY,
};
use crm_customer_enrichment_request_list_query_adapter::LIST_ENRICHMENT_REQUESTS_CAPABILITY;
use crm_customer_enrichment_suggestion_query_adapter::{
    GET_SUGGESTION_CAPABILITY, LIST_SUGGESTIONS_BY_PARTY_CAPABILITY,
};
use std::collections::BTreeSet;

pub const CRATE_NAME: &str = "crm-customer-enrichment-visibility";
pub const PARTY_RECORD_TYPE: &str = "parties.party";

pub const QUERY_VISIBILITY_CAPABILITY_IDS: &[&str] = &[
    GET_PROVIDER_PROFILE_CAPABILITY,
    GET_MAPPING_CAPABILITY,
    GET_ENRICHMENT_REQUEST_CAPABILITY,
    LIST_ENRICHMENT_REQUESTS_CAPABILITY,
    GET_SUGGESTION_CAPABILITY,
    LIST_SUGGESTIONS_BY_PARTY_CAPABILITY,
];

const DEFINITION_FIELDS: &[&str] = &["definition"];
const REQUEST_FIELDS: &[&str] = &[
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
];
const SUGGESTION_FIELDS: &[&str] = &[
    "enrichment_request_ref",
    "provider_response_receipt_ref",
    "provider_profile_version_ref",
    "mapping_version_ref",
    "target",
    "proposed_value",
    "proposed_value_digest",
    "observed_at_unix_ms",
    "retrieved_at_unix_ms",
    "effective_at_unix_ms",
    "fresh_until_unix_ms",
    "expires_at_unix_ms",
    "confidence_basis_points",
    "policy_evidence",
    "evidence_references",
    "lifecycle_status",
    "superseded_by_suggestion_ref",
];
const REVIEW_DECISION_FIELDS: &[&str] = &[
    "suggestion_ref",
    "target_party_resource_version",
    "proposed_value_digest",
    "reviewed_by_actor_id",
    "kind",
    "policy_version",
    "safe_reason_code",
    "approval_evidence_reference",
    "decided_at_unix_ms",
    "expires_at_unix_ms",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibilityResourceDeclaration {
    resource_type: &'static str,
    allowed_fields: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibilityCapabilityDeclaration {
    capability_id: &'static str,
    resources: &'static [VisibilityResourceDeclaration],
}

const DEFINITION_RESOURCES: &[VisibilityResourceDeclaration] = &[VisibilityResourceDeclaration {
    resource_type: PROVIDER_PROFILE_VERSION_RECORD_TYPE,
    allowed_fields: DEFINITION_FIELDS,
}];

const REQUEST_RESOURCES: &[VisibilityResourceDeclaration] = &[
    // Party visibility is a resource-existence gate. Customer Enrichment discloses no Party fields.
    VisibilityResourceDeclaration {
        resource_type: PARTY_RECORD_TYPE,
        allowed_fields: &[],
    },
    VisibilityResourceDeclaration {
        resource_type: ENRICHMENT_REQUEST_RECORD_TYPE,
        allowed_fields: REQUEST_FIELDS,
    },
];

const SUGGESTION_RESOURCES: &[VisibilityResourceDeclaration] = &[
    VisibilityResourceDeclaration {
        resource_type: PARTY_RECORD_TYPE,
        allowed_fields: &[],
    },
    VisibilityResourceDeclaration {
        resource_type: SUGGESTION_RECORD_TYPE,
        allowed_fields: SUGGESTION_FIELDS,
    },
    VisibilityResourceDeclaration {
        resource_type: REVIEW_DECISION_RECORD_TYPE,
        allowed_fields: REVIEW_DECISION_FIELDS,
    },
];

const VISIBILITY_DECLARATIONS: &[VisibilityCapabilityDeclaration] = &[
    VisibilityCapabilityDeclaration {
        capability_id: GET_PROVIDER_PROFILE_CAPABILITY,
        resources: DEFINITION_RESOURCES,
    },
    VisibilityCapabilityDeclaration {
        capability_id: GET_MAPPING_CAPABILITY,
        resources: DEFINITION_RESOURCES,
    },
    VisibilityCapabilityDeclaration {
        capability_id: GET_ENRICHMENT_REQUEST_CAPABILITY,
        resources: REQUEST_RESOURCES,
    },
    VisibilityCapabilityDeclaration {
        capability_id: LIST_ENRICHMENT_REQUESTS_CAPABILITY,
        resources: REQUEST_RESOURCES,
    },
    VisibilityCapabilityDeclaration {
        capability_id: GET_SUGGESTION_CAPABILITY,
        resources: SUGGESTION_RESOURCES,
    },
    VisibilityCapabilityDeclaration {
        capability_id: LIST_SUGGESTIONS_BY_PARTY_CAPABILITY,
        resources: SUGGESTION_RESOURCES,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerEnrichmentVisibilityResource {
    pub owner_module_id: &'static str,
    pub resource_type: &'static str,
    pub allowed_fields: BTreeSet<String>,
}

/// Resolves the exact module-owned visibility declaration for one query capability.
/// Unknown coordinates fail closed by contributing no bootstrap visibility grant.
pub fn query_visibility_resources(
    capability_id: &str,
) -> Vec<CustomerEnrichmentVisibilityResource> {
    VISIBILITY_DECLARATIONS
        .iter()
        .find(|declaration| declaration.capability_id == capability_id)
        .map(|declaration| {
            declaration
                .resources
                .iter()
                .map(|resource| CustomerEnrichmentVisibilityResource {
                    owner_module_id: MODULE_ID,
                    resource_type: resource.resource_type,
                    allowed_fields: resource
                        .allowed_fields
                        .iter()
                        .copied()
                        .map(str::to_owned)
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(values: &[&str]) -> BTreeSet<String> {
        values.iter().copied().map(str::to_owned).collect()
    }

    #[test]
    fn declaration_covers_exact_six_query_coordinates_once() {
        let declared = VISIBILITY_DECLARATIONS
            .iter()
            .map(|declaration| declaration.capability_id)
            .collect::<BTreeSet<_>>();
        let expected = QUERY_VISIBILITY_CAPABILITY_IDS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(VISIBILITY_DECLARATIONS.len(), expected.len());
        assert_eq!(declared, expected);
    }

    #[test]
    fn definition_queries_share_exact_provider_profile_visibility() {
        for capability in [GET_PROVIDER_PROFILE_CAPABILITY, GET_MAPPING_CAPABILITY] {
            assert_eq!(
                query_visibility_resources(capability),
                vec![CustomerEnrichmentVisibilityResource {
                    owner_module_id: MODULE_ID,
                    resource_type: PROVIDER_PROFILE_VERSION_RECORD_TYPE,
                    allowed_fields: fields(DEFINITION_FIELDS),
                }]
            );
        }
    }

    #[test]
    fn request_queries_share_party_gate_and_request_fields() {
        for capability in [
            GET_ENRICHMENT_REQUEST_CAPABILITY,
            LIST_ENRICHMENT_REQUESTS_CAPABILITY,
        ] {
            let resources = query_visibility_resources(capability);
            assert_eq!(resources.len(), 2);
            assert_eq!(resources[0].resource_type, PARTY_RECORD_TYPE);
            assert!(resources[0].allowed_fields.is_empty());
            assert_eq!(resources[1].resource_type, ENRICHMENT_REQUEST_RECORD_TYPE);
            assert_eq!(resources[1].allowed_fields, fields(REQUEST_FIELDS));
        }
    }

    #[test]
    fn suggestion_get_and_list_share_exact_lifecycle_visibility() {
        for capability in [
            GET_SUGGESTION_CAPABILITY,
            LIST_SUGGESTIONS_BY_PARTY_CAPABILITY,
        ] {
            let resources = query_visibility_resources(capability);
            assert_eq!(resources.len(), 3);
            assert_eq!(resources[0].resource_type, PARTY_RECORD_TYPE);
            assert!(resources[0].allowed_fields.is_empty());
            assert_eq!(resources[1].resource_type, SUGGESTION_RECORD_TYPE);
            assert_eq!(resources[1].allowed_fields, fields(SUGGESTION_FIELDS));
            assert_eq!(resources[2].resource_type, REVIEW_DECISION_RECORD_TYPE);
            assert_eq!(resources[2].allowed_fields, fields(REVIEW_DECISION_FIELDS));
        }
    }

    #[test]
    fn unknown_query_contributes_no_visibility() {
        assert!(query_visibility_resources("customer_enrichment.unknown").is_empty());
    }
}
