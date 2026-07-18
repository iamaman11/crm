#![forbid(unsafe_code)]

//! Governed mutation adapters for `crm.customer-enrichment`.
//!
//! Immutable definition publication and request lifecycle changes use the shared transactional
//! record/idempotency/outbox/audit runtime. Provider network I/O, credentials and owner mutation
//! remain outside this crate; governed Party and Consent pre-authorization is composed separately.

mod mapping_planner;
mod mapping_reference_planner;
mod mapping_snapshot;
mod provider_profile_planner;
mod provider_profile_snapshot;
mod request_cancel_planner;
mod request_planner;
mod request_reference_planner;
mod request_snapshot;
mod semantic_validator;

pub use mapping_planner::{
    CustomerEnrichmentMappingCapabilityPlanner, mapping_from_definition,
    mapping_persisted_contract, mapping_persisted_payload, mapping_record_ref, mapping_to_wire,
    provider_profile_version_id_from_external,
};
pub use mapping_reference_planner::CustomerEnrichmentMappingReferencePlanner;
pub use mapping_snapshot::mapping_from_snapshot;
pub use provider_profile_planner::{
    provider_profile_from_definition, provider_profile_persisted_contract,
    provider_profile_persisted_payload, provider_profile_record_ref, provider_profile_to_wire,
};
pub use provider_profile_snapshot::provider_profile_from_snapshot;
pub use request_cancel_planner::*;
pub use request_planner::*;
pub use request_reference_planner::CustomerEnrichmentRequestReferencePlanner;
pub use request_snapshot::enrichment_request_from_snapshot;
pub use semantic_validator::CustomerEnrichmentCapabilitySemanticValidator;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{AggregateTarget, CapabilityBatchExecutionPlan, TransactionalAggregatePlanner};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestStatus, ProviderResponseClass, ProviderResponseReceipt,
    ProviderResponseReceiptDraft,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordSnapshot, SdkError,
};

pub const MODULE_ID: &str = crm_customer_enrichment::MODULE_ID;
pub const PROVIDER_PROFILE_VERSION_RECORD_TYPE: &str =
    crm_customer_enrichment::PROVIDER_PROFILE_VERSION_RECORD_TYPE;
pub const MAPPING_VERSION_RECORD_TYPE: &str = crm_customer_enrichment::MAPPING_VERSION_RECORD_TYPE;

pub const PUBLISH_PROVIDER_PROFILE_CAPABILITY: &str =
    "customer_enrichment.provider_profile.publish";
pub const PUBLISH_PROVIDER_PROFILE_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishProviderProfileVersionRequest";
pub const PUBLISH_PROVIDER_PROFILE_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishProviderProfileVersionResponse";
pub const PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE: &str =
    "customer_enrichment.provider_profile.published";
pub const PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderProfileVersionPublishedEvent";

pub const PUBLISH_MAPPING_CAPABILITY: &str = "customer_enrichment.mapping.publish";
pub const PUBLISH_MAPPING_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishMappingVersionRequest";
pub const PUBLISH_MAPPING_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishMappingVersionResponse";
pub const MAPPING_PUBLISHED_EVENT_TYPE: &str = "customer_enrichment.mapping.published";
pub const MAPPING_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.MappingVersionPublishedEvent";

pub const DISPATCH_ENRICHMENT_REQUEST_CAPABILITY: &str = "customer_enrichment.request.dispatch";
pub const DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.DispatchEnrichmentRequestRequest";
pub const DISPATCH_ENRICHMENT_REQUEST_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.DispatchEnrichmentRequestResponse";

pub const RECORD_PROVIDER_RESPONSE_CAPABILITY: &str = "customer_enrichment.response.record";
pub const RECORD_PROVIDER_RESPONSE_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.RecordProviderResponseRequest";
pub const RECORD_PROVIDER_RESPONSE_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.RecordProviderResponseResponse";

/// Exact optimistic lifecycle expectation supplied to one provider-dispatch attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchExpectation {
    pub status: EnrichmentRequestStatus,
    pub retry_generation: u32,
}

/// Exact optimistic lifecycle expectation supplied to one response-recording attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseExpectation {
    pub status: EnrichmentRequestStatus,
    pub retry_generation: u32,
}

/// Sanitized canonical response evidence accepted from infrastructure-owned provider adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseEvidence {
    pub replay_key: String,
    pub provider_correlation_id: Option<String>,
    pub response_class: ProviderResponseClass,
    pub canonical_response_digest: [u8; 32],
    pub provider_observed_at_unix_ms: Option<u64>,
    pub retrieved_at_unix_ms: u64,
    pub metered_units: u64,
    pub protected_evidence_reference: Option<String>,
}

/// Applies the module-owned transition after infrastructure dispatch has succeeded.
///
/// Provider adapter selection, credential resolution, network I/O and payload sanitization remain
/// infrastructure-owned. This function only verifies the worker's exact optimistic expectation and
/// advances the deterministic request lifecycle.
pub fn prepare_request_dispatch(
    request: &mut EnrichmentRequest,
    expectation: DispatchExpectation,
    dispatched_at_unix_ms: u64,
) -> Result<(), SdkError> {
    if request.status() != expectation.status
        || request.retry_generation() != expectation.retry_generation
    {
        return Err(dispatch_conflict());
    }
    match expectation.status {
        EnrichmentRequestStatus::Created | EnrichmentRequestStatus::FailedRetryable => {
            request.queue(dispatched_at_unix_ms)?;
            request.mark_dispatched(dispatched_at_unix_ms)
        }
        EnrichmentRequestStatus::Queued => request.mark_dispatched(dispatched_at_unix_ms),
        _ => Err(dispatch_conflict()),
    }
}

/// Validates immutable sanitized provider evidence and binds it to the exact dispatched request.
///
/// Receipt construction happens before request mutation. Invalid digest, timestamp or bounded
/// evidence therefore leaves the request unchanged. The returned receipt and request transition
/// are intended to be persisted atomically by the future crash-safe worker composition.
pub fn prepare_provider_response(
    request: &mut EnrichmentRequest,
    expectation: ResponseExpectation,
    evidence: ProviderResponseEvidence,
) -> Result<ProviderResponseReceipt, SdkError> {
    if expectation.status != EnrichmentRequestStatus::Dispatched
        || request.status() != expectation.status
        || request.retry_generation() != expectation.retry_generation
    {
        return Err(response_conflict());
    }
    let retrieved_at_unix_ms = evidence.retrieved_at_unix_ms;
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.request_id().clone(),
        provider_profile_version_id: request.provider_profile_version_id().clone(),
        mapping_version_id: request.mapping_version_id().clone(),
        replay_key: evidence.replay_key,
        provider_correlation_id: evidence.provider_correlation_id,
        response_class: evidence.response_class,
        canonical_response_digest: evidence.canonical_response_digest,
        provider_observed_at_unix_ms: evidence.provider_observed_at_unix_ms,
        retrieved_at_unix_ms,
        metered_units: evidence.metered_units,
        protected_evidence_reference: evidence.protected_evidence_reference,
    })?;
    request.record_response(receipt.receipt_id().clone(), retrieved_at_unix_ms)?;
    Ok(receipt)
}

/// Exact mutation coordinates registered by production composition.
pub const IMPLEMENTED_MUTATION_CAPABILITY_IDS: &[&str] = &[
    PUBLISH_PROVIDER_PROFILE_CAPABILITY,
    PUBLISH_MAPPING_CAPABILITY,
    CREATE_ENRICHMENT_REQUEST_CAPABILITY,
    CANCEL_ENRICHMENT_REQUEST_CAPABILITY,
];

/// Module-owned planner router. The public type name is retained so production composition does
/// not gain a capability-specific switch as additional enrichment coordinates are implemented.
#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentProviderProfileCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentProviderProfileCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        match definition.capability_id.as_str() {
            PUBLISH_PROVIDER_PROFILE_CAPABILITY => {
                provider_profile_planner::CustomerEnrichmentProviderProfileCapabilityPlanner
                    .target(definition, request)
            }
            PUBLISH_MAPPING_CAPABILITY => {
                CustomerEnrichmentMappingReferencePlanner.target(definition, request)
            }
            CREATE_ENRICHMENT_REQUEST_CAPABILITY => {
                CustomerEnrichmentRequestReferencePlanner.target(definition, request)
            }
            CANCEL_ENRICHMENT_REQUEST_CAPABILITY => {
                CustomerEnrichmentRequestCancelPlanner.target(definition, request)
            }
            _ => Err(unsupported_capability()),
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        match definition.capability_id.as_str() {
            PUBLISH_PROVIDER_PROFILE_CAPABILITY => {
                provider_profile_planner::CustomerEnrichmentProviderProfileCapabilityPlanner
                    .plan(definition, request, current)
            }
            PUBLISH_MAPPING_CAPABILITY => {
                CustomerEnrichmentMappingReferencePlanner.plan(definition, request, current)
            }
            CREATE_ENRICHMENT_REQUEST_CAPABILITY => {
                CustomerEnrichmentRequestReferencePlanner.plan(definition, request, current)
            }
            CANCEL_ENRICHMENT_REQUEST_CAPABILITY => {
                CustomerEnrichmentRequestCancelPlanner.plan(definition, request, current)
            }
            _ => Err(unsupported_capability()),
        }
    }
}

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        provider_profile_capability_definition()?,
        mapping_capability_definition()?,
        request_create_capability_definition()?,
        request_cancel_capability_definition()?,
    ])
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    provider_profile_capability_definition()
}

pub fn provider_profile_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        PUBLISH_PROVIDER_PROFILE_CAPABILITY,
        PUBLISH_PROVIDER_PROFILE_REQUEST_SCHEMA,
        PUBLISH_PROVIDER_PROFILE_RESPONSE_SCHEMA,
        DataClass::Confidential,
    )
}

pub fn mapping_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        PUBLISH_MAPPING_CAPABILITY,
        PUBLISH_MAPPING_REQUEST_SCHEMA,
        PUBLISH_MAPPING_RESPONSE_SCHEMA,
        DataClass::Confidential,
    )
}

pub fn request_create_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        CREATE_ENRICHMENT_REQUEST_CAPABILITY,
        CREATE_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        CREATE_ENRICHMENT_REQUEST_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

pub fn request_cancel_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        CANCEL_ENRICHMENT_REQUEST_CAPABILITY,
        CANCEL_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        CANCEL_ENRICHMENT_REQUEST_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

/// Worker-only definition factory retained outside the public production mutation catalog until
/// exact provider-adapter registry and crash-safe orchestration are composed.
pub fn request_dispatch_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        DISPATCH_ENRICHMENT_REQUEST_CAPABILITY,
        DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        DISPATCH_ENRICHMENT_REQUEST_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

/// Worker-only definition factory retained outside the public production mutation catalog until
/// response receipts, request transition and usage evidence are persisted in one crash-safe unit.
pub fn provider_response_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        RECORD_PROVIDER_RESPONSE_CAPABILITY,
        RECORD_PROVIDER_RESPONSE_REQUEST_SCHEMA,
        RECORD_PROVIDER_RESPONSE_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

fn mutation_definition(
    capability_id: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
    data_class: DataClass,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(MODULE_ID, request_schema, vec![data_class])?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            response_schema,
            vec![data_class],
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| configuration_error().with_internal_reference(error.to_string()))
}

fn configuration_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment capability configuration is invalid.",
    )
}

fn dispatch_conflict() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_DISPATCH_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The enrichment request is no longer eligible for this dispatch attempt.",
    )
}

fn response_conflict() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_RESPONSE_EXPECTATION_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The enrichment request no longer matches this provider response attempt.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment mutation capability is not supported.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_enrichment::{
        EnrichmentRequestDraft, MappingDraft, MappingNormalization, MappingVersion,
        ProviderProfileDraft, ProviderProfileVersion, RawPayloadPolicy, RequestPolicyEvidence,
        TargetField, TargetSnapshot,
    };
    use crm_module_sdk::{ActorId, IdempotencyKey, TenantId};

    fn enrichment_request() -> EnrichmentRequest {
        let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "provider".to_owned(),
            adapter_kind: "adapter".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["enrichment".to_owned()],
            license_id: "license-v1".to_owned(),
            permitted_use_class: "customer_data".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::DigestOnly,
            credential_handle_aliases: vec!["provider_key".to_owned()],
            effective_at_unix_ms: 1,
            expires_at_unix_ms: None,
        })
        .unwrap();
        let mapping = MappingVersion::publish(MappingDraft {
            mapping_key: "display_name".to_owned(),
            provider_profile_version_id: profile.version_id().clone(),
            provider_response_field_path: "person.display_name".to_owned(),
            target_field: TargetField::PartyDisplayName,
            normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
            maximum_suggestions_per_response: 1,
            confidence_required: false,
        })
        .unwrap();
        EnrichmentRequest::create(EnrichmentRequestDraft {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            requested_by: ActorId::try_new("worker-a").unwrap(),
            idempotency_key: IdempotencyKey::try_new("dispatch-test").unwrap(),
            target: TargetSnapshot::try_new("party-a", 1, TargetField::PartyDisplayName).unwrap(),
            provider_profile_version_id: profile.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            requested_fields: vec![TargetField::PartyDisplayName],
            policy_evidence: RequestPolicyEvidence::try_new(
                "enrichment",
                "legitimate_interest",
                None,
                "request-policy-v1",
            )
            .unwrap(),
            created_at_unix_ms: 1,
            deadline_at_unix_ms: 100,
            expires_at_unix_ms: 200,
        })
        .unwrap()
    }

    fn dispatched_request() -> EnrichmentRequest {
        let mut request = enrichment_request();
        prepare_request_dispatch(
            &mut request,
            DispatchExpectation {
                status: EnrichmentRequestStatus::Created,
                retry_generation: 0,
            },
            2,
        )
        .unwrap();
        request
    }

    fn response_evidence() -> ProviderResponseEvidence {
        ProviderResponseEvidence {
            replay_key: "provider-response-1".to_owned(),
            provider_correlation_id: Some("correlation-1".to_owned()),
            response_class: ProviderResponseClass::Success,
            canonical_response_digest: [7; 32],
            provider_observed_at_unix_ms: Some(2),
            retrieved_at_unix_ms: 3,
            metered_units: 1,
            protected_evidence_reference: None,
        }
    }

    #[test]
    fn implemented_mutation_catalog_is_exact() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 4);
        let ids = definitions
            .iter()
            .map(|definition| definition.capability_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            ids,
            IMPLEMENTED_MUTATION_CAPABILITY_IDS
                .iter()
                .copied()
                .collect()
        );
        for definition in definitions {
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(definition.capability_version.as_str(), "1.0.0");
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
            assert_eq!(definition.risk, CapabilityRisk::Medium);
        }
    }

    #[test]
    fn request_lifecycle_definitions_are_personal_and_registered() {
        for definition in [
            request_create_capability_definition().unwrap(),
            request_cancel_capability_definition().unwrap(),
        ] {
            assert_eq!(
                definition.input_contract.allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert!(
                IMPLEMENTED_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
            );
        }
    }

    #[test]
    fn worker_definitions_are_personal_but_remain_outside_production_catalog() {
        for definition in [
            request_dispatch_capability_definition().unwrap(),
            provider_response_capability_definition().unwrap(),
        ] {
            assert_eq!(
                definition.input_contract.allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert!(!IMPLEMENTED_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()));
        }
    }

    #[test]
    fn created_request_transitions_directly_to_dispatched() {
        let mut request = enrichment_request();
        prepare_request_dispatch(
            &mut request,
            DispatchExpectation {
                status: EnrichmentRequestStatus::Created,
                retry_generation: 0,
            },
            2,
        )
        .unwrap();
        assert_eq!(request.status(), EnrichmentRequestStatus::Dispatched);
        assert_eq!(request.retry_generation(), 0);
    }

    #[test]
    fn queued_request_transitions_to_dispatched() {
        let mut request = enrichment_request();
        request.queue(2).unwrap();
        prepare_request_dispatch(
            &mut request,
            DispatchExpectation {
                status: EnrichmentRequestStatus::Queued,
                retry_generation: 0,
            },
            3,
        )
        .unwrap();
        assert_eq!(request.status(), EnrichmentRequestStatus::Dispatched);
        assert_eq!(request.retry_generation(), 0);
    }

    #[test]
    fn retryable_request_increments_generation_before_dispatch() {
        let mut request = enrichment_request();
        request.fail_retryable("provider_timeout", 2).unwrap();
        prepare_request_dispatch(
            &mut request,
            DispatchExpectation {
                status: EnrichmentRequestStatus::FailedRetryable,
                retry_generation: 0,
            },
            3,
        )
        .unwrap();
        assert_eq!(request.status(), EnrichmentRequestStatus::Dispatched);
        assert_eq!(request.retry_generation(), 1);
    }

    #[test]
    fn stale_dispatch_expectation_is_rejected_without_mutation() {
        let mut request = enrichment_request();
        let error = prepare_request_dispatch(
            &mut request,
            DispatchExpectation {
                status: EnrichmentRequestStatus::Queued,
                retry_generation: 0,
            },
            2,
        )
        .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_REQUEST_DISPATCH_CONFLICT");
        assert_eq!(request.status(), EnrichmentRequestStatus::Created);
        assert_eq!(request.retry_generation(), 0);
    }

    #[test]
    fn valid_provider_response_creates_receipt_and_binds_request() {
        let mut request = dispatched_request();
        let receipt = prepare_provider_response(
            &mut request,
            ResponseExpectation {
                status: EnrichmentRequestStatus::Dispatched,
                retry_generation: 0,
            },
            response_evidence(),
        )
        .unwrap();
        assert_eq!(request.status(), EnrichmentRequestStatus::ResponseRecorded);
        assert_eq!(request.response_receipt_id(), Some(receipt.receipt_id()));
        assert_eq!(receipt.request_id(), request.request_id());
        assert_eq!(receipt.canonical_response_digest(), &[7; 32]);
    }

    #[test]
    fn stale_response_expectation_is_rejected_without_mutation() {
        let mut request = dispatched_request();
        let error = prepare_provider_response(
            &mut request,
            ResponseExpectation {
                status: EnrichmentRequestStatus::Dispatched,
                retry_generation: 1,
            },
            response_evidence(),
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_RESPONSE_EXPECTATION_CONFLICT"
        );
        assert_eq!(request.status(), EnrichmentRequestStatus::Dispatched);
        assert!(request.response_receipt_id().is_none());
    }

    #[test]
    fn invalid_response_evidence_leaves_request_dispatched() {
        let mut request = dispatched_request();
        let mut evidence = response_evidence();
        evidence.canonical_response_digest = [0; 32];
        let error = prepare_provider_response(
            &mut request,
            ResponseExpectation {
                status: EnrichmentRequestStatus::Dispatched,
                retry_generation: 0,
            },
            evidence,
        )
        .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_RESPONSE_DIGEST_INVALID");
        assert_eq!(request.status(), EnrichmentRequestStatus::Dispatched);
        assert!(request.response_receipt_id().is_none());
    }
}
