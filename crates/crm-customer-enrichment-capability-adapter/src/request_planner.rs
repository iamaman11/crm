use crate::{MODULE_ID, provider_profile_version_id_from_external};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, RelationshipMutation, TransactionalAggregatePlanner,
};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, ENRICHMENT_REQUEST_STATE_MAXIMUM_BYTES,
    ENRICHMENT_REQUEST_STATE_SCHEMA_ID, EnrichmentRequest, EnrichmentRequestDraft,
    EnrichmentRequestStatus, LIFECYCLE_STATE_RETENTION_POLICY_ID, LIFECYCLE_STATE_SCHEMA_VERSION,
    MappingVersionId, RequestPolicyEvidence, TargetField, TargetSnapshot,
    encode_enrichment_request_state, enrichment_request_state_descriptor_hash,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, RecordId, RecordRef, RecordSnapshot, RecordType, RelationshipRef,
    RelationshipType, SdkError,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_enrichment::v1 as wire};
use serde::Deserialize;

pub const CREATE_ENRICHMENT_REQUEST_CAPABILITY: &str = "customer_enrichment.request.create";
pub const CREATE_ENRICHMENT_REQUEST_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.CreateEnrichmentRequestRequest";
pub const CREATE_ENRICHMENT_REQUEST_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.CreateEnrichmentRequestResponse";
pub const ENRICHMENT_REQUEST_CREATED_EVENT_TYPE: &str = "customer_enrichment.request.created";
pub const ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.EnrichmentRequestCreatedEvent";
pub const REQUEST_PARTY_RELATIONSHIP_TYPE: &str = "customer_enrichment.request.party";
pub const REQUEST_PARTY_SOURCE_RECORD_TYPE: &str = "parties.party";

const REQUEST_PARTY_LINK_SCHEMA_ID: &str = "crm.customer-enrichment.request.party-link";
const REQUEST_PARTY_LINK_SCHEMA_VERSION: &str = "1.0.0";
const REQUEST_PARTY_LINK_MAXIMUM_BYTES: u64 = 1_024;
const REQUEST_PARTY_LINK_DESCRIPTOR_HASH: [u8; 32] = [
    234, 78, 62, 183, 114, 97, 170, 255, 30, 94, 169, 60, 144, 234, 17, 235, 225, 88,
    121, 223, 86, 225, 45, 149, 201, 194, 155, 186, 10, 226, 131, 230,
];

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentRequestCreateCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentRequestCreateCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let enrichment_request = enrichment_request_from_create_request(request)?;
        Ok(AggregateTarget {
            reference: enrichment_request_record_ref(&enrichment_request)?,
            presence: AggregatePresence::MustBeAbsent,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        if current.is_some() {
            return Err(invalid_plan("deterministic enrichment request already exists"));
        }
        let enrichment_request = enrichment_request_from_create_request(request)?;
        let aggregate = enrichment_request_record_ref(&enrichment_request)?;
        let public_request = enrichment_request_to_wire(&enrichment_request)?;
        let output = support::protobuf_payload(
            MODULE_ID,
            CREATE_ENRICHMENT_REQUEST_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::CreateEnrichmentRequestResponse {
                enrichment_request: Some(public_request.clone()),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: ENRICHMENT_REQUEST_CREATED_EVENT_TYPE,
                event_schema_id: ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Personal,
            &wire::EnrichmentRequestCreatedEvent {
                enrichment_request: Some(public_request),
            },
        )?;
        let audit = support::audit_intent(
            request,
            &aggregate,
            1,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;
        let party_link = RelationshipMutation::Link {
            relationship: RelationshipRef {
                relationship_type: configured_relationship_type()?,
                source: RecordRef {
                    record_type: configured_record_type(REQUEST_PARTY_SOURCE_RECORD_TYPE)?,
                    record_id: RecordId::try_new(enrichment_request.target().resource_id.clone())
                        .map_err(configuration_error)?,
                },
                target: aggregate.clone(),
            },
            payload: request_party_link_payload()?,
        };
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Create {
                    reference: aggregate,
                    payload: enrichment_request_persisted_payload(&enrichment_request)?,
                }],
                relationships: vec![party_link],
                events: vec![event],
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![audit],
            },
            output: Some(output),
        })
    }
}

pub fn enrichment_request_from_create_request(
    request: &CapabilityRequest,
) -> Result<EnrichmentRequest, SdkError> {
    let command: wire::CreateEnrichmentRequestRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let target = command.target.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.request.target",
            "Enrichment target snapshot is required",
        )
    })?;
    let party_ref = target.party_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.request.target.party_ref",
            "Party reference is required",
        )
    })?;
    let provider_profile_version_ref = command.provider_profile_version_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.request.provider_profile_version_ref",
            "Provider-profile version reference is required",
        )
    })?;
    let mapping_version_ref = command.mapping_version_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.request.mapping_version_ref",
            "Mapping version reference is required",
        )
    })?;
    let policy = command.policy_evidence.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.request.policy_evidence",
            "Request policy evidence is required",
        )
    })?;
    EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: request.context.execution.tenant_id.clone(),
        requested_by: request.context.execution.actor_id.clone(),
        idempotency_key: request.context.execution.idempotency_key.clone(),
        target: TargetSnapshot::try_new(
            RecordId::try_new(party_ref.party_id)
                .map_err(|error| {
                    SdkError::invalid_argument(
                        "customer_enrichment.request.target.party_ref.party_id",
                        error.to_string(),
                    )
                })?
                .as_str()
                .to_owned(),
            positive_u64(
                target.party_resource_version,
                "customer_enrichment.request.target.party_resource_version",
            )?,
            target_field_from_wire(
                target.target_field,
                "customer_enrichment.request.target.target_field",
            )?,
        )?,
        provider_profile_version_id: provider_profile_version_id_from_external(
            provider_profile_version_ref.provider_profile_version_id,
        )?,
        mapping_version_id: mapping_version_id_from_external(
            mapping_version_ref.mapping_version_id,
        )?,
        requested_fields: command
            .requested_fields
            .into_iter()
            .map(|value| {
                target_field_from_wire(value, "customer_enrichment.request.requested_fields")
            })
            .collect::<Result<Vec<_>, _>>()?,
        policy_evidence: RequestPolicyEvidence::try_new(
            policy.purpose_code,
            policy.legal_basis_code,
            policy.consent_evidence_reference,
            policy.policy_version,
        )?,
        created_at_unix_ms: request_started_at_unix_ms(request)?,
        deadline_at_unix_ms: positive_u64(
            command.deadline_at_unix_ms,
            "customer_enrichment.request.deadline_at_unix_ms",
        )?,
        expires_at_unix_ms: positive_u64(
            command.expires_at_unix_ms,
            "customer_enrichment.request.expires_at_unix_ms",
        )?,
    })
}

pub fn enrichment_request_to_wire(
    enrichment_request: &EnrichmentRequest,
) -> Result<wire::EnrichmentRequest, SdkError> {
    let state: EnrichmentRequestStateView =
        serde_json::from_slice(&encode_enrichment_request_state(enrichment_request)?)
            .map_err(|error| invalid_plan(error.to_string()))?;
    Ok(wire::EnrichmentRequest {
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: state.request_id,
        }),
        requested_by_actor_id: state.requested_by,
        target: Some(wire::EnrichmentTargetSnapshot {
            party_ref: Some(customer::PartyRef {
                party_id: state.target.resource_id,
            }),
            party_resource_version: checked_i64(
                state.target.resource_version,
                "target resource version",
            )?,
            target_field: target_field_to_wire(state.target.target_field),
        }),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: state.provider_profile_version_id,
        }),
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: state.mapping_version_id,
        }),
        requested_fields: state
            .requested_fields
            .into_iter()
            .map(target_field_to_wire)
            .collect(),
        policy_evidence: Some(wire::EnrichmentRequestPolicyEvidence {
            purpose_code: state.policy_evidence.purpose_code,
            legal_basis_code: state.policy_evidence.legal_basis_code,
            consent_evidence_reference: state.policy_evidence.consent_evidence_reference,
            policy_version: state.policy_evidence.policy_version,
        }),
        created_at_unix_ms: checked_i64(state.created_at_unix_ms, "created timestamp")?,
        deadline_at_unix_ms: checked_i64(state.deadline_at_unix_ms, "deadline timestamp")?,
        expires_at_unix_ms: checked_i64(state.expires_at_unix_ms, "expiry timestamp")?,
        status: request_status_to_wire(state.status),
        retry_generation: state.retry_generation,
        provider_response_receipt_ref: state.response_receipt_id.map(|value| {
            wire::ProviderResponseReceiptRef {
                provider_response_receipt_id: value,
            }
        }),
        last_safe_failure_code: state.last_safe_failure_code,
        updated_at_unix_ms: checked_i64(state.updated_at_unix_ms, "updated timestamp")?,
    })
}

pub fn mapping_version_id_from_external(value: String) -> Result<MappingVersionId, SdkError> {
    const PREFIX: &str = "enrichment-mapping-";
    let suffix = value
        .strip_prefix(PREFIX)
        .ok_or_else(invalid_mapping_version_id)?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(invalid_mapping_version_id());
    }
    serde_json::from_value(serde_json::Value::String(value))
        .map_err(|_| invalid_mapping_version_id())
}

pub fn enrichment_request_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: ENRICHMENT_REQUEST_STATE_SCHEMA_ID,
        schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
        descriptor_hash: enrichment_request_state_descriptor_hash(),
        maximum_size_bytes: ENRICHMENT_REQUEST_STATE_MAXIMUM_BYTES,
        retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
    }
}

pub fn enrichment_request_persisted_payload(
    enrichment_request: &EnrichmentRequest,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        enrichment_request_persisted_contract(),
        DataClass::Personal,
        encode_enrichment_request_state(enrichment_request)?,
    )
}

pub fn enrichment_request_record_ref(
    enrichment_request: &EnrichmentRequest,
) -> Result<RecordRef, SdkError> {
    support::record_ref(
        ENRICHMENT_REQUEST_RECORD_TYPE,
        enrichment_request.request_id().as_str(),
        "customer_enrichment.enrichment_request_ref.enrichment_request_id",
    )
}

fn request_party_link_payload() -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: REQUEST_PARTY_LINK_SCHEMA_ID,
            schema_version: REQUEST_PARTY_LINK_SCHEMA_VERSION,
            descriptor_hash: REQUEST_PARTY_LINK_DESCRIPTOR_HASH,
            maximum_size_bytes: REQUEST_PARTY_LINK_MAXIMUM_BYTES,
            retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        b"{}".to_vec(),
    )
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != CREATE_ENRICHMENT_REQUEST_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan(
            "capability definition does not match request context",
        ));
    }
    Ok(())
}

fn request_started_at_unix_ms(request: &CapabilityRequest) -> Result<u64, SdkError> {
    let nanos = request.context.execution.request_started_at_unix_nanos;
    if nanos < 0 {
        return Err(invalid_plan("request start timestamp is negative"));
    }
    u64::try_from(nanos / 1_000_000)
        .map_err(|_| invalid_plan("request start timestamp cannot be represented in milliseconds"))
}

fn positive_u64(value: i64, field: &'static str) -> Result<u64, SdkError> {
    if value <= 0 {
        return Err(SdkError::invalid_argument(field, "value must be positive"));
    }
    u64::try_from(value).map_err(|_| SdkError::invalid_argument(field, "value is out of range"))
}

fn checked_i64(value: u64, label: &'static str) -> Result<i64, SdkError> {
    i64::try_from(value).map_err(|_| invalid_plan(format!("{label} exceeds the wire range")))
}

fn target_field_from_wire(value: i32, field: &'static str) -> Result<TargetField, SdkError> {
    match wire::EnrichmentTargetField::try_from(value) {
        Ok(wire::EnrichmentTargetField::PartyDisplayName) => Ok(TargetField::PartyDisplayName),
        Ok(wire::EnrichmentTargetField::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            field,
            "Enrichment target field is unsupported",
        )),
    }
}

fn target_field_to_wire(value: TargetField) -> i32 {
    match value {
        TargetField::PartyDisplayName => wire::EnrichmentTargetField::PartyDisplayName as i32,
    }
}

fn request_status_to_wire(value: EnrichmentRequestStatus) -> i32 {
    match value {
        EnrichmentRequestStatus::Created => wire::EnrichmentRequestStatus::Created as i32,
        EnrichmentRequestStatus::Queued => wire::EnrichmentRequestStatus::Queued as i32,
        EnrichmentRequestStatus::Dispatched => wire::EnrichmentRequestStatus::Dispatched as i32,
        EnrichmentRequestStatus::ResponseRecorded => {
            wire::EnrichmentRequestStatus::ResponseRecorded as i32
        }
        EnrichmentRequestStatus::SuggestionsMaterialized => {
            wire::EnrichmentRequestStatus::SuggestionsMaterialized as i32
        }
        EnrichmentRequestStatus::Completed => wire::EnrichmentRequestStatus::Completed as i32,
        EnrichmentRequestStatus::FailedRetryable => {
            wire::EnrichmentRequestStatus::FailedRetryable as i32
        }
        EnrichmentRequestStatus::FailedTerminal => {
            wire::EnrichmentRequestStatus::FailedTerminal as i32
        }
        EnrichmentRequestStatus::Cancelled => wire::EnrichmentRequestStatus::Cancelled as i32,
        EnrichmentRequestStatus::Expired => wire::EnrichmentRequestStatus::Expired as i32,
    }
}

fn configured_relationship_type() -> Result<RelationshipType, SdkError> {
    RelationshipType::try_new(REQUEST_PARTY_RELATIONSHIP_TYPE).map_err(configuration_error)
}

fn configured_record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(configuration_error)
}

fn invalid_mapping_version_id() -> SdkError {
    SdkError::invalid_argument(
        "customer_enrichment.request.mapping_version_ref.mapping_version_id",
        "Mapping-version identity must be a canonical content-derived identifier",
    )
}

fn invalid_plan(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_CREATE_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The enrichment request could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The enrichment request capability is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnrichmentRequestStateView {
    request_id: String,
    #[allow(dead_code)]
    tenant_id: String,
    requested_by: String,
    #[allow(dead_code)]
    idempotency_key: String,
    target: TargetSnapshot,
    provider_profile_version_id: String,
    mapping_version_id: String,
    requested_fields: Vec<TargetField>,
    policy_evidence: RequestPolicyEvidence,
    created_at_unix_ms: u64,
    deadline_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    status: EnrichmentRequestStatus,
    retry_generation: u32,
    response_receipt_id: Option<String>,
    last_safe_failure_code: Option<String>,
    updated_at_unix_ms: u64,
}
